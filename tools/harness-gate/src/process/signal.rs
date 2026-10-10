use std::sync::atomic::{AtomicBool, Ordering};

static CANCELLED: AtomicBool = AtomicBool::new(false);

#[cfg(unix)]
extern "C" fn handle_signal(_signal: libc::c_int) {
    CANCELLED.store(true, Ordering::Relaxed);
}

#[cfg(windows)]
unsafe extern "system" fn handle_console_signal(signal: u32) -> i32 {
    // CTRL_C_EVENT, CTRL_BREAK_EVENT, CTRL_CLOSE_EVENT, CTRL_LOGOFF_EVENT,
    // and CTRL_SHUTDOWN_EVENT all require the same bounded cleanup path.
    if matches!(signal, 0..=2 | 5..=6) {
        CANCELLED.store(true, Ordering::Relaxed);
        1
    } else {
        0
    }
}

pub fn install_signal_handlers() {
    #[cfg(unix)]
    unsafe {
        // SAFETY: the handler only performs an atomic store, which is async-signal-safe.
        let handler = handle_signal as *const () as libc::sighandler_t;
        libc::signal(libc::SIGINT, handler);
        libc::signal(libc::SIGTERM, handler);
    }

    #[cfg(windows)]
    unsafe {
        type HandlerRoutine = Option<unsafe extern "system" fn(u32) -> i32>;
        #[link(name = "Kernel32")]
        extern "system" {
            fn SetConsoleCtrlHandler(handler: HandlerRoutine, add: i32) -> i32;
        }

        // SAFETY: the callback has the ABI and lifetime required by the Windows API,
        // and it only updates the process cancellation flag.
        let _ = SetConsoleCtrlHandler(Some(handle_console_signal), 1);
    }
}

pub fn cancelled() -> bool {
    CANCELLED.load(Ordering::Relaxed)
}
