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
        /// Its name, when it says.
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

/// A link to one printer.
pub struct Link {
    /// Its address.
    pub host: String,
    state: Arc<Mutex<State>>,
    jobs: Sender<(Bitmap, &'static Media)>,
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

    /// Queues a label (a raster bitmap) for `media`.
    pub fn print(&self, bitmap: Bitmap, media: &'static Media) {
        let _ = self.jobs.send((bitmap, media));
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
            name: net::query_model_name(host).ok(),
        },
        Err(_) => State::Offline,
    }
}

fn worker(
    host: &str,
    state: &Mutex<State>,
    jobs: &Receiver<(Bitmap, &'static Media)>,
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
        let (bitmap, media) =
            match jobs.recv_timeout(next_poll.saturating_duration_since(Instant::now())) {
                Ok(job) => job,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return,
            };

        // The tape right before printing, not from a status seconds ago.
        let fresh = read_state(host);
        let outcome = match &fresh {
            State::Online { status, name } if status.has_media(media) && status.is_ready() => {
                raster::raster_lines(&bitmap, media)
                    .map_err(|error| error.to_string())
                    .and_then(|lines| {
                        net::send_job(host, &job::job_bytes(&lines, media))
                            .map_err(|error| error.to_string())
                    })
                    .map(|()| name.clone().unwrap_or_else(|| host.to_owned()))
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
        publish(fresh);
        if results.send(outcome).is_err() {
            return;
        }
        wake();
    }
}
