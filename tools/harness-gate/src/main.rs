mod app;
mod audit;
mod cli;
mod config;
mod doctor;
mod error;
mod preset;
mod process;
mod project;
mod scope;
mod secrets;
mod service;
#[cfg(test)]
mod test_support;
mod utils;
mod verify;

use error::CodedError;
use std::process::ExitCode;

fn main() -> ExitCode {
    process::install_signal_handlers();
    match app::run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("ERROR [{}]: {error}", error.code());
            ExitCode::FAILURE
        }
    }
}
