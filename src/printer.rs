//! One printer: its status every few seconds, and print jobs, on a thread of its own.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chopshop_i18n::tf;
use chopshop_label::Bitmap;
use chopshop_printer::{Media, Status, job, net, raster};

/// How often the status is asked.
const POLL: Duration = Duration::from_secs(3);

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

fn read_state(host: &str) -> State {
    match net::query_status(host) {
        Ok(status) => State::Online {
            status,
            model: net::query_model_name(host).ok(),
            name: net::query_name(host).ok().flatten(),
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

    loop {
        if Instant::now() >= next_poll {
            publish(read_state(host));
            next_poll = Instant::now() + POLL;
        }
        let (labels, media, cut, resolution) =
            match jobs.recv_timeout(next_poll.saturating_duration_since(Instant::now())) {
                Ok(job) => job,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return,
            };

        // The tape right before printing, not from a status seconds ago.
        let fresh = read_state(host);
        let outcome = match &fresh {
            State::Online {
                status,
                model,
                name,
            } if status.has_media(media) && status.is_ready() => labels
                .iter()
                .map(|bitmap| raster::raster_lines_at(bitmap, media, resolution))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())
                .and_then(|pages| {
                    let pages: Vec<&[_]> = pages.iter().map(Vec::as_slice).collect();
                    net::send_job(host, &job::pages_bytes(&pages, media, cut, resolution))
                        .map_err(|error| error.to_string())
                })
                .map(|()| {
                    name.clone()
                        .or_else(|| model.clone())
                        .unwrap_or_else(|| host.to_owned())
                }),
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
        publish(fresh);
        if results.send(outcome).is_err() {
            return;
        }
        wake();
    }
}
