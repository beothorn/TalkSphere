use std::{env, process::ExitCode};

fn main() -> ExitCode {
    talksphere::cli::run(env::args_os().skip(1))
}
