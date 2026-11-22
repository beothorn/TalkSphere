//! `node-ctl`: send one authorized request to a running node over its owner-only control socket.
//!
//! Parsing is effect-free; execution reads only the files the command names, sends one request and
//! prints the node's reply. Spending always needs an explicit `--max-spend`, expected bytes stay on
//! this side until a check, and retrieved bytes are written only from a validated reply.
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::PathBuf};

/// Actions with the options each accepts; required options come first.
const ACTIONS: &[(&str, &[&str], &[&str])] = &[
    ("status", &[], &[]),
    ("leases", &[], &[]),
    ("outcomes", &[], &["--unknown"]),
    ("grant", &["--peer", "--amount"], &["--request-id"]),
    ("own", &["--peer", "--amount"], &[]),
    ("trust", &["--peer", "--millionths"], &[]),
    ("balance", &["--peer"], &[]),
    ("fetch", &["--contact"], &[]),
    ("fulfil", &["--contact"], &[]),
    ("wanted-add", &["--file"], &[]),
    ("replica-add", &["--file"], &[]),
    ("replicas", &[], &[]),
    ("policy-show", &[], &[]),
    ("policy-explain", &["--peer"], &[]),
    (
        "store",
        &["--contact", "--file", "--retention-seconds", "--max-spend"],
        &[],
    ),
    (
        "retrieve",
        &["--lease", "--max-spend"],
        &["--provider", "--offset", "--length", "--output"],
    ),
    (
        "renew",
        &["--lease", "--seconds", "--max-spend"],
        &["--provider", "--expected-expiry"],
    ),
    (
        "check",
        &["--lease", "--max-spend"],
        &[
            "--provider",
            "--offset",
            "--expected-file",
            "--expected-byte",
        ],
    ),
    ("remove", &["--lease", "--max-spend"], &["--provider"]),
];

/// A validated but not yet executed control command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Ctl {
    pub root: PathBuf,
    pub action: String,
    pub options: BTreeMap<String, String>,
}

// Parse `--node-root DIR ACTION [--option VALUE]...` without any effect.
pub(super) fn parse(words: &[String]) -> Result<Ctl, String> {
    tracing::trace!("Parsing a node-ctl command");
    let [flag, root, action, rest @ ..] = words else {
        return Err("node-ctl requires --node-root DIRECTORY and an action".into());
    };
    if flag != "--node-root" || root.is_empty() {
        return Err("node-ctl requires --node-root DIRECTORY first".into());
    }
    let (_, required, optional) = ACTIONS
        .iter()
        .find(|(name, _, _)| name == action)
        .ok_or_else(|| format!("unknown node-ctl action {action:?}"))?;
    let mut options = BTreeMap::new();
    let mut words = rest.iter();
    while let Some(name) = words.next() {
        let allowed = required.contains(&name.as_str()) || optional.contains(&name.as_str());
        if !allowed {
            return Err(format!("option {name:?} is not valid for {action}"));
        }
        // --unknown is a switch; every other option needs a nonempty value.
        let value = if name == "--unknown" {
            "yes".to_string()
        } else {
            words
                .next()
                .filter(|v| !v.is_empty())
                .ok_or_else(|| format!("{name} needs a value"))?
                .clone()
        };
        if options.insert(name.clone(), value).is_some() {
            return Err(format!("{name} was given twice"));
        }
    }
    if let Some(missing) = required.iter().find(|name| !options.contains_key(**name)) {
        return Err(format!("{action} requires {missing}"));
    }
    if action == "check"
        && options.contains_key("--expected-file") == options.contains_key("--expected-byte")
    {
        return Err("check requires exactly one of --expected-file and --expected-byte".into());
    }
    if action == "check"
        && options.contains_key("--expected-byte")
        && !options.contains_key("--offset")
    {
        return Err("--expected-byte requires --offset".into());
    }
    for (name, value) in &options {
        let numeric = matches!(
            name.as_str(),
            "--amount"
                | "--millionths"
                | "--request-id"
                | "--retention-seconds"
                | "--max-spend"
                | "--offset"
                | "--length"
                | "--seconds"
                | "--expected-expiry"
        );
        if numeric && value.parse::<u64>().is_err() {
            return Err(format!("{name} must be an unsigned integer"));
        }
    }
    Ok(Ctl {
        root: root.into(),
        action: action.clone(),
        options,
    })
}

