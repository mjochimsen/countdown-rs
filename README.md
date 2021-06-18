# countdown

`countdown` is a Rust crate which permits the user to create a countdown
thread which decrements to zero. The thread keeps track of the how far the
countdown has progressed, and also how long it has taken. The current
state of the countdown can be retrieved from another thread.

The crate is designed for use with long running tasks which need to
provide some degree of feedback regarding how far they have progressed,
and how much longer the work is likely to take.

## Example

Add `countdown` to your `Cargo.toml`.

```toml
[dependencies]
countdown = "0.1"
```
