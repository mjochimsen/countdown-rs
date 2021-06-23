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

/// A [`Countdown`] handle. Create it with the [`Countdown::start`]
/// function.
#[derive(Debug, Clone)]
pub struct Countdown(SyncSender<Msg>);

impl Countdown {
    /// Start a new countdown from `count`. A handle to the [`Countdown`]
    /// is returned, which can be used to decrement the counter and read
    /// its current progress.
    ///
    /// The [`Countdown`] will continue to run until all references to it
    /// are dropped.
    pub fn start(count: usize) -> Self {
        let (tx, rx) = sync_channel(64);
        let _handle = spawn(move || run(rx, count));
        Self(tx)
    }

    /// Decrement the [`Countdown`] by `count`. If the [`Countdown`]
    /// thread has unexpectedly terminated then [`Error::Terminated`] is
    /// returned. If the `count` value is larger than the remaining
    /// countdown in [`Countdown`], then the [`Countdown`] is reduced to
    /// zero.
    pub fn decrement(&self, count: usize) -> Result<()> {
        self.0
            .send(Msg::Decrement(count))
            .map_err(|_err| Error::Terminated)
    }

    /// Return the progress made by the [`Countdown`]. If the
    /// [`Countdown`] thread has unexpectedly terminated then
    /// [`Error::Terminated`] is returned.
    pub fn progress(&self) -> Result<Progress> {
        let (tx, rx) = channel();
        self.0
            .send(Msg::State(tx))
            .map_err(|_err| Error::Terminated)?;
        let state = rx.recv().map_err(|_err| Error::Terminated)?;
        Ok(Progress(state))
    }
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
    /// The [`Countdown`] thread terminated unexpectedly. This should not
    /// normally happen, but in a stressed system it may be terminated by
    /// the OS. The entire process will likely be terminated should this
    /// happen, though.
    Terminated,
}

impl fmt::Display for Error {
    /// Format the Error for display.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Terminated => {
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

/// Private message `enum` used to communicate with the [`Countdown`]
/// thread.
#[derive(Debug)]
enum Msg {
    /// Message used to decrement the [`Countdown`] by a given amount.
    Decrement(usize),
    /// Message used to return the [`State`] of the [`Countdown`] to the
    /// calling thread.
    State(Sender<State>),
}

/// Run loop for the countdown. This just initializes the countdown value,
/// and enters a [`recv<Msg>`](Receiver::recv) loop. The loop will
/// continue to run until the [`Receiver`] is dropped by the
/// [`Countdown`] (which won't happen until the [`Countdown`] itself is
/// dropped). Note that if the [`Countdown`] handle is cloned, then the
/// thread will run until all clones have been also been dropped.
///
/// If a [`Msg::State`] message is received and the [`Receiver`]
/// corresponding to the [`Sender`] has been dropped, then the loop will
/// continue without returning a value to the [`Sender`]. This should
/// never happen, though, as the only way it should be callable is through
/// [`Countdown::progress()`], which doesn't drop the [`Sender`] until it
/// has received the [`Progress`].
///
/// Attempts to count down below zero will stop at zero.
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
    fn start_countdown() {
        let _countdown = Countdown::start(10);
    }

    #[test]
    fn get_progress() {
        let countdown = Countdown::start(10);
        for i in 0..5 {
            let result = countdown.progress();
            assert!(result.is_ok());
            let progress = result.unwrap();
            assert_eq!(progress.total(), 10);
            assert_eq!(progress.completed(), i * 2);
            assert_eq!(progress.count(), 10 - (i * 2));
            let result = countdown.decrement(2);
            assert_eq!(result, Ok(()));
        }
        let result = countdown.progress();
        assert!(result.is_ok());
        let progress = result.unwrap();
        assert_eq!(progress.total(), 10);
        assert_eq!(progress.completed(), 10);
        assert_eq!(progress.count(), 0);
    }

    /// Note that this test makes some assumptions about the speed of the
    /// system it is running on. If the countdown takes longer than 10ms,
    /// then it will fail.
    #[test]
    fn get_time_estimates() {
        // Set constants for 0s and 10ms.
        let zero = Duration::new(0, 0);
        let expected = Duration::new(0, 10_000_000);
        // Start the Countdown.
        let countdown = Countdown::start(2);
        // Get the Progress prior to any decrements.
        let result = countdown.progress();
        assert!(result.is_ok());
        let progress = result.unwrap();
        assert!(progress.elapsed() < expected);
        assert!(progress.runtime().is_none());
        assert!(progress.remaining().is_none());
        // Decrement by 1.
        let result = countdown.decrement(1);
        assert!(result.is_ok());
        // Get the Progress after the decrement.
        let result = countdown.progress();
        assert!(result.is_ok());
        let progress = result.unwrap();
        assert!(progress.elapsed() < expected);
        assert!(progress.runtime().is_some());
        let runtime = progress.runtime().unwrap();
        assert!(runtime < expected);
        assert!(progress.remaining().is_some());
        let remaining = progress.remaining().unwrap();
        assert!(remaining < expected);
        // Decrement by 1.
        let result = countdown.decrement(1);
        assert!(result.is_ok());
        // Get the Progress after the decrement. The Countdown should now
        // be finished.
        let result = countdown.progress();
        assert!(result.is_ok());
        let progress = result.unwrap();
        assert!(progress.elapsed() < expected);
        assert!(progress.runtime().is_some());
        let runtime = progress.runtime().unwrap();
        assert!(runtime == progress.elapsed());
        assert!(progress.remaining().is_some());
        let remaining = progress.remaining().unwrap();
        assert!(remaining == zero);
    }

    #[test]
    fn countdown_below_zero() {
        let countdown = Countdown::start(10);
        let result = countdown.decrement(20);
        assert_eq!(result, Ok(()));
        let result = countdown.progress();
        assert!(result.is_ok());
        let progress = result.unwrap();
        assert_eq!(progress.total(), 10);
        assert_eq!(progress.completed(), 10);
        assert_eq!(progress.count(), 0);
    }

    #[test]
    fn display_errors() {
        assert_eq!(
            format!("{}", Error::Terminated),
            "Countdown unexpectedly terminated"
        );
    }
}
