//! Explicit authenticated Storage exercise with native owner stores and provider reopen.
#[cfg(target_os = "linux")]
#[path = "storage/exercise.rs"]
mod exercise;
#[cfg(target_os = "linux")]
#[path = "peer/node.rs"]
mod node;
#[cfg(target_os = "linux")]
#[path = "storage/owners.rs"]
mod owners;
#[cfg(all(test, target_os = "linux"))]
#[path = "storage/process.rs"]
mod process;

const OVERVIEW: &str = "Usage: cargo run --locked --example storage -- [run | help run]\n\nRun paid Storage over authenticated loopback with protected journals and provider reopen.\nDetailed help: cargo run --locked --example storage -- help run\n";
const DETAILS: &str = "Usage: cargo run --locked --example storage -- run\n\nCreates fresh temporary native stores and two authenticated loopback nodes. Buys a 60-second lease for 5000 bytes, reopens the original provider, then retrieves, checks, renews and removes that lease. The exercise uses finite demo balances and one copy. The temporary root is removed after shutdown. Requires Linux and comparable uptime within the same boot. RUST_LOG controls diagnostics on stderr. Exit status 0 means help/success, 1 means runtime failure and 2 means invalid arguments.\n";

#[derive(Debug, PartialEq, Eq)]
enum Action {
    Overview,
    Details,
    Run,
}

// Parse local commands before creating stores, obtaining keys or opening listeners.
fn parse(args: &[String]) -> Result<Action, &'static str> {
    tracing::trace!("Parsing the Storage exercise command");
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] | ["--help" | "-h" | "help"] | ["help", "--help" | "-h"] => Ok(Action::Overview),
        ["help", "run"] => Ok(Action::Details),
        ["run"] => Ok(Action::Run),
        _ => Err("Invalid arguments. Use --help or help run."),
    }
}

// Report only bounded dependency categories instead of printing values, keys or file contents.
#[cfg(target_os = "linux")]
fn failure(error: impl std::fmt::Debug) -> String {
    tracing::trace!("Reporting a Storage exercise dependency failure");
    format!("Storage exercise failed: {error:?}")
}

// Run only after explicit local authorization and report completion on stdout.
#[cfg(target_os = "linux")]
fn run() -> Result<(), String> {
    tracing::trace!("Running the explicit authenticated Storage exercise");
    talksphere::composition::logging::initialize().map_err(str::to_owned)?;
    exercise::run()?;
    println!("Storage: stored, reopened provider, retrieved, checked, renewed and removed");
    Ok(())
}

// No unprotected fallback exists on other platforms.
#[cfg(not(target_os = "linux"))]
fn run() -> Result<(), String> {
    Err("The durable Storage example requires Linux.".into())
}

// Map argument errors and runtime failures to the documented exit statuses.
fn main() {
    tracing::trace!("Entering the Storage exercise command");
    match parse(&std::env::args().skip(1).collect::<Vec<_>>()) {
        Ok(Action::Overview) => print!("{OVERVIEW}"),
        Ok(Action::Details) => print!("{DETAILS}"),
        Ok(Action::Run) => {
            if let Err(error) = run() {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Help and exact run syntax are discoverable and malformed calls cannot acquire resources.
    #[test]
    fn help_and_action_syntax_are_explicit() {
        for (args, expected) in [
            (vec![], Action::Overview),
            (vec!["--help"], Action::Overview),
            (vec!["help", "run"], Action::Details),
            (vec!["run"], Action::Run),
        ] {
            assert_eq!(
                parse(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()),
                Ok(expected)
            );
        }
        assert!(parse(&["run".into(), "unexpected".into()]).is_err());
        assert!(DETAILS.contains("RUST_LOG"));
    }

    // Protected authenticated endpoints use native byte, coordinator and accounting journals.
    #[cfg(target_os = "linux")]
    #[test]
    fn paid_storage_survives_provider_reopen() {
        exercise::run().unwrap();
    }

    // Authenticate to a separate provider process and reopen its original native journals on restart.
    #[cfg(target_os = "linux")]
    #[test]
    fn paid_storage_survives_provider_process_restart() {
        exercise::run_with(true).unwrap();
    }
}
