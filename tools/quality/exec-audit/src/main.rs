//! Linux-only repository acceptance observer, never shipped with the collector.
//! Observe successful exec transitions, not syscall entry/exit register reads.
use serde_json::json;
use std::{
    collections::HashSet,
    env,
    ffi::CString,
    fs,
    io::{self, Read, Write},
    os::unix::ffi::OsStrExt,
    path::Path,
    process,
};

const LIMIT: u64 = 4 * 1024 * 1024;
fn fail(message: &str) -> io::Error {
    io::Error::other(message)
}
fn ptrace(request: libc::c_uint, pid: libc::pid_t, data: usize) -> io::Result<()> {
    // All requests here return zero on success; no PEEK request uses -1 as data.
    if unsafe {
        libc::ptrace(
            request,
            pid,
            std::ptr::null_mut::<libc::c_void>(),
            data as *mut libc::c_void,
        )
    } == -1
    {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
fn wait(pid: libc::pid_t) -> io::Result<(libc::pid_t, i32)> {
    loop {
        let mut status = 0;
        let child = unsafe { libc::waitpid(pid, &mut status, libc::__WALL) };
        if child >= 0 {
            return Ok((child, status));
        }
        let err = io::Error::last_os_error();
        if err.kind() != io::ErrorKind::Interrupted {
            return Err(err);
        }
    }
}
fn event(out: &mut fs::File, value: serde_json::Value) -> io::Result<()> {
    serde_json::to_writer(&mut *out, &value)?;
    out.write_all(b"\n")?;
    out.flush()
}
fn capture(pid: libc::pid_t) -> io::Result<serde_json::Value> {
    // At PTRACE_EVENT_EXEC the new image is installed, but no new-image user
    // instruction can run until we continue it. Threads cannot race cmdline.
    let exe = fs::read_link(format!("/proc/{pid}/exe"))?;
    let exe = exe.to_str().ok_or_else(|| fail("non-UTF8 executable"))?;
    let mut bytes = Vec::new();
    fs::File::open(format!("/proc/{pid}/cmdline"))?
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)?;
    if bytes.is_empty() || bytes.len() as u64 > LIMIT || bytes.last() != Some(&0) {
        return Err(fail("missing, oversized or incomplete exec arguments"));
    }
    let args = bytes[..bytes.len() - 1]
        .split(|b| *b == 0)
        .map(|b| std::str::from_utf8(b).map(str::to_owned))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| fail("non-UTF8 exec arguments"))?;
    Ok(json!({"event":"exec", "pid":pid, "executable":exe, "argv":args}))
}
fn run() -> io::Result<i32> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.len() < 3 || args[1] != "--" {
        return Err(fail(
            "usage: harness-gate-exec-audit NEW_LOG -- COMMAND [ARGS]",
        ));
    }
    let mut out = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(Path::new(&args[0]))?;
    let words = args[2..]
        .iter()
        .map(|x| CString::new(x.as_bytes()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| fail("NUL in command"))?;
    let mut pointers: Vec<_> = words.iter().map(|x| x.as_ptr()).collect();
    pointers.push(std::ptr::null());
    // No threads have been started. Child uses only libc until exec and never
    // returns into Rust allocation/unwinding or the parent's buffered writer.
    let root = unsafe { libc::fork() };
    if root < 0 {
        return Err(io::Error::last_os_error());
    }
    if root == 0 {
        unsafe {
            if libc::ptrace(libc::PTRACE_TRACEME, 0, 0, 0) == -1 {
                libc::_exit(125);
            }
            libc::raise(libc::SIGSTOP);
            libc::execvp(pointers[0], pointers.as_ptr());
            libc::_exit(127);
        }
    }
    let (_, status) = wait(root)?;
    if !libc::WIFSTOPPED(status) || libc::WSTOPSIG(status) != libc::SIGSTOP {
        return Err(fail("trace handshake failed"));
    }
    let options = libc::PTRACE_O_TRACEEXEC
        | libc::PTRACE_O_TRACEFORK
        | libc::PTRACE_O_TRACEVFORK
        | libc::PTRACE_O_TRACECLONE
        | libc::PTRACE_O_EXITKILL;
    if let Err(err) = ptrace(libc::PTRACE_SETOPTIONS, root, options as usize) {
        unsafe {
            libc::kill(root, libc::SIGKILL);
        }
        return Err(err);
    }
    event(
        &mut out,
        json!({"event":"start","schema":"harness-exec-events/v1","root_pid":root,"scope":"successful-exec"}),
    )?;
    ptrace(libc::PTRACE_CONT, root, 0)?;
    let mut seen = HashSet::from([root]);
    let mut exit = None;
    let mut count = 0;
    loop {
        let (pid, status) = match wait(-1) {
            Ok(value) => value,
            Err(err) if err.raw_os_error() == Some(libc::ECHILD) => break,
            Err(err) => return Err(err),
        };
        if libc::WIFEXITED(status) || libc::WIFSIGNALED(status) {
            seen.remove(&pid);
            if pid == root {
                exit = Some(if libc::WIFEXITED(status) {
                    libc::WEXITSTATUS(status)
                } else {
                    128 + libc::WTERMSIG(status)
                });
            }
            continue;
        }
        if !libc::WIFSTOPPED(status) {
            return Err(fail("unexpected trace wait state"));
        }
        let signal = libc::WSTOPSIG(status);
        let kind = status >> 16;
        let first_stop = seen.insert(pid);
        if kind == libc::PTRACE_EVENT_EXEC {
            // A nonleader exec replaces its TID with the group leader's PID.
            // Retire the former TID so a later reused PID gets its initial stop.
            let mut former_tid: libc::c_ulong = 0;
            ptrace(
                libc::PTRACE_GETEVENTMSG,
                pid,
                (&mut former_tid as *mut libc::c_ulong) as usize,
            )?;
            if former_tid != pid as libc::c_ulong {
                seen.remove(&(former_tid as libc::pid_t));
            }
            event(&mut out, capture(pid)?)?;
            count += 1;
        } else if kind != 0
            && ![
                libc::PTRACE_EVENT_FORK,
                libc::PTRACE_EVENT_VFORK,
                libc::PTRACE_EVENT_CLONE,
            ]
            .contains(&kind)
        {
            return Err(fail("unexpected ptrace event"));
        }
        let deliver = if kind != 0 || (first_stop && signal == libc::SIGSTOP) {
            0
        } else {
            signal
        };
        if let Err(err) = ptrace(libc::PTRACE_CONT, pid, deliver as usize) {
            // Another thread may have exited the group. Only wait notifications
            // finish it; no register/argument read is attempted on this path.
            if err.raw_os_error() != Some(libc::ESRCH) {
                return Err(err);
            }
        }
    }
    let code = exit.ok_or_else(|| fail("missing root exit"))?;
    if count == 0 {
        return Err(fail("no successful program execution observed"));
    }
    event(
        &mut out,
        json!({"event":"complete","exec_count":count,"root_exit_code":code}),
    )?;
    out.sync_all()?;
    Ok(code)
}
fn main() {
    match run() {
        Ok(code) => process::exit(code),
        Err(err) => {
            eprintln!("execution audit failed: {err}");
            process::exit(125);
        }
    }
}
