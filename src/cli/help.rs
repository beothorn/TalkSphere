//! Shared concise overview and discoverable per-command/option help definitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Topic {
    Overview,
    Help,
    Version,
    Serve,
    Identity,
    ServerOptions,
    Logging,
}

pub(super) struct HelpTopic {
    pub topic: Topic,
    pub names: &'static [&'static str],
    label: &'static str,
    summary: &'static str,
    details: &'static str,
}

// The overview and detailed help share these definitions. Every new command
// or option needs a topic here and corresponding parser/behavior tests.
pub(super) const TOPICS: &[HelpTopic] = &[
    HelpTopic {
        topic: Topic::Logging,
        names: &["logging", "RUST_LOG"],
        label: "RUST_LOG",
        summary: "Filter structured operational logs on stderr.",
        details: "Usage: RUST_LOG=info talksphere serve --identity-store DIRECTORY\nRUST_LOG defaults to info; off disables logs; target filters such as talksphere=trace enable diagnosis.\nOperational logs use stderr; stdout contains intentional public CLI results. Never include secrets in diagnostics.\nHelp/version ignore logging configuration and exit with status 0; malformed filters prevent runtime startup with status 1.\n",
    },
    HelpTopic {
        topic: Topic::ServerOptions,
        names: &[
            "--listen",
            "--contact",
            "--stored-contacts",
            "--max-connections",
            "--max-streams",
            "--outbound-bytes",
            "--buffered-bytes",
            "--handshake-ms",
            "--shutdown-ms",
        ],
        label: "serve options",
        summary: "Configure listening, contacts, limits and shutdown.",
        details: "Usage: talksphere serve --identity-store DIRECTORY [OPTION VALUE]...\n\n--listen IP:PORT defaults to 127.0.0.1:0; zero selects an assigned listening port.\n--contact CONTACT repeats up to 64 times; requires a canonical ID with an IP/port, no Discovery.\n--stored-contacts yes|no defaults to no; yes selects the protected snapshot instead of explicit contacts.\n--max-connections COUNT defaults to 64; --max-streams COUNT defaults to 128 per connection.\n--outbound-bytes BYTES defaults to 4196096; --buffered-bytes BYTES defaults to 65536 per stream.\n--handshake-ms MILLISECONDS defaults to 10000; --shutdown-ms MILLISECONDS defaults to 5000.\nBudgets must support maximum protocol frames, durations must be positive; duplicate non-contact options are invalid.\nOptions are validated before I/O; invalid arguments exit with status 2. SIGINT/SIGTERM stop admission and drain to --shutdown-ms; a repeated signal forces closure.\n",
    },
    HelpTopic {
        topic: Topic::Identity,
        names: &["identity-init", "identity-show", "--identity-store"],
        label: "identity-init | identity-show",
        summary: "Create or inspect a protected local identity.",
        details: "Usage: talksphere identity-init --identity-store DIRECTORY | talksphere identity-show --identity-store DIRECTORY\n\nExplicit initialization installs both key pairs without overwrite; inspection never generates keys.\n--identity-store requires an explicit owner-only Linux directory; parents must exist.\nThe same --identity-store option is required by serve, which only loads existing keys.\nOutput from identity administration is only the public canonical CR2SE ID. Files are permission-protected, not encrypted.\nMissing/corrupt/insecure stores and conflicting initialization exit with status 1; invalid syntax exits with status 2.\nHelp exits with status 0. See plans/4_Transport_integration/PLAN.md.\n",
    },
    HelpTopic {
        topic: Topic::Serve,
        names: &["serve"],
        label: "serve",
        summary: "Run authenticated Transport connections (Linux).",
        details: "Usage: talksphere serve --identity-store DIRECTORY [OPTION VALUE]...\n\nRequesting help opens no sockets and exits with status 0.\nLoads existing protected keys; missing or inconsistent credentials never generate replacements.\nReports ready <PUBLIC_ID> <ACTUAL_ADDRESS> and verified authenticated peer IDs.\nFrame payloads remain visible; application encryption and services are separate modules.\nProduction registers no application protocol; unsupported OPEN requests are refused.\nUse talksphere help --listen for contacts, defaults, limits and timeouts.\nSIGINT/SIGTERM stop admission and drain; repeated signals force remaining connections closed.\nInvalid syntax exits with status 2, startup/runtime failure with status 1, normal shutdown with status 0.\nSee plans/4_Transport_integration/PLAN.md and docs/transport-integration/ACCEPTANCE.md.\n",
    },
    HelpTopic {
        topic: Topic::Help,
        names: &["help", "--help", "-h"],
        label: "help [TOPIC], -h, --help",
        summary: "Show the overview or detailed topic help.",
        details: "Usage: talksphere --help | talksphere -h | talksphere help [TOPIC]\n\n\
                  With no arguments, or with --help, -h or help alone, show the overview.\n\
                  Use 'talksphere help <topic>' for details on any command or option.\n\
                  Topics accept option spellings: 'talksphere help --version' and\n\
                  'talksphere help version' show the same details.\n\
                  Use 'talksphere serve --help' for command-specific help.\n\
                  Help exits with status 0 and does not start the server. Unknown topics\n\
                  and unsupported argument combinations exit with status 2.\n",
    },
    HelpTopic {
        topic: Topic::Version,
        names: &["version", "--version", "-V"],
        label: "-V, --version",
        summary: "Print the TalkSphere application version.",
        details: "Usage: talksphere --version | talksphere -V\n\n\
                  Print 'talksphere <version>' from the Cargo package metadata and exit\n\
                  with status 0. This is the application version, not a CR2SE protocol\n\
                  version or a claim of conformance. The server is not started.\n\
                  This option takes no value and must be used on its own.\n",
    },
];

