#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Topic {
    Overview,
    Help,
    Version,
    Serve,
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
        topic: Topic::Serve,
        names: &["serve"],
        label: "serve",
        summary: "Start the server (currently a scaffold).",
        details: "Usage: talksphere serve\n\
                  Help:  talksphere help serve | talksphere serve --help\n\n\
                  Transport is not implemented yet. This command opens no sockets,\n\
                  creates no identity or runtime state, and exits with status 1.\n\
                  Implementation begins in plans/PLAN.md step 2. There are no server options yet.\n",
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

pub(super) fn find_topic(name: &str) -> Option<Topic> {
    TOPICS
        .iter()
        .find(|entry| entry.names.contains(&name))
        .map(|entry| entry.topic)
}

pub(super) fn render(topic: Topic) -> String {
    if let Some(entry) = TOPICS.iter().find(|entry| entry.topic == topic) {
        return format!("{}\n\n{}", entry.summary, entry.details);
    }

    let mut overview = format!(
        "TalkSphere {} - Rust server and proof of concept for CR2SE\n\
         Protocol: https://github.com/beothorn/CR2SE\n\
         Status: setup only; Transport is not implemented.\n\n\
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
        assert!(overview.contains("Transport is not implemented"));
        for entry in TOPICS {
            assert!(overview.contains(entry.label));
            assert!(overview.contains(entry.summary));
        }
    }

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
