//! Command-line parsing for the TalkSphere executable.

use std::io;

use clap::{Parser, Subcommand};

mod run;
mod version;

use run::RunArguments;
use version::VersionArguments;

/// The complete command line accepted by TalkSphere.
#[derive(Debug, Parser)]
#[command(version, about = "Talk to another CR2SE peer")]
pub struct Arguments {
    /// Selects the operation TalkSphere should perform.
    #[command(subcommand)]
    pub command: TalkSphereCommand,
}

/// Operations exposed by the TalkSphere executable.
#[derive(Debug, Subcommand)]
pub enum TalkSphereCommand {
    /// Listens locally, connects to another local peer, and exchanges a greeting.
    Run(RunArguments),

    /// Prints the installed TalkSphere version.
    Version(VersionArguments),
}

impl Arguments {
    /// Parses and validates arguments supplied by the operating system.
    ///
    /// Clap prints actionable help for missing commands, invalid ports, and
    /// `--help`, then terminates with the conventional command-line exit code.
    pub fn parse_from_environment() -> Self {
        tracing::trace!(">Arguments::parse_from_environment(): parsing command-line arguments");

        let arguments = Self::parse();

        tracing::trace!("<Arguments::parse_from_environment(): returning validated arguments");
        arguments
    }
}

impl TalkSphereCommand {
    /// Delegates execution to the module responsible for the selected command.
    pub fn execute(self) -> io::Result<()> {
        tracing::trace!(">TalkSphereCommand::execute(): dispatching the selected command");

        let execution_result = match self {
            Self::Run(run_arguments) => run_arguments.execute(),
            Self::Version(version_arguments) => version_arguments.execute(),
        };

        tracing::trace!("<TalkSphereCommand::execute(): returning the command execution result");
        execution_result
    }
}
