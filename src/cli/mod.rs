//! Command-line entry point. Protocol behavior belongs to `modules`.

mod args;
mod help;

use std::{ffi::OsString, process::ExitCode};

use crate::modules::transport;
use args::Command;

/// Run the CLI with arguments excluding the executable name.
pub fn run(arguments: impl IntoIterator<Item = OsString>) -> ExitCode {
    tracing::trace!(function = "cli::run", "Dispatching command-line input");
    let arguments: Result<Vec<String>, _> = arguments
        .into_iter()
        .map(|argument| argument.into_string())
        .collect();

    let command = match arguments {
        Ok(arguments) => args::parse(&arguments),
        Err(_) => Err("arguments must be valid UTF-8".to_owned()),
    };

    match command {
        Ok(Command::Help(topic)) => {
            print!("{}", help::render(topic));
            ExitCode::SUCCESS
        }
        Ok(Command::Version) => {
            println!("talksphere {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Ok(Command::Serve) => match transport::start() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                tracing::error!(%error, "Server startup failed; exiting without a listener");
                eprintln!("error: {error}\nTry 'talksphere help serve' for details.");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            eprintln!("error: {error}\nTry 'talksphere --help' for usage.");
            ExitCode::from(2)
        }
    }
}
