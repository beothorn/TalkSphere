//! Shared helpers for tests that run real node processes: private temporary directories, node
//! initialization, serve/stop/crash control and `node-ctl` calls.
#![allow(dead_code)]
#![cfg(target_os = "linux")]
use serde_json::Value;
use std::{
    io::{BufRead, BufReader},
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

pub const BIN: &str = env!("CARGO_BIN_EXE_talksphere");

/// A private temporary directory removed when the test ends, whatever its outcome.
pub struct Dir(pub PathBuf);
impl Dir {
    // Allocate a unique owner-only directory below the system temporary directory.
    pub fn new(name: &str) -> Self {
        use std::os::unix::fs::DirBuilderExt;
        let mut nonce = [0; 8];
        getrandom::fill(&mut nonce).unwrap();
        let path = std::env::temp_dir().join(format!("ts-{name}-{}", u64::from_le_bytes(nonce)));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .unwrap();
        Self(path)
    }
}
impl Drop for Dir {
    // Remove the whole tree; failures to clean up never mask the test result.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// One node: its root, public ID and (while running) its serve process.
pub struct Node {
    pub root: PathBuf,
    pub id: String,
    pub child: Option<Child>,
    pub lines: Arc<Mutex<Vec<String>>>,
    pub port: u16,
}
impl Node {
    // Initialize a fresh node, optionally from a configuration document.
    pub fn init(parent: &Path, name: &str, config: Option<&str>) -> Self {
        let root = parent.join(name);
        let mut args = vec![
            "node-init".to_string(),
            "--node-root".into(),
            root.display().to_string(),
        ];
        if let Some(config) = config {
            let file = parent.join(format!("{name}.json"));
            std::fs::write(&file, config).unwrap();
            args.extend(["--config".into(), file.display().to_string()]);
        }
        let out = Command::new(BIN).args(&args).output().unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        Self {
            root,
            id: String::from_utf8(out.stdout).unwrap().trim().to_string(),
            child: None,
            lines: Arc::default(),
            port: 0,
        }
    }

    // Start serving, optionally on a fixed port, and wait for the ready line.
    pub fn start(&mut self, port: Option<u16>) {
        self.start_with(port, None);
    }

    // Path of this node's captured diagnostics (stderr), appended across restarts.
    pub fn log_path(&self) -> PathBuf {
        self.root.with_extension("log")
    }

    // Start serving with an optional RUST_LOG filter; diagnostics go to a file so a chatty node never blocks.
    pub fn start_with(&mut self, port: Option<u16>, filter: Option<&str>) {
        let listen = format!("127.0.0.1:{}", port.unwrap_or(0));
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.log_path())
            .unwrap();
        let mut command = Command::new(BIN);
        command.args([
            "serve",
            "--node-root",
            self.root.to_str().unwrap(),
            "--listen",
            &listen,
        ]);
        if let Some(filter) = filter {
            command.env("RUST_LOG", filter);
        }
        let mut child = command.stdout(Stdio::piped()).stderr(log).spawn().unwrap();
        let stdout = child.stdout.take().unwrap();
        let lines = Arc::new(Mutex::new(Vec::new()));
        self.lines = lines.clone();
        let (ready, ready_rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(address) = line
                    .strip_prefix("ready ")
                    .and_then(|r| r.split(' ').nth(1))
                {
                    let _ = ready.send(address.to_string());
                }
                lines.lock().unwrap().push(line);
            }
        });
        let address = ready_rx
            .recv_timeout(Duration::from_secs(30))
            .unwrap_or_else(|_| {
                let _ = child.kill();
                let stderr = std::fs::read_to_string(self.log_path()).unwrap_or_default();
                panic!("node did not become ready: {stderr}")
            });
        self.port = address.rsplit(':').next().unwrap().parse().unwrap();
        self.child = Some(child);
    }

    // Contact text for this node at the given port (the real one unless a proxy is used).
    pub fn contact(&self, port: u16) -> String {
        format!("{}?address=127.0.0.1&port={port}", self.id)
    }

    // Stop with SIGTERM and require a clean exit and the stopped line.
    pub fn stop(&mut self) {
        let mut child = self.child.take().unwrap();
        rustix::process::kill_process(
            rustix::process::Pid::from_raw(child.id() as i32).unwrap(),
            rustix::process::Signal::TERM,
        )
        .unwrap();
        let status = child.wait().unwrap();
        assert!(status.success(), "node did not stop cleanly: {status:?}");
        std::thread::sleep(Duration::from_millis(50));
        assert!(self.lines.lock().unwrap().iter().any(|l| l == "stopped"));
    }

    // Kill without any drain, as a crash would.
    pub fn crash(&mut self) {
        let mut child = self.child.take().unwrap();
        child.kill().unwrap();
        child.wait().unwrap();
    }

    // Run one control action and return its parsed result or its error text.
    pub fn ctl(&self, args: &[&str]) -> Result<Value, String> {
        let mut command = Command::new(BIN);
        command
            .args(["node-ctl", "--node-root", self.root.to_str().unwrap()])
            .args(args);
        let out = command.output().unwrap();
        if out.status.success() {
            return Ok(serde_json::from_slice(&out.stdout).unwrap());
        }
        assert_eq!(
            out.status.code(),
            Some(1),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        Err(String::from_utf8(out.stderr).unwrap().trim().to_string())
    }
}
impl Drop for Node {
    // Never leave a node process behind after a failed assertion.
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

// Give `provider` finite credits to spend at it and mirror them on the requester, with explicit trust.
pub fn fund(provider: &Node, requester: &Node, credits: u64) {
    let amount = credits.to_string();
    provider
        .ctl(&["grant", "--peer", &requester.id, "--amount", &amount])
        .unwrap();
    provider
        .ctl(&["trust", "--peer", &requester.id, "--millionths", "1"])
        .unwrap();
    requester
        .ctl(&["own", "--peer", &provider.id, "--amount", &amount])
        .unwrap();
    requester
        .ctl(&["trust", "--peer", &provider.id, "--millionths", "1"])
        .unwrap();
}

// Reserve a port by binding then releasing it, so a restarted node can listen on the same one.
pub fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

pub fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap().to_string()
}
pub fn number(value: &Value, key: &str) -> u64 {
    value[key].as_u64().unwrap()
}
pub fn balance(node: &Node, peer: &Node) -> Value {
    node.ctl(&["balance", "--peer", &peer.id]).unwrap()
}
