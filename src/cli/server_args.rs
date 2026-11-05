//! Server-only argument validation before filesystem or network effects.
use crate::composition::server::config::{Config, Options};
use crate::modules::{
    identity::{api::Contact, contact},
    transport::config::{MAX_FRAME_BYTES, MAX_PAYLOAD_BYTES, ResourceLimits, TransportConfig},
};
use std::{collections::BTreeSet, net::SocketAddr, path::PathBuf, time::Duration};
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ServerArgs {
    pub store: PathBuf,
    pub listen: SocketAddr,
    pub contacts: Vec<Contact>,
    pub stored_contacts: bool,
    pub connections: usize,
    pub streams: usize,
    pub outbound: usize,
    pub buffered: usize,
    pub handshake_ms: u64,
    pub shutdown_ms: u64,
}
impl ServerArgs {
    // Convert owned CLI values to the shared validated runtime policy without performing I/O.
    pub(super) fn config(&self) -> Result<Config, String> {
        tracing::trace!("Validating server command policy");
        let mut options = Options::local(self.store.clone()).map_err(|e| e.to_string())?;
        options.listen = self.listen;
        options.contacts = self.contacts.clone();
        options.shutdown_timeout = Duration::from_millis(self.shutdown_ms);
        options.transport = TransportConfig::new(
            ResourceLimits {
                connections: self.connections,
                streams_per_connection: self.streams,
                outbound_bytes: self.outbound,
                buffered_bytes_per_stream: self.buffered,
            },
            Duration::from_millis(self.handshake_ms),
        )
        .map_err(|e| e.to_string())?;
        Config::new(options).map_err(|e| format!("invalid server configuration: {e:?}"))
    }
}
// Parse a finite set of exact option/value pairs; repeated contacts are bounded and all other duplicates fail.
pub(super) fn parse(words: &[String]) -> Result<ServerArgs, String> {
    tracing::trace!("Parsing explicit server options");
    if words.len() > 160 || !words.len().is_multiple_of(2) {
        return Err("server options require bounded option/value pairs".into());
    }
    let mut args = ServerArgs {
        store: PathBuf::new(),
        listen: "127.0.0.1:0"
            .parse()
            .map_err(|_| "invalid default address")?,
        contacts: Vec::new(),
        stored_contacts: false,
        connections: 64,
        streams: 128,
        outbound: 64 * MAX_FRAME_BYTES,
        buffered: MAX_PAYLOAD_BYTES,
        handshake_ms: 10000,
        shutdown_ms: 5000,
    };
    let mut seen = BTreeSet::new();
    for pair in words.as_chunks::<2>().0 {
        let flag = pair[0].as_str();
        let value = pair[1].as_str();
        if value.is_empty() || (flag != "--contact" && !seen.insert(flag)) {
            return Err("empty or duplicate server option".into());
        }
        let invalid = || format!("invalid value for {flag}");
        match flag {
            "--identity-store" => args.store = value.into(),
            "--listen" => args.listen = value.parse().map_err(|_| invalid())?,
            "--contact" => {
                if args.contacts.len() >= 64 {
                    return Err("at most 64 contacts are supported".into());
                }
                args.contacts
                    .push(contact::parse(value, 1024).map_err(|_| invalid())?);
            }
            "--stored-contacts" => {
                args.stored_contacts = match value {
                    "yes" => true,
                    "no" => false,
                    _ => return Err(invalid()),
                }
            }
            "--max-connections" => args.connections = value.parse().map_err(|_| invalid())?,
            "--max-streams" => args.streams = value.parse().map_err(|_| invalid())?,
            "--outbound-bytes" => args.outbound = value.parse().map_err(|_| invalid())?,
            "--buffered-bytes" => args.buffered = value.parse().map_err(|_| invalid())?,
            "--handshake-ms" => args.handshake_ms = value.parse().map_err(|_| invalid())?,
            "--shutdown-ms" => args.shutdown_ms = value.parse().map_err(|_| invalid())?,
            _ => return Err(format!("unknown server option {flag:?}")),
        }
    }
    if args.stored_contacts && !args.contacts.is_empty() {
        return Err("choose explicit or stored contacts, not both".into());
    }
    args.config()?;
    Ok(args)
}
#[cfg(test)]
mod tests {
    // Finite defaults and explicit port zero are valid; all policy errors fail before resource acquisition.
    #[test]
    fn server_options_validate_without_effects() {
        let parse =
            |words: &[&str]| super::parse(&words.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        let args = parse(&["--identity-store", "fixture"]).unwrap();
        assert_eq!(args.listen.port(), 0);
        assert!(args.config().is_ok());
        for words in [
            vec![],
            vec!["--identity-store"],
            vec!["--identity-store", ""],
            vec!["--identity-store", "fixture", "--shutdown-ms", "0"],
            vec!["--identity-store", "fixture", "--handshake-ms", "0"],
            vec!["--identity-store", "fixture", "--max-connections", "0"],
            vec!["--identity-store", "fixture", "--identity-store", "other"],
            vec!["--identity-store", "fixture", "--listen", "hostname:42"],
            vec!["--identity-store", "fixture", "--stored-contacts", "maybe"],
        ] {
            assert!(parse(&words).is_err());
        }
    }
}
