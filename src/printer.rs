//! One printer, on the network or on USB (`usb:<serial>`): its status every few seconds, and
//! print jobs, on a thread of its own.
//!
//! Around a print no link asks for a status: every request over USB keeps the printer with
//! USB until USB is quiet for a few seconds, and meanwhile network jobs wait in it (one
//! printer can be set on the network and on USB). So a job stops the polling of every link
//! and goes out at once; the polling stays held until it has printed. The network doesn't
//! tell when: the time the printer may still be with USB ([`USB_RELEASE`]) and the tape
//! takes, then until the printer is ready again ([`NOT_READY_MAX`] at most).
//!
//! A USB printer that also answers on the network (the same serial) isn't asked over USB at
//! all: its status is the network's (SNMP, from the network card: it doesn't hold the
//! printer). Only a job on its USB link reads the status there, right before.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::i18n::tf;
use ql_raster::label::Bitmap;
use ql_raster::{Media, Status, job, net, raster, usb};

/// How often the status is asked.
const POLL: Duration = Duration::from_secs(3);

/// How long the printer stays with USB after the last request there (measured on the QL-710W:
/// within 6 s; a little more).
const USB_RELEASE: Duration = Duration::from_secs(7);

/// Polling held at most this long for one job (if something goes wrong on the way).
const JOB_HOLD_MAX: Duration = Duration::from_secs(180);

/// After a job, how long to wait at most for the printer to report ready.
const NOT_READY_MAX: Duration = Duration::from_secs(30);

/// Tape speed taken for a print time: slower than the QL's 150 mm/s at its fastest (HQ,
/// cooling, the cut), so the estimate errs long.
const MM_PER_SECOND: f32 = 50.0;

/// The polling of every link: held while a job prints, and when USB was last asked.
struct Polling {
    held_until: Option<Instant>,
    last_usb: Option<Instant>,
}

static POLLING: Mutex<Polling> = Mutex::new(Polling {
    held_until: None,
    last_usb: None,
});

fn polling<T>(change: impl FnOnce(&mut Polling) -> T) -> Option<T> {
    POLLING.lock().ok().map(|mut polling| change(&mut polling))
}

/// Until when the polling is held, if it is.
fn held_until() -> Option<Instant> {
    polling(|polling| polling.held_until)
        .flatten()
        .filter(|until| *until > Instant::now())
}

/// How long the printer may still be with USB (asked there less than `USB_RELEASE` ago): a
/// network job waits in it that long.
fn usb_release() -> Duration {
    polling(|polling| polling.last_usb)
        .flatten()
        .map_or(Duration::ZERO, |last| {
            (last + USB_RELEASE).saturating_duration_since(Instant::now())
        })
}

/// About how long `pages` take to print (lines of `resolution`), cut and fed.
fn print_time(
    pages: &[Vec<[u8; raster::BYTES_PER_LINE]>],
    resolution: job::Resolution,
) -> Duration {
    let lines_per_mm = match resolution {
        job::Resolution::Normal => 300.0 / 25.4,
        job::Resolution::High => 600.0 / 25.4,
    };
    let lines: usize = pages.iter().map(Vec::len).sum();
    Duration::from_secs_f32(2.0 + pages.len() as f32 + lines as f32 / lines_per_mm / MM_PER_SECOND)
}

/// What is known about a printer.
#[derive(Debug, Clone)]
pub enum State {
    /// Not asked yet.
    Checking,
    /// It answered.
    Online {
        /// Its status (tape, errors).
        status: Status,
        /// Its model (`Brother QL-710W`), when it says.
        model: Option<String>,
        /// Its own name (SNMP `sysName`), when it has one.
        name: Option<String>,
    },
    /// It didn't answer.
    Offline,
}

impl State {
    /// The tape loaded, when the printer is ready to print.
    #[must_use]
    pub fn ready_media(&self) -> Option<&'static Media> {
        match self {
            Self::Online { status, .. } if status.is_ready() => status.loaded_media(),
            _ => None,
        }
    }
}

