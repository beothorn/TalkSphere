//! Explicit two-node Peer Protocol demonstration over real protected credentials and actual public module owners.
#[cfg(target_os = "linux")]
#[path = "peer/exercise.rs"]
mod exercise;
#[cfg(target_os = "linux")]
#[path = "services/family.rs"]
mod family;
#[cfg(target_os = "linux")]
#[path = "peer/node.rs"]
mod node;
#[cfg(target_os = "linux")]
#[path = "peer/owners.rs"]
mod owners;
const OVERVIEW: &str = "Usage: cargo run --locked --example peer -- [run | help <topic>]\n\nNo arguments show help. run creates two ephemeral loopback nodes with real protected Identity, Cryptography, Transport, Board, Services and Ledger; one explicitly installed custom text handler costs seven credits.\nService/accounting state is volatile. This is not Storage, Messaging or an operator server. No automatic retries.\nDetailed help: cargo run --locked --example peer -- help run\nHelp options: --help, -h; detailed help also accepts help --help and help -h.\nExit status: 0 help/success, 1 runtime failure, 2 invalid arguments.\n";
const DETAILS: &str = "Usage: cargo run --locked --example peer -- run\n\nCreates an owner-only random temporary directory and fresh protected identities; binds ephemeral loopback ports, pins peer IDs and explicitly registers cr2se.peer version1. Fetches direct Board/definition metadata, separately consents to price7, exchanges inline custom text and waits for normal Transport completion and both real local commitments. Prints requester/provider remaining=93. Removes its temporary stores on exit.\nDefault finite profile: 4 connections/exchanges, 2 workers, 131072 pending input bytes per stream, 1048581 queued output bytes per stream, 32000000 framing reservation bytes, 32 controls/second and 10-second exchange lifetime. Logical values are bounded to65536 bytes,4096 nodes and16 bodies; endpoint aggregate budgets are268435456 bytes/65536 bodies/4 handlers per peer.\nThe custom handler and Services/Ledger stores are volatile: every run starts from100 credits, cannot resume and never opens operator stores. Protected issuer restart and third-party settlement have separate tests in docs/peer-protocol/NODES.md. Storage and optional families are not installed; remote checks and claims are unsupported here.\nRUST_LOG controls stderr diagnostics, default info; use RUST_LOG=talksphere::modules::peer_protocol=trace. Payloads and private keys are not logged. stdout contains the deliberate result.\nExit status: 0 success/help, 1 runtime/configuration failure, 2 invalid arguments. See talksphere help peer-protocol and docs/peer-protocol/RUNTIME.md.\n";
#[derive(Debug, PartialEq, Eq)]
enum Action {
    Overview,
    Details,
    Run,
}
// Parse only explicit local demonstration actions; help never acquires keys, stores or sockets.
fn parse(args: &[String]) -> Result<Action, &'static str> {
    tracing::trace!("Parsing explicit Peer Protocol demonstration arguments");
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] | ["--help" | "-h" | "help"] | ["help", "--help" | "-h"] => Ok(Action::Overview),
        ["help", "run"] => Ok(Action::Details),
        ["run"] => Ok(Action::Run),
        _ => Err("Invalid arguments; use --help or help run."),
    }
}
// Preserve typed failure categories without dumping peer payloads, credential paths or protected state.
#[cfg(target_os = "linux")]
fn failure(error: impl std::fmt::Debug) -> String {
    tracing::trace!("Mapping a demonstration dependency failure");
    format!("Peer Protocol demonstration failed: {error:?}")
}
// Return explicit unsupported-platform failure instead of advertising an unauthenticated substitute.
#[cfg(not(target_os = "linux"))]
fn run() -> Result<(), String> {
    Err("The protected two-node example requires Linux.".into())
}
// Run only after explicit local authorization; subscriber configuration stays outside protocol internals.
#[cfg(target_os = "linux")]
fn run() -> Result<(), String> {
    tracing::trace!("Starting explicit authenticated two-node demonstration");
    talksphere::composition::logging::initialize().map_err(str::to_owned)?;
    let (requester, provider) = exercise::run()?;
    println!(
        "Peer Protocol: authenticated nodes=2, price=7, requester remaining={requester}, provider remaining={provider}"
    );
    Ok(())
}
// Help succeeds without side effects; argument and runtime failures use their separately documented statuses.
fn main() {
    tracing::trace!("Entering Peer Protocol demonstration command");
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
    // Every documented spelling is discoverable and unknown/incomplete options cannot start networking.
    #[test]
    fn help_and_action_syntax_are_explicit() {
        for (args, expected) in [
            (vec![], Action::Overview),
            (vec!["--help"], Action::Overview),
            (vec!["-h"], Action::Overview),
            (vec!["help"], Action::Overview),
            (vec!["help", "--help"], Action::Overview),
            (vec!["help", "-h"], Action::Overview),
            (vec!["help", "run"], Action::Details),
            (vec!["run"], Action::Run),
        ] {
            assert_eq!(
                parse(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()),
                Ok(expected)
            );
        }
        for args in [vec!["serve"], vec!["run", "extra"], vec!["help", "missing"]] {
            assert!(parse(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err());
        }
        assert!(DETAILS.contains("RUST_LOG"));
        assert!(OVERVIEW.contains("help run"));
    }
    // Two real authenticated endpoints commit exactly seven credits using the public peer factories and terminal gates.
    #[cfg(target_os = "linux")]
    #[test]
    fn real_two_node_example_commits_once() {
        assert_eq!(exercise::run(), Ok((93, 93)));
    }
}
