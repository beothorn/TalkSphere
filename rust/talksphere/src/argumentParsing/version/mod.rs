//! Execution of the `version` command.

use std::io;

use clap::Args;

/// Arguments and behavior belonging exclusively to the `version` command.
#[derive(Debug, Args)]
pub struct VersionArguments {}

impl VersionArguments {
    /// Prints the package version embedded by Cargo at compile time.
    pub fn execute(self) -> io::Result<()> {
        tracing::trace!(">VersionArguments::execute(): printing the TalkSphere version");

        println!("talksphere {}", env!("CARGO_PKG_VERSION"));

        tracing::trace!("<VersionArguments::execute(): the TalkSphere version was printed");
        Ok(())
    }
}
