//! Explicit authenticated Messaging exercise: sender, recipient and two replica providers over protected identities.
#[cfg(target_os = "linux")]
#[path = "messaging/exercise.rs"]
mod exercise;
#[cfg(target_os = "linux")]
#[path = "peer/node.rs"]
mod node;
#[cfg(target_os = "linux")]
#[path = "messaging/owners.rs"]
mod owners;

const OVERVIEW: &str = "Usage: cargo run --locked --example messaging -- [run | help run]\n\nRun paid Messaging with replicas over authenticated loopback and protected identities.\nDetailed help: cargo run --locked --example messaging -- help run\n";
const DETAILS: &str = "Usage: cargo run --locked --example messaging -- run\n\nCreates fresh temporary native stores and four authenticated loopback nodes, each with its own protected identity: a sender, a recipient and two providers. The sender encrypts one message to the recipient's published key, signs it and stores a replica at each provider at its exact quoted price. The recipient discovers it at both, recovers it with a signed request, decrypts it and receives it once although both providers hold it. The sender then reads the recovery receipt, runs one paid byte check, renews a placement and removes an unread second message, which releases credits exactly once. The exercise uses finite demo balances. The temporary root is removed after shutdown. Requires Linux. RUST_LOG controls diagnostics on stderr. Exit status 0 means help/success, 1 means runtime failure and 2 means invalid arguments.\n";

#[derive(Debug, PartialEq, Eq)]
enum Action {
    Overview,
    Details,
    Run,
}

// Parse local commands before creating stores, obtaining keys or opening listeners.
fn parse(args: &[String]) -> Result<Action, &'static str> {
    tracing::trace!("Parsing the Messaging exercise command");
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
    tracing::trace!("Reporting a Messaging exercise dependency failure");
    format!("Messaging exercise failed: {error:?}")
}

// Run only after explicit local authorization and report completion on stdout.
#[cfg(target_os = "linux")]
fn run() -> Result<(), String> {
    tracing::trace!("Running the explicit authenticated Messaging exercise");
    talksphere::composition::logging::initialize().map_err(str::to_owned)?;
    exercise::run()?;
    println!(
        "Messaging: stored two replicas, recovered once, checked, renewed and removed with release credits"
    );
    Ok(())
}

// No unprotected fallback exists on other platforms.
#[cfg(not(target_os = "linux"))]
fn run() -> Result<(), String> {
    Err("The durable Messaging example requires Linux.".into())
}

// Map argument errors and runtime failures to the documented exit statuses.
fn main() {
    tracing::trace!("Entering the Messaging exercise command");
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
        for args in [vec!["run", "extra"], vec!["help", "other"], vec!["--bogus"]] {
            assert!(parse(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err());
        }
    }

    // The exercise itself needs real sockets and files, so it is driven by one explicit test on Linux.
    #[cfg(target_os = "linux")]
    #[test]
    fn four_authenticated_nodes_exchange_a_replicated_message() {
        exercise::run().unwrap();
    }
}
