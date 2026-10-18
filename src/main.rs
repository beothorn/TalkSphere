//! Thin executable entry point; CLI and protocol composition live in the library.
use std::{env, process::ExitCode};

// Forward OS arguments without assuming UTF-8; the CLI chooses the exit status.
fn main() -> ExitCode {
    tracing::trace!(function = "main", "Entering the TalkSphere command line");
    talksphere::cli::run(env::args_os().skip(1))
}
