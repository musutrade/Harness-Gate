use std::sync::atomic::{AtomicBool, Ordering};

static CANCELLED: AtomicBool = AtomicBool::new(false);

#[cfg(unix)]
extern "C" fn handle_signal(_signal: libc::c_int) {
    CANCELLED.store(true, Ordering::SeqCst);
}

pub fn install_signal_handlers() {
    #[cfg(unix)]
    unsafe {
        let handler = handle_signal as *const () as libc::sighandler_t;
        libc::signal(libc::SIGINT, handler);
        libc::signal(libc::SIGTERM, handler);
    }
}

pub fn cancelled() -> bool {
    CANCELLED.load(Ordering::SeqCst)
}
