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

/// Pipe types the bounded reader accepts. On Unix the reader polls the
/// descriptor so a timed-out collection can stop it and close the read end.
#[cfg(unix)]
pub(crate) trait OutputPipe: Read + std::os::fd::AsFd + Send + 'static {}
#[cfg(unix)]
impl<T: Read + std::os::fd::AsFd + Send + 'static> OutputPipe for T {}
#[cfg(not(unix))]
pub(crate) trait OutputPipe: Read + Send + 'static {}
#[cfg(not(unix))]
impl<T: Read + Send + 'static> OutputPipe for T {}

/// Reader thread plus the host-owned stop request used after a deadline.
pub(crate) struct ReaderThread {
    handle: JoinHandle<()>,
    stop: Arc<AtomicBool>,
}

/// Poll interval for observing a stop request while no output arrives.
#[cfg(unix)]
const STOP_POLL_MS: libc::c_int = 50;

/// Start a reader that retains at most `limit` bytes.  The overflow flag is
/// set before the reader returns, allowing the process waiter to terminate a
/// noisy child promptly.
pub(crate) fn spawn_limited_reader<R>(
    reader: R,
    limit: usize,
    overflow: Arc<AtomicBool>,
) -> (ReaderThread, Receiver<io::Result<LimitedOutput>>)
where
    R: OutputPipe,
{
    let (sender, receiver) = mpsc::sync_channel(1);
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let handle = thread::spawn(move || {
        let result = read_limited(reader, limit, &overflow, &thread_stop);
        let _ = sender.send(result);
    });
    (ReaderThread { handle, stop }, receiver)
}

/// Finish a reader without waiting longer than the independent reader
/// deadline.  When a descendant keeps the pipe open past the deadline the
/// reader is asked to stop; on Unix it observes the request within one poll
/// interval, drops (closes) the read end and is joined, so repeated timeouts
/// do not accumulate threads or descriptors (#312). Other platforms cannot
/// interrupt a blocking pipe read and still detach the bounded reader.
pub(crate) fn collect_limited_reader(
    reader: ReaderThread,
    receiver: Receiver<io::Result<LimitedOutput>>,
    deadline: Duration,
    stream: &str,
) -> io::Result<LimitedOutput> {
    let ReaderThread { handle, stop } = reader;
    match receiver.recv_timeout(deadline) {
        Ok(result) => {
            if handle.join().is_err() {
                return Err(io::Error::other(format!("{stream} reader thread panicked")));
            }
            result
        }
        Err(RecvTimeoutError::Timeout) => {
            stop.store(true, Ordering::Release);
            if cfg!(unix) {
                let _ = handle.join();
            } else {
                drop(handle);
            }
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

/// Wait until the pipe is readable (data, EOF or error) or a stop request is
/// observed. A subsequent `read` therefore never blocks indefinitely.
#[cfg(unix)]
fn wait_readable(reader: &impl std::os::fd::AsFd, stop: &AtomicBool) -> io::Result<bool> {
    use std::os::fd::AsRawFd;
    loop {
        if stop.load(Ordering::Acquire) {
            return Ok(false);
        }
        let mut poll = libc::pollfd {
            fd: reader.as_fd().as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one valid pollfd for the borrowed descriptor.
        let ready = unsafe { libc::poll(&mut poll, 1, STOP_POLL_MS) };
        if ready > 0 {
            return Ok(true);
        }
        if ready < 0 {
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }
}

#[cfg(not(unix))]
fn wait_readable<R>(_reader: &R, _stop: &AtomicBool) -> io::Result<bool> {
    Ok(true)
}

fn read_limited<R: OutputPipe>(
    mut reader: R,
    limit: usize,
    overflow: &AtomicBool,
    stop: &AtomicBool,
) -> io::Result<LimitedOutput> {
    let mut output = Vec::with_capacity(limit.min(64 * 1024));
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        if !wait_readable(&reader, stop)? {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "reader stopped after its deadline",
            ));
        }
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

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use super::*;
    #[cfg(unix)]
    use std::process::{Command, Stdio};

    #[cfg(target_os = "linux")]
    fn count(path: &str) -> usize {
        std::fs::read_dir(path).unwrap().count()
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn timed_out_reader_is_joined_and_closes_its_pipe() {
        let collect = || {
            // The shell exits at once; its background child keeps stdout open.
            let mut child = Command::new("sh")
                .args(["-c", "sleep 3 & echo started"])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            let stdout = child.stdout.take().unwrap();
            let (reader, receiver) =
                spawn_limited_reader(stdout, 1024, Arc::new(AtomicBool::new(false)));
            child.wait().unwrap();
            let result =
                collect_limited_reader(reader, receiver, Duration::from_millis(100), "stdout");
            assert_eq!(result.unwrap_err().kind(), io::ErrorKind::TimedOut);
        };
        collect();
        let (threads, fds) = (count("/proc/self/task"), count("/proc/self/fd"));
        for _ in 0..5 {
            collect();
        }
        // A joined thread's /proc task entry can outlive pthread_join for a
        // moment while the kernel reaps it; allow a bounded settle period.
        let settle = std::time::Instant::now();
        while count("/proc/self/task") > threads && settle.elapsed() < Duration::from_secs(2) {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(count("/proc/self/task"), threads, "reader threads leaked");
        assert_eq!(count("/proc/self/fd"), fds, "pipe descriptors leaked");
    }

    #[cfg(unix)]
    #[test]
    fn reader_still_collects_complete_output_before_eof() {
        let mut child = Command::new("sh")
            .args(["-c", "printf abc; sleep 0.2; printf def"])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let (reader, receiver) =
            spawn_limited_reader(stdout, 1024, Arc::new(AtomicBool::new(false)));
        let output = collect_limited_reader(reader, receiver, Duration::from_secs(5), "stdout")
            .expect("output");
        child.wait().unwrap();
        assert_eq!(output.bytes, b"abcdef");
        assert!(!output.truncated);
    }
}
