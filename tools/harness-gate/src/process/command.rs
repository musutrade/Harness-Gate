use std::process::{Child, Command, ExitStatus};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
pub(super) fn isolate_process_tree(command: &mut Command) {
    use std::os::unix::process::CommandExt;

    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
}

#[cfg(not(unix))]
pub(super) fn isolate_process_tree(_command: &mut Command) {
    // Windows termination uses `taskkill /T` below to cover descendants. The
    // command itself has no portable process-group primitive to configure here.
}

#[cfg(unix)]
pub(super) fn terminate(child: &mut Child) -> std::io::Result<ExitStatus> {
    let process_group = -(child.id() as i32);
    // The child is its process-group leader, so this also stops spawned test/build processes.
    unsafe {
        libc::kill(process_group, libc::SIGTERM);
    }
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        thread::sleep(Duration::from_millis(50));
    }
    unsafe {
        libc::kill(process_group, libc::SIGKILL);
    }
    child.wait()
}

#[cfg(not(unix))]
pub(super) fn terminate(child: &mut Child) -> std::io::Result<ExitStatus> {
    #[cfg(windows)]
    {
        let pid = child.id().to_string();
        let tree_status = Command::new("taskkill")
            .args(["/PID", &pid, "/T", "/F"])
            .status();
        if !tree_status.is_ok_and(|status| status.success()) {
            let _ = child.kill();
        }
    }
    #[cfg(not(windows))]
    child.kill()?;
    child.wait()
}
