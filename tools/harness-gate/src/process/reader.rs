//! Bounded process-output readers.
//!
//! Child processes must not be able to grow an in-memory buffer without a
//! host-owned limit.  Readers run independently from the waiter so a process
//! that keeps a pipe open after termination cannot make the caller join
//! forever.

use std::io::{self, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub(crate) const DEFAULT_CAPTURE_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const DEFAULT_READER_DEADLINE: Duration = Duration::from_secs(2);

#[derive(Debug)]
pub(crate) struct LimitedOutput {
    pub(crate) bytes: Vec<u8>,
    pub(crate) truncated: bool,
}

/// Start a reader that retains at most `limit` bytes.  The overflow flag is
/// set before the reader returns, allowing the process waiter to terminate a
/// noisy child promptly.
pub(crate) fn spawn_limited_reader<R>(
    reader: R,
    limit: usize,
    overflow: Arc<AtomicBool>,
) -> (JoinHandle<()>, Receiver<io::Result<LimitedOutput>>)
where
    R: Read + Send + 'static,
{
    let (sender, receiver) = mpsc::sync_channel(1);
    let handle = thread::spawn(move || {
        let result = read_limited(reader, limit, &overflow);
        let _ = sender.send(result);
    });
    (handle, receiver)
}

/// Finish a reader without waiting longer than the independent reader
/// deadline.  A timed-out handle is deliberately detached; its bounded reader
/// cannot grow memory while a descendant-owned pipe remains open.
pub(crate) fn collect_limited_reader(
    handle: JoinHandle<()>,
    receiver: Receiver<io::Result<LimitedOutput>>,
    deadline: Duration,
    stream: &str,
) -> io::Result<LimitedOutput> {
    match receiver.recv_timeout(deadline) {
        Ok(result) => {
            if handle.join().is_err() {
                return Err(io::Error::other(format!("{stream} reader thread panicked")));
            }
            result
        }
        Err(RecvTimeoutError::Timeout) => {
            drop(handle);
            Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!(
                    "{stream} reader deadline exceeded after {} ms",
                    deadline.as_millis()
                ),
            ))
        }
        Err(RecvTimeoutError::Disconnected) => {
            let _ = handle.join();
            Err(io::Error::other(format!("{stream} reader disconnected")))
        }
    }
}

fn read_limited(
    mut reader: impl Read,
    limit: usize,
    overflow: &AtomicBool,
) -> io::Result<LimitedOutput> {
    let mut output = Vec::with_capacity(limit.min(64 * 1024));
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            return Ok(LimitedOutput {
                bytes: output,
                truncated: false,
            });
        }
        let remaining = limit.saturating_sub(output.len());
        if read > remaining {
            output.extend_from_slice(&buffer[..remaining]);
            overflow.store(true, Ordering::Release);
            return Ok(LimitedOutput {
                bytes: output,
                truncated: true,
            });
        }
        output.extend_from_slice(&buffer[..read]);
    }
}
