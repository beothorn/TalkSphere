//! Exact CLI argument forms; parser errors never start protocol work.
use super::help::{self, Topic};

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Command {
    Help(Topic),
    Version,
    ServeConfigured(super::server_args::ServerArgs),
    Ledger {
        path: std::path::PathBuf,
        local: crate::modules::identity::api::IdentityId,
        account: crate::modules::identity::api::IdentityId,
    },
    Board {
        path: std::path::PathBuf,
        publisher: Option<crate::modules::identity::api::IdentityId>,
    },
    Identity {
        path: std::path::PathBuf,
        create: bool,
    },
}

// Match supported forms and reject unknown or extra arguments with a usage error.
pub(super) fn parse(arguments: &[String]) -> Result<Command, String> {
    tracing::trace!(function = "args::parse", "Parsing CLI command forms");
    match arguments {
        [command, flag] if command == "ledger-show" && matches!(flag.as_str(), "--help" | "-h") => {
            Ok(Command::Help(Topic::Ledger))
        }
        [command, store, path, identity, local, account_flag, account]
            if command == "ledger-show"
                && store == "--ledger-store"
                && !path.is_empty()
                && identity == "--local-identity"
                && account_flag == "--account" =>
        {
            let local = crate::modules::identity::text::parse(local)
                .map_err(|_| "--local-identity requires a valid CR2SE ID".to_owned())?;
            let account = crate::modules::identity::text::parse(account)
                .map_err(|_| "--account requires a valid CR2SE ID".to_owned())?;
            Ok(Command::Ledger {
                path: path.into(),
                local,
                account,
            })
        }

        [command, option]
            if matches!(command.as_str(), "board-validate" | "board-show")
                && matches!(option.as_str(), "--help" | "-h") =>
        {
            Ok(Command::Help(Topic::Board))
        }
        [command, option, path]
            if command == "board-validate" && option == "--board-file" && !path.is_empty() =>
        {
            Ok(Command::Board {
                path: path.into(),
                publisher: None,
            })
        }
        [command, option, path, id_option, id]
            if command == "board-show"
                && option == "--board-store"
                && !path.is_empty()
                && id_option == "--publisher" =>
        {
            let publisher = crate::modules::identity::text::parse(id)
                .map_err(|_| "--publisher requires a valid CR2SE ID".to_owned())?;
            Ok(Command::Board {
                path: path.into(),
                publisher: Some(publisher),
            })
        }
        [command, option]
            if matches!(command.as_str(), "identity-init" | "identity-show")
                && matches!(option.as_str(), "--help" | "-h") =>
        {
            Ok(Command::Help(Topic::Identity))
        }
        [command, option, path]
            if matches!(command.as_str(), "identity-init" | "identity-show")
                && option == "--identity-store"
                && !path.is_empty() =>
        {
            Ok(Command::Identity {
                path: path.into(),
                create: command == "identity-init",
            })
        }
        [command, tail @ ..] if command == "serve" && tail.len() > 1 => {
            super::server_args::parse(tail).map(Command::ServeConfigured)
        }
        [] => Ok(Command::Help(Topic::Overview)),
        [argument] => match argument.as_str() {
            "help" | "--help" | "-h" => Ok(Command::Help(Topic::Overview)),
            "--version" | "-V" => Ok(Command::Version),
            "serve" => super::server_args::parse(&[]).map(Command::ServeConfigured),
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
        assert!(parse_words(&["serve"]).is_err());
    }

    #[test]
    fn both_version_flags_select_version_output() {
        for flag in ["-V", "--version"] {
            assert_eq!(parse_words(&[flag]), Ok(Command::Version));
        }
    }

    // Administration requires an explicit nonempty directory and never accepts implicit creation syntax.
    #[test]
    fn identity_commands_require_explicit_store() {
        for command in ["identity-init", "identity-show"] {
            assert!(
                matches!(parse_words(&[command,"--identity-store","fixture"]),Ok(Command::Identity{create,..}) if create==(command=="identity-init"))
            );
            for tail in [
                vec![],
                vec!["--identity-store"],
                vec!["--identity-store", ""],
            ] {
                let mut words = vec![command];
                words.extend(tail);
                assert!(parse_words(&words).is_err());
            }
            assert_eq!(
                parse_words(&[command, "--help"]),
                Ok(Command::Help(Topic::Identity))
            );
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
