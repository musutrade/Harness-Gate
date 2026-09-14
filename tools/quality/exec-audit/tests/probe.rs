//! Real subprocess/thread fixtures for the repository observer.
use std::{env, process::Command, thread};
fn main() {
    let mode = env::args().nth(1).unwrap_or_default();
    match mode.as_str() {
        "race" => {
            for _ in 0..100 {
                let pid = unsafe { libc::fork() };
                assert!(pid >= 0);
                if pid == 0 {
                    for _ in 0..6 {
                        thread::spawn(|| loop {
                            unsafe {
                                libc::getpid();
                            }
                        });
                    }
                    unsafe {
                        libc::_exit(0);
                    }
                }
                let mut status = 0;
                assert_eq!(unsafe { libc::waitpid(pid, &mut status, 0) }, pid);
                assert_eq!(status, 0);
            }
        }
        "child" => {
            assert!(Command::new("/bin/true").status().unwrap().success());
        }
        "forbidden" => {
            assert!(Command::new("python3")
                .args(["-c", "pass"])
                .status()
                .unwrap()
                .success());
        }
        "thread-exec" => {
            thread::spawn(|| {
                use std::os::unix::process::CommandExt;
                panic!("exec failed: {}", Command::new("/bin/true").exec());
            });
            loop {
                thread::park();
            }
        }
        "execveat" => {
            let path = std::ffi::CString::new("/bin/true").unwrap();
            let empty = std::ffi::CString::new("").unwrap();
            let argv = [path.as_ptr(), std::ptr::null()];
            let envp: [*const libc::c_char; 1] = [std::ptr::null()];
            unsafe {
                let fd = libc::open(path.as_ptr(), libc::O_RDONLY);
                assert!(fd >= 0);
                libc::syscall(
                    libc::SYS_execveat,
                    fd,
                    empty.as_ptr(),
                    argv.as_ptr(),
                    envp.as_ptr(),
                    libc::AT_EMPTY_PATH,
                );
            }
            panic!("execveat failed");
        }
        "exit7" => std::process::exit(7),
        _ => panic!("unknown probe"),
    }
}