// Resolve documented aliases without guessing unsupported options.
pub(super) fn find_topic(name: &str) -> Option<Topic> {
    tracing::trace!(function = "help::find_topic", "Resolving a help topic");
    TOPICS
        .iter()
        .find(|entry| entry.names.contains(&name))
        .map(|entry| entry.topic)
}

// Render the shared overview or full topic with usage and exit behavior.
pub(super) fn render(topic: Topic) -> String {
    tracing::trace!(function = "help::render", "Rendering discoverable CLI help");
    if let Some(entry) = TOPICS.iter().find(|entry| entry.topic == topic) {
        return format!("{}\n\n{}", entry.summary, entry.details);
    }

    let mut overview = format!(
        "TalkSphere {} - Rust server and proof of concept for CR2SE\n\
         Protocol: https://github.com/beothorn/CR2SE\n\
         Status: authenticated Transport server available on Linux; application services pending.\n\n\
         Usage: talksphere [COMMAND | OPTION]\n\n\
         Commands and options:\n",
        env!("CARGO_PKG_VERSION")
    );
    for entry in TOPICS {
        overview.push_str(&format!("  {:<28} {}\n", entry.label, entry.summary));
    }
    let names: Vec<_> = TOPICS.iter().map(|entry| entry.names[0]).collect();
    overview.push_str(&format!(
        "\nDetailed help: talksphere help <topic>\n\
         Topics: {} (option spellings also work).\n\
         Examples: talksphere help serve | talksphere help --version\n",
        names.join(", ")
    ));
    overview
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overview_lists_all_topics_and_how_to_get_details() {
        let overview = render(Topic::Overview);
        assert!(overview.contains("talksphere help <topic>"));
        assert!(overview.contains(
            "authenticated Transport server available on Linux; application services pending"
        ));
        for entry in TOPICS {
            assert!(overview.contains(entry.label));
            assert!(overview.contains(entry.summary));
        }
    }

    // Serve help distinguishes configured authenticated startup from a missing identity selection.
    #[test]
    fn serve_help_explains_protected_startup_and_integration_evidence() {
        let details = render(Topic::Serve);
        assert!(details.contains("existing protected keys"));
        assert!(details.contains("plans/4_Transport_integration/PLAN.md"));
        assert!(details.contains("ACCEPTANCE.md"));
        assert!(details.contains("never generate replacements"));
        assert!(details.contains("unsupported OPEN"));
    }

    // Every detailed topic preserves discoverable usage and exit behavior.
    #[test]
    fn detailed_topics_explain_usage_and_exit_behavior() {
        for entry in TOPICS {
            let details = render(entry.topic);
            assert!(details.contains("Usage:"));
            assert!(details.contains("status"));
            assert!(details.len() > entry.summary.len());
        }
    }
}
