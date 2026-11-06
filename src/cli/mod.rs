//! Command-line entry point. Protocol behavior belongs to `modules`.

mod args;
mod help;
mod identity;
mod serve;
mod server_args;

use std::{ffi::OsString, process::ExitCode};

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

    if matches!(
        &command,
        Ok(Command::ServeConfigured(_) | Command::Identity { .. })
    ) && let Err(error) = crate::composition::logging::initialize()
    {
        eprintln!("error: {error}");
        return ExitCode::FAILURE;
    }

    match command {
        Ok(Command::Help(topic)) => {
            print!("{}", help::render(topic));
            ExitCode::SUCCESS
        }
        Ok(Command::Version) => {
            println!("talksphere {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Ok(Command::Identity { path, create }) => match identity::execute(&path, create) {
            Ok(public) => {
                println!("{public}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                tracing::error!(%error,"Identity operation failed; exiting without replacement");
                ExitCode::FAILURE
            }
        },
        Ok(Command::ServeConfigured(args)) => match serve::execute(&args) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                tracing::error!(%error,"Server failed; releasing resources and exiting");
                ExitCode::FAILURE
            }
        },

        Err(error) => {
            eprintln!("error: {error}\nTry 'talksphere --help' for usage.");
            ExitCode::from(2)
        }
    }
}
