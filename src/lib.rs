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

/// A [`Countdown`] handle. Create it with the [`start`] function.
#[derive(Debug, Clone)]
pub struct Countdown(SyncSender<Msg>);

impl Countdown {
    /// Decrement the [`Countdown`] by `count`. If the [`Countdown`]
    /// thread has unexpectedly terminated then an [`Error::NoCountdown`]
    /// is returned.
    pub fn decrement(&self, count: usize) -> Result<()> {
        self.0
            .send(Msg::Decrement(count))
            .map_err(|_err| Error::NoCountdown)
    }

    /// Return the current count held by the [`Countdown`]. If the
    /// [`Countdown`] thread has unexpectedly terminated then an
    /// [`Error::NoCountdown`] is returned.
    pub fn get(&self) -> Result<usize> {
        let (tx, rx) = channel();
        self.0
            .send(Msg::Get(tx))
            .map_err(|_err| Error::NoCountdown)?;
        let count = rx.recv().map_err(|_err| Error::NoCountdown)?;
        Ok(count)
    }
}

/// An error type for [`Countdown`].
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Error {
    /// The [`Countdown`] thread terminated unexpectedly.
    NoCountdown,
}

/// A specialized [`Result`] type for `countdown` operations.
pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::NoCountdown => {
                write!(f, "Countdown unexpectedly terminated")
            }
        }
    }
}

/// Private message enum used to communicate with the [`Countdown`]
/// thread.
#[derive(Debug)]
enum Msg {
    /// Message used to decrement the [`Countdown`] by a given amount.
    Decrement(usize),
    /// Message used to return the [`Countdown`] value to a different
    /// thread.
    Get(Sender<usize>),
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

/// Run loop for the countdown. This just initializes the countdown value,
/// and enters a [`recv<Msg>`](Receiver::recv) loop. The loop will
/// continue to run until the [`Receiver`] is dropped by the [`Countdown`]
/// handle. Note that if the [`Countdown`] handle is cloned, then the
/// thread will run until all clones have been dropped.
///
/// Note that if a [`Msg::Get`] message is received and the [`Receiver`]
/// corresponding to the [`Sender`] has been dropped, then the loop will
/// continue without returning a value to the [`Sender`]. This should
/// never happen, though, as the only way it should be callable is through
/// [`Countdown::get()`].
fn run(rx: Receiver<Msg>, count: usize) {
    let mut countdown = count;
    loop {
        match rx.recv() {
            Ok(Msg::Decrement(c)) => countdown -= c,
            Ok(Msg::Get(tx)) => tx.send(countdown).unwrap_or(()),
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
        assert_eq!(countdown.get(), Ok(0));
    }

    #[test]
    fn display_errors() {
        assert_eq!(
            format!("{}", Error::NoCountdown),
            "Countdown unexpectedly terminated"
        );
    }
}
