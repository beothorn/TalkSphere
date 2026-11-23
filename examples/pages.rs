//! Explicit authenticated hosted pages exercise: a publisher that never accepts connections, two hosts and a reader over protected identities.
#[cfg(target_os = "linux")]
#[path = "pages/exercise.rs"]
mod exercise;
#[cfg(target_os = "linux")]
#[path = "peer/node.rs"]
mod node;
#[cfg(target_os = "linux")]
#[path = "pages/owners.rs"]
mod owners;

const OVERVIEW: &str = "Usage: cargo run --locked --example pages -- [run | help run]\n\nRun paid hosted pages with two hosts over authenticated loopback and protected identities.\nDetailed help: cargo run --locked --example pages -- help run\n";
const DETAILS: &str = "Usage: cargo run --locked --example pages -- run\n\nCreates fresh temporary native stores and four authenticated loopback nodes, each with its own protected identity: two hosts, a publisher that only dials out (like a phone that cannot accept connections) and a reader. The publisher signs one public page snapshot and stores the same signed publications at both hosts at their exact quoted prices. While the publisher is offline the reader lists, fetches and verifies the page, then again from one host alone. The publisher then stores a newer page encrypted to the reader's published key at one host: the reader opens it, a reader without the key learns nothing, and across both hosts the reader takes the best-known (never proven latest) version. The publisher renews a placement, runs one paid byte check and removes the old page, which releases credits exactly once. The exercise uses finite demo balances. The temporary root is removed after shutdown. Requires Linux. RUST_LOG controls diagnostics on stderr. Exit status 0 means help/success, 1 means runtime failure and 2 means invalid arguments.\n";

#[derive(Debug, PartialEq, Eq)]
enum Action {
    Overview,
    Details,
    Run,
}

// Parse local commands before creating stores, obtaining keys or opening listeners.
fn parse(args: &[String]) -> Result<Action, &'static str> {
    tracing::trace!("Parsing the hosted pages exercise command");
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
    tracing::trace!("Reporting a hosted pages exercise dependency failure");
    format!("Hosted pages exercise failed: {error:?}")
}

// Run only after explicit local authorization and report completion on stdout.
#[cfg(target_os = "linux")]
fn run() -> Result<(), String> {
    tracing::trace!("Running the explicit authenticated hosted pages exercise");
    talksphere::composition::logging::initialize().map_err(str::to_owned)?;
    exercise::run()?;
    println!(
        "Hosted pages: stored a signed page at two hosts, read it with the publisher offline, opened a restricted newer page, renewed, checked and removed with release credits"
    );
    Ok(())
}

// No unprotected fallback exists on other platforms.
#[cfg(not(target_os = "linux"))]
fn run() -> Result<(), String> {
    Err("The hosted pages example requires Linux.".into())
}

// Map argument errors and runtime failures to the documented exit statuses.
fn main() {
    tracing::trace!("Entering the hosted pages exercise command");
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

    // Commands are parsed locally: no arguments and help forms show the overview, `help run` the details, `run` runs and anything else is invalid.
    #[test]
    fn commands_are_parsed_before_any_work() {
        let args = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        for list in [
            &[][..],
            &["--help"],
            &["-h"],
            &["help"],
            &["help", "--help"],
        ] {
            assert_eq!(parse(&args(list)), Ok(Action::Overview));
        }
        assert_eq!(parse(&args(&["help", "run"])), Ok(Action::Details));
        assert_eq!(parse(&args(&["run"])), Ok(Action::Run));
        for list in [&["run", "now"][..], &["help", "x"], &["start"]] {
            assert!(parse(&args(list)).is_err());
        }
    }
}
