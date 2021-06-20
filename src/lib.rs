//! `countdown` permits the user to create a countdown thread which
//! decrements to zero. The thread keeps track of the how far the
//! countdown has progressed, and also how much time it has taken. The
//! current state of the countdown can be retrieved from another thread.
//!
//! `countdown` is designed for use with long running tasks which need to
//! provide some degree of feedback regarding how far they have progressed,
//! and how much longer the work is likely to take.
//!
//! `countdown` can be accessed across thread boundaries, and may be
//! updated by multiple threads (for monitoring concurrent task loads).

use std::fmt;
use std::sync::mpsc::{channel, sync_channel, Receiver, Sender, SyncSender};
use std::thread::spawn;
use std::time::{Duration, Instant};

/// A [`Countdown`] handle. Create it with the [`start`] function.
#[derive(Debug, Clone)]
pub struct Countdown(SyncSender<Msg>);

impl Countdown {
    /// Decrement the [`Countdown`] by `count`. If the [`Countdown`]
    /// thread has unexpectedly terminated then an [`Error::NoCountdown`]
    /// is returned. If the `count` value is larger than the remaining
    /// countdown in [`Countdown`], then the [`Countdown`] is reduced to
    /// zero.
    pub fn decrement(&self, count: usize) -> Result<()> {
        self.0
            .send(Msg::Decrement(count))
            .map_err(|_err| Error::NoCountdown)
    }

    /// Return the progress made by the [`Countdown`]. If the
    /// [`Countdown`] thread has unexpectedly terminated then an
    /// [`Error::NoCountdown`] is returned.
    pub fn progress(&self) -> Result<Progress> {
        let (tx, rx) = channel();
        self.0
            .send(Msg::State(tx))
            .map_err(|_err| Error::NoCountdown)?;
        let state = rx.recv().map_err(|_err| Error::NoCountdown)?;
        Ok(Progress(state))
    }
}

/// Start a new countdown from `count`. A handle to the [`Countdown`] is
/// returned, which can be used to increment the counter and read its
/// current value.
///
/// The [`Countdown`] will continue to run until the handle (and any
/// clones) are dropped.
pub fn start(count: usize) -> Countdown {
    let (tx, rx) = sync_channel(64);
    let _handle = spawn(move || run(rx, count));
    Countdown(tx)
}

/// The current progress of the [`Countdown`].
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Progress(State);

impl Progress {
    /// The starting value of our [`Countdown`].
    pub fn total(&self) -> usize {
        self.0.total
    }

    /// The number of steps completed.
    pub fn completed(&self) -> usize {
        self.0.total - self.0.remaining
    }

    /// A count of the remaining steps to perform.
    pub fn count(&self) -> usize {
        self.0.remaining
    }

    /// The estimated total runtime for the [`Countdown`]. If we have not
    /// yet completed any steps, this will be `None`, as we lack
    /// sufficient data to estimate a completion time.
    pub fn runtime(&self) -> Option<Duration> {
        self.rate()
            .map(|rate| Duration::from_secs_f32(rate * self.total() as f32))
    }

    /// The current elapsed time spent running.
    pub fn elapsed(&self) -> Duration {
        self.0.elapsed
    }

    /// The estimated time remaining for the [`Countdown`]. If we have not
    /// yet completed any steps, this will be `None`, as we lack
    /// sufficient data to estimate the remaining time.
    pub fn remaining(&self) -> Option<Duration> {
        self.rate()
            .map(|rate| Duration::from_secs_f32(rate * self.count() as f32))
    }

    /// Compute the rate of step completion. This is computed by dividing
    /// the elapsed time by the number of steps completed. If no steps
    /// have yet been completed, then `None` is returned.
    fn rate(&self) -> Option<f32> {
        let completed = self.completed() as f32;
        if completed != 0.0 {
            Some(self.elapsed().as_secs_f32() / completed)
        } else {
            None
        }
    }
}

/// An error type for [`Countdown`].
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Error {
    /// The [`Countdown`] thread terminated unexpectedly.
    NoCountdown,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::NoCountdown => {
                write!(f, "Countdown unexpectedly terminated")
            }
        }
    }
}

/// A specialized [`Result`] type for `countdown` operations.
pub type Result<T> = std::result::Result<T, Error>;

/// The current state of the [`Countdown`] thread.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
struct State {
    /// The starting value of our [`Countdown`].
    total: usize,
    /// The remaining count for our [`Countdown`].
    remaining: usize,
    /// The elapsed time spent running.
    elapsed: Duration,
}

/// Private message enum used to communicate with the [`Countdown`]
/// thread.
#[derive(Debug)]
enum Msg {
    /// Message used to decrement the [`Countdown`] by a given amount.
    Decrement(usize),
    /// Message used to return the [`Countdown`] [`State`] to a different
    /// thread.
    State(Sender<State>),
}

/// Run loop for the countdown. This just initializes the countdown value,
/// and enters a [`recv<Msg>`](Receiver::recv) loop. The loop will
/// continue to run until the [`Receiver`] is dropped by the [`Countdown`]
/// handle. Note that if the [`Countdown`] handle is cloned, then the
/// thread will run until all clones have been dropped.
///
/// Note that if a [`Msg::State`] message is received and the [`Receiver`]
/// corresponding to the [`Sender`] has been dropped, then the loop will
/// continue without returning a value to the [`Sender`]. This should
/// never happen, though, as the only way it should be callable is through
/// [`Countdown::progress()`].
fn run(rx: Receiver<Msg>, count: usize) {
    let start_time = Instant::now();
    let mut countdown = count;
    loop {
        match rx.recv() {
            Ok(Msg::Decrement(c)) => {
                if c > countdown {
                    countdown = 0;
                } else {
                    countdown -= c;
                }
            }
            Ok(Msg::State(tx)) => {
                let state = State {
                    total: count,
                    remaining: countdown,
                    elapsed: Instant::now() - start_time,
                };
                tx.send(state).unwrap_or(());
            }
            Err(_err) => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn use_countdown() {
        let countdown = start(10_000);
        for _i in 0..1000 {
            let result = countdown.decrement(10);
            assert_eq!(result, Ok(()));
        }
        let result = countdown.progress();
        assert!(result.is_ok());
        let progress = result.unwrap();
        assert_eq!(progress.total(), 10_000);
        assert_eq!(progress.completed(), 10_000);
        assert_eq!(progress.count(), 0);
        assert!(progress.elapsed() >= Duration::new(0, 0));
        assert!(progress.runtime().unwrap() >= Duration::new(0, 0));
        assert_eq!(progress.remaining().unwrap(), Duration::new(0, 0));
    }

    #[test]
    fn countdown_below_zero() {
        let countdown = start(10);
        let result = countdown.decrement(20);
        assert_eq!(result, Ok(()));
        let result = countdown.progress();
        assert!(result.is_ok());
        let progress = result.unwrap();
        assert_eq!(progress.count(), 0);
    }

    #[test]
    fn display_errors() {
        assert_eq!(
            format!("{}", Error::NoCountdown),
            "Countdown unexpectedly terminated"
        );
    }
}