/// Labels (raster bitmaps) for a medium, cut and at a resolution: one job.
type Job = (Vec<Bitmap>, &'static Media, job::Cut, job::Resolution);

/// A link to one printer.
pub struct Link {
    /// Its address.
    pub host: String,
    state: Arc<Mutex<State>>,
    jobs: Sender<Job>,
    results: Receiver<Result<String, String>>,
}

impl Link {
    /// Starts asking `host`; `wake` is called when something changed (the GUI repaints).
    #[must_use]
    pub fn start(host: String, wake: impl Fn() + Send + 'static) -> Self {
        let state = Arc::new(Mutex::new(State::Checking));
        let (jobs, job_receiver) = mpsc::channel();
        let (result_sender, results) = mpsc::channel();
        let (worker_host, worker_state) = (host.clone(), Arc::clone(&state));
        std::thread::spawn(move || {
            worker(
                &worker_host,
                &worker_state,
                &job_receiver,
                &result_sender,
                &wake,
            );
        });
        Self {
            host,
            state,
            jobs,
            results,
        }
    }

    /// The latest state.
    #[must_use]
    pub fn state(&self) -> State {
        self.state
            .lock()
            .map(|state| state.clone())
            .unwrap_or(State::Checking)
    }

    /// Queues labels (raster bitmaps) for `media`, as one job, cut as `cut` says, drawn at
    /// `resolution`.
    pub fn print(
        &self,
        labels: Vec<Bitmap>,
        media: &'static Media,
        cut: job::Cut,
        resolution: job::Resolution,
    ) {
        let _ = self.jobs.send((labels, media, cut, resolution));
    }

    /// Whether its model prints in high resolution (as far as known now).
    #[must_use]
    pub fn has_high_resolution(&self) -> bool {
        matches!(self.state(), State::Online { status, .. } if status.model.high_resolution())
    }

    /// Print results since the last call: the printer's name, or why not.
    pub fn results(&self) -> Vec<Result<String, String>> {
        self.results.try_iter().collect()
    }
}

/// The network printers that answer: address, serial and latest status.
static ON_THE_NETWORK: Mutex<Vec<(String, String, Status)>> = Mutex::new(Vec::new());

/// The latest network status of USB printer `address`, when it answers on the network too.
#[must_use]
pub fn network_status(address: &str) -> Option<Status> {
    ON_THE_NETWORK
        .lock()
        .ok()?
        .iter()
        .find_map(|(_, serial, status)| usb::has_serial(address, serial).then(|| status.clone()))
}

/// Notes network printer `host`'s status (`None`: it doesn't answer); its serial is asked once.
fn note_network(host: &str, status: Option<&Status>) {
    let Ok(mut known) = ON_THE_NETWORK.lock() else {
        return;
    };
    let serial = known
        .iter()
        .find(|(known, ..)| known == host)
        .map(|(_, serial, _)| serial.clone());
    known.retain(|(known, ..)| known != host);
    let Some(status) = status else {
        return;
    };
    drop(known);
    let serial = serial.or_else(|| net::query_serial(host).ok().flatten());
    if let (Some(serial), Ok(mut known)) = (serial, ON_THE_NETWORK.lock()) {
        known.push((host.to_owned(), serial, status.clone()));
    }
}

/// The state for polling: a USB printer that answers on the network from there.
fn poll_state(host: &str) -> State {
    if usb::is_usb(host)
        && let Some(status) = network_status(host)
    {
        return State::Online {
            model: Some(format!("Brother {}", status.model)),
            name: Some("USB".to_owned()),
            status,
        };
    }
    read_state(host)
}

/// The state, asked of the printer itself.
fn read_state(host: &str) -> State {
    if usb::is_usb(host) {
        let state = read_usb_state(host);
        polling(|polling| polling.last_usb = Some(Instant::now()));
        return state;
    }
    match net::query_status(host) {
        Ok(status) => {
            note_network(host, Some(&status));
            State::Online {
                status,
                model: net::query_model_name(host).ok(),
                name: net::query_name(host).ok().flatten(),
            }
        }
        Err(_) => {
            note_network(host, None);
            State::Offline
        }
    }
}

/// On USB the status is all there is: the model from it, "USB" as its name.
fn read_usb_state(host: &str) -> State {
    match usb::query_status(host) {
        Ok(status) => State::Online {
            model: Some(format!("Brother {}", status.model)),
            name: Some("USB".to_owned()),
            status,
        },
        Err(_) => State::Offline,
    }
}

fn worker(
    host: &str,
    state: &Mutex<State>,
    jobs: &Receiver<Job>,
    results: &Sender<Result<String, String>>,
    wake: &dyn Fn(),
) {
    let publish = |new: State| {
        if let Ok(mut current) = state.lock() {
            *current = new;
        }
        wake();
    };
    let mut next_poll = Instant::now();
    // After this link's job: until when to wait for the printer to be ready again.
    let mut ready_by: Option<Instant> = None;

    loop {
        // Held: look again shortly (the hold ends when the jobs have printed). This link's
        // jobs: then until the printer is ready, holding the others a little longer each time.
        let wait = if held_until().is_some() {
            Duration::from_millis(500)
        } else if let Some(deadline) = ready_by {
            let now = poll_state(host);
            let ready = matches!(&now, State::Online { status, .. } if status.is_ready());
            publish(now);
            if ready || Instant::now() > deadline {
                ready_by = None;
                polling(|polling| polling.held_until = None);
            } else {
                polling(|polling| polling.held_until = Some(Instant::now() + POLL));
            }
            next_poll = Instant::now() + POLL;
            Duration::from_millis(500)
        } else {
            if Instant::now() >= next_poll {
                publish(poll_state(host));
                next_poll = Instant::now() + POLL;
            }
            next_poll.saturating_duration_since(Instant::now())
        };
        let (labels, media, cut, resolution) = match jobs.recv_timeout(wait) {
            Ok(job) => job,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return,
        };

        // No link asks anything until this job has printed (after the ones before it).
        let before = held_until();
        polling(|polling| polling.held_until = Some(Instant::now() + JOB_HOLD_MAX));
        let on_usb = usb::is_usb(host);
        // The tape right before printing, not from a status seconds ago.
        let fresh = read_state(host);
        let sent = match &fresh {
            State::Online { status, .. } if status.has_media(media) && status.is_ready() => {
                labels
                    .iter()
                    .map(|bitmap| raster::raster_lines_at(bitmap, media, resolution))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| error.to_string())
                    .and_then(|pages| {
                        let page_refs: Vec<&[_]> = pages.iter().map(Vec::as_slice).collect();
                        // TIFF on models that take it: a fraction of the data.
                        let compression = if status.model.compression() {
                            job::Compression::Tiff
                        } else {
                            job::Compression::None
                        };
                        let bytes =
                            job::pages_bytes(&page_refs, media, cut, resolution, compression);
                        // Over USB the printer says when it's done; the network doesn't.
                        if on_usb {
                            usb::send_job(host, &bytes).map(|()| Duration::ZERO)
                        } else {
                            net::send_job(host, &bytes)
                                .map(|()| usb_release() + print_time(&pages, resolution))
                        }
                        .map_err(|error| error.to_string())
                    })
            }
            State::Online { status, .. } if !status.is_ready() => Err(tf(
                "{host} reports {errors}",
                &[
                    ("host", &host),
                    ("errors", &status.error_names().join(", ")),
                ],
            )),
            State::Online { status, .. } => Err(tf(
                "{host} now has {tape} loaded",
                &[("host", &host), ("tape", &status.media_description())],
            )),
            _ => Err(tf("{host} doesn't answer", &[("host", &host)])),
        };
        let outcome = sent.as_ref().map(|_| match &fresh {
            State::Online { name, model, .. } => name
                .clone()
                .or_else(|| model.clone())
                .unwrap_or_else(|| host.to_owned()),
            _ => host.to_owned(),
        });
        publish(fresh);
        let _ = results.send(outcome.map_err(Clone::clone));
        wake();

        // Printed: hold the polling for the time it takes (after what still prints), then
        // look whether the printer is ready. Not sent: as it was.
        let until = match sent {
            Ok(printing) => {
                let printed = before.unwrap_or_else(Instant::now).max(Instant::now()) + printing;
                ready_by = Some(printed + NOT_READY_MAX);
                Some(printed)
            }
            Err(_) => before,
        };
        polling(|polling| polling.held_until = until);
        next_poll = Instant::now() + POLL;
    }
}
