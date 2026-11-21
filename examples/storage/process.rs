//! Separate provider process fixture using the same protected composition and explicit readiness.
use super::{failure, node::Node, owners};
use std::{
    io::{BufRead, BufReader, Write},
    net::SocketAddr,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};
use talksphere::{
    composition::server::pump::Signal,
    modules::identity::{api::IdentityId, text},
};
const ENV: &str = "TALKSPHERE_STORAGE_PROVIDER_FIXTURE";

pub(super) struct Process {
    child: Child,
    stop: PathBuf,
    pub address: SocketAddr,
}

impl Process {
    // Launch the exact test-owned child, then wait for the actual listener readiness message.
    pub(super) fn start(
        root: &Path,
        identity: &Path,
        local: IdentityId,
        requester: IdentityId,
        create: bool,
    ) -> Result<Self, String> {
        let stop = root.with_extension("stop");
        if !create && stop.exists() {
            std::fs::remove_file(&stop).map_err(failure)?;
        }
        let config = serde_json::json!({
            "root": root, "identity": identity, "local": text::format(local),
            "requester": text::format(requester), "create": create, "stop": stop
        });
        let mut child = Command::new(std::env::current_exe().map_err(failure)?)
            .args(["--exact", "process::provider_child", "--nocapture"])
            .env(ENV, config.to_string())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(failure)?;
        let stdout = child.stdout.take().ok_or("Missing readiness pipe")?;
        let (sender, ready) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            for line in BufReader::new(stdout)
                .lines()
                .take(16)
                .map_while(Result::ok)
            {
                if let Some((_, address)) = line.split_once("STORAGE_READY ") {
                    let _ = sender.send(address.trim().parse::<SocketAddr>());
                }
            }
        });
        let address = match ready.recv_timeout(Duration::from_secs(20)) {
            Ok(Ok(addr)) => addr,
            error => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(failure(error));
            }
        };
        Ok(Self {
            child,
            stop,
            address,
        })
    }

    // Wait for normal process exit and release of all independent journal/identity locks.
    pub(super) fn stop(&mut self) -> Result<(), String> {
        std::fs::write(&self.stop, b"stop").map_err(failure)?;
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if let Some(status) = self.child.try_wait().map_err(failure)? {
                return if status.success() {
                    Ok(())
                } else {
                    Err(failure(status))
                };
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Err("Provider child did not stop".into())
    }
}
impl Drop for Process {
    // A failing test must reap its child before dropping the temporary directories.
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// The helper is inert during normal test runs and owns only its explicitly provisioned fixture stores.
#[test]
fn provider_child() {
    let Some(config) = std::env::var_os(ENV) else {
        return;
    };
    let config = config.into_string().unwrap();
    assert!(config.len() <= 4096);
    let config: serde_json::Value = serde_json::from_str(&config).unwrap();
    let field = |name: &str| config[name].as_str().unwrap();
    let local = text::parse(field("local")).unwrap();
    let requester = text::parse(field("requester")).unwrap();
    let owner = owners::open(
        Path::new(field("root")),
        local,
        requester,
        false,
        config["create"].as_bool().unwrap(),
        Arc::new(owners::Catalog(Mutex::new(None))),
    )
    .unwrap();
    let signal = Signal::default();
    let mut node = Node::new(Path::new(field("identity")), owner.endpoint, signal.clone()).unwrap();
    println!("STORAGE_READY {}", node.address);
    std::io::stdout().flush().unwrap();
    let deadline = Instant::now() + Duration::from_secs(90);
    while !Path::new(field("stop")).exists() {
        assert!(Instant::now() < deadline);
        node.poll().unwrap();
        signal.wait(Duration::from_millis(20)).unwrap();
    }
}
