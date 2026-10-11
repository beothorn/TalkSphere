use super::help::{self, Topic};

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Command {
    Help(Topic),
    Version,
    Serve,
}

pub(super) fn parse(arguments: &[String]) -> Result<Command, String> {
    match arguments {
        [] => Ok(Command::Help(Topic::Overview)),
        [argument] => match argument.as_str() {
            "help" | "--help" | "-h" => Ok(Command::Help(Topic::Overview)),
            "--version" | "-V" => Ok(Command::Version),
            "serve" => Ok(Command::Serve),
            _ => Err(format!("unknown command or option {argument:?}")),
        },
        [command, topic] if command == "help" => help::find_topic(topic)
            .map(Command::Help)
            .ok_or_else(|| format!("unknown help topic {topic:?}")),
        [command, option] if command == "serve" && matches!(option.as_str(), "--help" | "-h") => {
            Ok(Command::Help(Topic::Serve))
        }
        _ => Err(format!("unsupported argument combination {arguments:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_words(words: &[&str]) -> Result<Command, String> {
        parse(
            &words
                .iter()
                .map(|word| (*word).to_owned())
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn no_arguments_and_help_aliases_show_overview() {
        for words in [&[][..], &["help"], &["--help"], &["-h"]] {
            assert_eq!(parse_words(words), Ok(Command::Help(Topic::Overview)));
        }
    }

    #[test]
    fn every_documented_topic_and_alias_has_detailed_help() {
        for entry in help::TOPICS {
            for name in entry.names {
                assert_eq!(
                    parse_words(&["help", name]),
                    Ok(Command::Help(entry.topic)),
                    "help topic {name}"
                );
            }
        }
    }

    #[test]
    fn command_help_does_not_request_server_startup() {
        for flag in ["-h", "--help"] {
            assert_eq!(
                parse_words(&["serve", flag]),
                Ok(Command::Help(Topic::Serve))
            );
        }
        assert_eq!(parse_words(&["serve"]), Ok(Command::Serve));
    }

    #[test]
    fn both_version_flags_select_version_output() {
        for flag in ["-V", "--version"] {
            assert_eq!(parse_words(&[flag]), Ok(Command::Version));
        }
    }

    #[test]
    fn unknown_and_extra_arguments_are_rejected() {
        for words in [
            vec!["unknown"],
            vec!["--listen", "127.0.0.1:8000"],
            vec!["serve", "extra"],
            vec!["help", "unknown"],
            vec!["help", "serve", "extra"],
            vec!["--version", "extra"],
            vec!["--help", "extra"],
            vec![""],
        ] {
            assert!(parse_words(&words).is_err(), "accepted {words:?}");
        }
    }
}
