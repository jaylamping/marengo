//! Operator text on stdout and stderr (the stdout contract lines MCP waits on,
//! refusals, usage).
//!
//! `println!` panics when the pipe is closed: Rust ignores SIGPIPE, so the
//! write fails with EPIPE. That happens when an MCP bench session's ssh drops
//! and its `tee` dies while marengo-pi keeps running (TOOL-06). Here a closed
//! pipe is recorded in [`OUTPUT_CLOSED`] instead, and `run_control_loop` treats
//! it like `quit`: the normal shutdown stops every drive. Any other write
//! error drops the line. Log lines on stdout set the same flag
//! (`chappe::tracing_layer::init_subscriber_watching_stdout`).

use std::fmt;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};

/// Set once a write to stdout or stderr failed with EPIPE: nobody reads the
/// operator text any more.
pub(crate) static OUTPUT_CLOSED: AtomicBool = AtomicBool::new(false);

/// Write one line to `out`; on EPIPE set `closed`. Returns whether the line
/// was written.
pub(crate) fn write_line(
    out: &mut impl Write,
    args: fmt::Arguments<'_>,
    closed: &AtomicBool,
) -> bool {
    match writeln!(out, "{args}").and_then(|()| out.flush()) {
        Ok(()) => true,
        Err(error) => {
            if error.kind() == io::ErrorKind::BrokenPipe {
                closed.store(true, Ordering::SeqCst);
            }
            false
        }
    }
}

#[cfg(not(test))]
pub(crate) fn stdout_line(args: fmt::Arguments<'_>) {
    write_line(&mut io::stdout().lock(), args, &OUTPUT_CLOSED);
}

#[cfg(not(test))]
pub(crate) fn stderr_line(args: fmt::Arguments<'_>) {
    write_line(&mut io::stderr().lock(), args, &OUTPUT_CLOSED);
}

// Tests keep the harness's output capture, which only the print macros reach.
#[cfg(test)]
pub(crate) fn stdout_line(args: fmt::Arguments<'_>) {
    if !closed_pipe_line(args) {
        println!("{args}");
    }
}

#[cfg(test)]
pub(crate) fn stderr_line(args: fmt::Arguments<'_>) {
    if !closed_pipe_line(args) {
        eprintln!("{args}");
    }
}

/// A pipe whose reader is gone: every write fails with EPIPE.
#[cfg(test)]
struct ClosedPipe;

#[cfg(test)]
impl Write for ClosedPipe {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        Err(io::Error::from(io::ErrorKind::BrokenPipe))
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::from(io::ErrorKind::BrokenPipe))
    }
}

#[cfg(test)]
thread_local! {
    /// Test seam: while set, this thread's operator lines go to a closed pipe
    /// that records the EPIPE in the given flag, as the real stdout would.
    static CLOSED_PIPE: std::cell::Cell<Option<&'static AtomicBool>> =
        const { std::cell::Cell::new(None) };
}

/// Send this thread's operator lines to a closed pipe recording into
/// `closed` (`None` restores the harness capture).
#[cfg(test)]
pub(crate) fn close_pipe_on_this_thread(closed: Option<&'static AtomicBool>) {
    CLOSED_PIPE.with(|pipe| pipe.set(closed));
}

#[cfg(test)]
fn closed_pipe_line(args: fmt::Arguments<'_>) -> bool {
    let Some(closed) = CLOSED_PIPE.with(std::cell::Cell::get) else {
        return false;
    };
    write_line(&mut ClosedPipe, args, closed);
    true
}

/// `println!` that never panics; see the module doc.
macro_rules! say {
    ($($arg:tt)*) => {
        $crate::operator_output::stdout_line(format_args!($($arg)*))
    };
}

/// `eprintln!` that never panics; see the module doc.
macro_rules! say_err {
    ($($arg:tt)*) => {
        $crate::operator_output::stderr_line(format_args!($($arg)*))
    };
}

pub(crate) use {say, say_err};

#[cfg(test)]
mod tests {
    use super::*;

    struct Full;

    impl Write for Full {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::from(io::ErrorKind::WriteZero))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_closed_pipe_is_recorded_instead_of_panicking() {
        let closed = AtomicBool::new(false);
        assert!(!write_line(
            &mut ClosedPipe,
            format_args!("disabled"),
            &closed
        ));
        assert!(closed.load(Ordering::SeqCst));
    }

    #[test]
    fn other_write_errors_drop_the_line_without_closing() {
        let closed = AtomicBool::new(false);
        assert!(!write_line(&mut Full, format_args!("disabled"), &closed));
        assert!(!closed.load(Ordering::SeqCst));
    }

    #[test]
    fn a_line_is_written_with_its_newline() {
        let closed = AtomicBool::new(false);
        let mut out = Vec::new();
        assert!(write_line(
            &mut out,
            format_args!("enabled (operator={})", "bench"),
            &closed
        ));
        assert_eq!(out, b"enabled (operator=bench)\n");
        assert!(!closed.load(Ordering::SeqCst));
    }
}
