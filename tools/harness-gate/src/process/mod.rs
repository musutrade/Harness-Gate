mod capture;
mod command;
mod signal;
mod task;
#[cfg(test)]
mod tests;

pub use capture::{capture, capture_cleanup, CapturedOutput};
pub use signal::{cancelled, install_signal_handlers};
pub use task::{Task, TaskResult};
