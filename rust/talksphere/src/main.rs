use std::process::ExitCode;

#[path = "argumentParsing/mod.rs"]
mod argument_parsing;

use argument_parsing::Arguments;

const SUCCESS_EXIT_CODE: u8 = 0;
const FAILURE_EXIT_CODE: u8 = 1;

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .init();
    tracing::trace!(">main(): parsing and executing the requested TalkSphere command");

    let arguments = Arguments::parse_from_environment();
    if let Err(command_error) = arguments.command.execute() {
        tracing::error!(%command_error, "TalkSphere could not complete the requested command");
        tracing::trace!("<main(): returning failure after command execution failed");
        return ExitCode::from(FAILURE_EXIT_CODE);
    }

    tracing::trace!("<main(): returning success after command execution completed");
    ExitCode::from(SUCCESS_EXIT_CODE)
}