// Send the request and render the reply; the returned text is printed on success.
#[cfg(target_os = "linux")]
pub(super) fn execute(ctl: &Ctl) -> Result<String, String> {
    use crate::composition::node::{
        control::client::{Reply, call},
        layout::Layout,
    };
    tracing::trace!("Executing a node-ctl command");
    let request = build(ctl)?;
    let timeout = std::time::Duration::from_secs(3700);
    match call(&Layout::new(&ctl.root).control(), &request, timeout)? {
        Reply::Error { code, message } => Err(format!("{code}: {message}")),
        Reply::Ok(mut result) => {
            if let Some(path) = ctl.options.get("--output") {
                write_output(path, &mut result)?;
            }
            Ok(serde_json::to_string_pretty(&result)
                .map_err(|_| "reply could not be rendered".to_string())?
                + "\n")
        }
    }
}
// Unsupported platforms have no node.
#[cfg(not(target_os = "linux"))]
pub(super) fn execute(_ctl: &Ctl) -> Result<String, String> {
    tracing::trace!("Rejecting unsupported node platform");
    Err("the node is unsupported on this platform".into())
}

// Build the control request, reading local files only for store and check.
fn build(ctl: &Ctl) -> Result<Value, String> {
    tracing::trace!("Building a node-ctl request");
    let o = &ctl.options;
    let number = |name: &str| o.get(name).and_then(|v| v.parse::<u64>().ok());
    let mut request =
        json!({"id": number("--request-id").unwrap_or_else(random_id), "op": ctl.action});
    let mut set = |key: &str, value: Value| request[key] = value;
    for (flag, key) in [
        ("--peer", "peer"),
        ("--contact", "contact"),
        ("--lease", "lease"),
        ("--provider", "provider"),
    ] {
        if let Some(v) = o.get(flag) {
            set(key, json!(v));
        }
    }
    for (flag, key) in [
        ("--amount", "amount"),
        ("--millionths", "millionths"),
        ("--retention-seconds", "retentionSeconds"),
        ("--max-spend", "maximum"),
        ("--offset", "offset"),
        ("--length", "length"),
        ("--seconds", "seconds"),
        ("--expected-expiry", "expectedExpiresAt"),
    ] {
        if let Some(v) = number(flag) {
            set(key, json!(v));
        }
    }
    if o.contains_key("--unknown") {
        set("unknownOnly", json!(true));
    }
    if let Some(path) = o.get("--file") {
        let data =
            std::fs::read(path).map_err(|_| "the file to store is unreadable".to_string())?;
        set(
            "data",
            json!(crate::composition::node::control::protocol::hex(&data)),
        );
    }
    if ctl.action == "check" {
        let (offset, byte) = expected(o)?;
        set("offset", json!(offset));
        set(
            "expectedByte",
            json!(crate::composition::node::control::protocol::hex(&[byte])),
        );
    }
    Ok(request)
}

// Resolve the private expected byte locally: from the original file at a sampled or given offset, or explicit hex.
fn expected(options: &BTreeMap<String, String>) -> Result<(u64, u8), String> {
    tracing::trace!("Resolving the private expected check byte");
    if let Some(hex) = options.get("--expected-byte") {
        let byte = crate::composition::node::control::protocol::unhex(hex)
            .filter(|b| b.len() == 1)
            .ok_or("--expected-byte must be exactly one byte of lowercase hex")?[0];
        let offset = options
            .get("--offset")
            .and_then(|v| v.parse().ok())
            .ok_or("--offset is required")?;
        return Ok((offset, byte));
    }
    let path = options
        .get("--expected-file")
        .ok_or("--expected-file is required")?;
    let data = std::fs::read(path).map_err(|_| "the expected file is unreadable".to_string())?;
    if data.is_empty() {
        return Err("the expected file is empty".into());
    }
    let offset = match options.get("--offset").and_then(|v| v.parse::<u64>().ok()) {
        Some(offset) => offset,
        None => crate::modules::storage::requester::sampling::offset(
            &mut crate::modules::cryptography::random::SystemEntropy,
            data.len() as u64,
            64,
        )
        .map_err(|_| "a random offset could not be chosen".to_string())?,
    };
    let byte = *data
        .get(usize::try_from(offset).map_err(|_| "offset is too large")?)
        .ok_or("offset is outside the expected file")?;
    Ok((offset, byte))
}

// Write validated retrieved bytes to a new owner-only file and remove them from the printed reply.
#[cfg(target_os = "linux")]
fn write_output(path: &str, result: &mut Value) -> Result<(), String> {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    tracing::trace!("Writing validated retrieved bytes");
    let hex = result["data"]
        .as_str()
        .ok_or("the reply carried no data to write")?;
    let bytes = crate::composition::node::control::protocol::unhex(hex)
        .ok_or("the reply data was malformed")?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|_| "the output file could not be created; it must not exist".to_string())?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| "the output file could not be written".to_string())?;
    result["data"] = json!(format!("{} bytes written to the output file", bytes.len()));
    Ok(())
}

// A random request identifier; resending a grant needs an explicit --request-id instead.
fn random_id() -> u64 {
    tracing::trace!("Choosing a request identifier");
    let mut bytes = [0u8; 8];
    let _ = getrandom::fill(&mut bytes);
    u64::from_le_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn words(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_string).collect()
    }
    // Every action parses with its required options and rejects missing, unknown, duplicate and malformed options.
    #[test]
    fn parsing_validates_each_action() {
        let ok = parse(&words(
            "--node-root r store --contact c --file f --retention-seconds 5 --max-spend 9",
        ))
        .unwrap();
        assert_eq!(ok.action, "store");
        assert_eq!(ok.options["--max-spend"], "9");
        assert!(parse(&words("--node-root r status")).is_ok());
        // The wanted-Storage actions take exactly their one required option.
        assert!(parse(&words("--node-root r fulfil --contact c")).is_ok());
        assert!(parse(&words("--node-root r wanted-add --file f")).is_ok());
        assert!(parse(&words("--node-root r replica-add --file f")).is_ok());
        assert!(parse(&words("--node-root r replicas")).is_ok());
        assert!(parse(&words("--node-root r policy-show")).is_ok());
        assert!(parse(&words("--node-root r policy-explain --peer p")).is_ok());
        assert!(parse(&words("--node-root r policy-explain")).is_err());
        assert!(parse(&words("--node-root r policy-show --peer p")).is_err());
        assert!(parse(&words("--node-root r replica-add")).is_err());
        assert!(parse(&words("--node-root r replicas --file f")).is_err());
        assert!(parse(&words("--node-root r fulfil")).is_err());
        assert!(parse(&words("--node-root r wanted-add")).is_err());
        assert!(parse(&words("--node-root r wanted-add --file f --max-spend 1")).is_err());
        assert!(parse(&words("--node-root r outcomes --unknown")).is_ok());
        for bad in [
            "",
            "--node-root",
            "--node-root r",
            "--node-root r nope",
            "node-root r status",
            "--node-root r store --contact c --file f --retention-seconds 5",
            "--node-root r status --peer x",
            "--node-root r grant --peer p --amount -1",
            "--node-root r grant --peer p --amount 1 --amount 2",
            "--node-root r grant --peer p --amount",
            "--node-root r grant --peer p --amount 1.5",
            "--node-root r check --lease l --max-spend 1",
            "--node-root r check --lease l --max-spend 1 --expected-file f --expected-byte 00",
            "--node-root r check --lease l --max-spend 1 --expected-byte 00",
        ] {
            assert!(parse(&words(bad)).is_err(), "accepted {bad:?}");
        }
    }
    // The expected byte comes from the local file at the given offset or from explicit hex, never the node.
    #[test]
    fn expected_bytes_are_resolved_locally() {
        let dir = std::env::temp_dir().join(format!("ctl-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("original");
        std::fs::write(&file, b"abcdef").unwrap();
        let ctl = |extra: &str| {
            parse(&words(&format!(
                "--node-root r check --lease l --max-spend 1 {extra}"
            )))
            .unwrap()
        };
        let from_file = ctl(&format!("--expected-file {} --offset 2", file.display()));
        assert_eq!(expected(&from_file.options).unwrap(), (2, b'c'));
        let sampled = ctl(&format!("--expected-file {}", file.display()));
        let (offset, byte) = expected(&sampled.options).unwrap();
        assert_eq!(byte, b"abcdef"[offset as usize]);
        assert_eq!(
            expected(&ctl("--expected-byte 7a --offset 9").options).unwrap(),
            (9, 0x7a)
        );
        assert!(expected(&ctl("--expected-byte zz --offset 1").options).is_err());
        assert!(
            expected(&ctl(&format!("--expected-file {} --offset 99", file.display())).options)
                .is_err()
        );
        let request = build(&ctl(&format!(
            "--expected-file {} --offset 1",
            file.display()
        )))
        .unwrap();
        assert_eq!(request["expectedByte"], "62");
        assert_eq!(request["offset"], 1);
    }
    // Retrieved bytes are written only to a new file and replaced by a count in the printed reply.
    #[test]
    fn output_files_are_new_and_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let path = std::env::temp_dir().join(format!("ctl-out-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut reply = json!({"data": "616263"});
        write_output(path.to_str().unwrap(), &mut reply).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"abc");
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(reply["data"].as_str().unwrap().starts_with("3 bytes"));
        assert!(write_output(path.to_str().unwrap(), &mut json!({"data": "00"})).is_err());
        assert!(write_output("/nonexistent/x", &mut json!({"data": "00"})).is_err());
        assert!(write_output(path.to_str().unwrap(), &mut json!({})).is_err());
    }
}
