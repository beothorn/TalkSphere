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

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Fault {
    None,
    InterruptedUpload,
    LostResult,
    LostCheckResult,
    SlowRetrieve,
}
impl Fault {
    // Keep the child fixture control independent of all wire fields.
    fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::InterruptedUpload => "upload",
            Self::LostResult => "result",
            Self::LostCheckResult => "check-result",
            Self::SlowRetrieve => "slow-retrieve",
        }
    }
}

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
        Self::start_with_fault(root, identity, local, requester, create, Fault::None)
    }

    // Select a controlled test-only stream cut before the child handles any real peer input.
    pub(super) fn start_with_fault(
        root: &Path,
        identity: &Path,
        local: IdentityId,
        requester: IdentityId,
        create: bool,
        fault: Fault,
    ) -> Result<Self, String> {
        let stop = root.with_extension("stop");
        if !create && stop.exists() {
            std::fs::remove_file(&stop).map_err(failure)?;
        }
        let config = serde_json::json!({
            "root": root, "identity": identity, "local": text::format(local),
            "requester": text::format(requester), "create": create, "stop": stop, "fault": fault.label()
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
    let fault = match field("fault") {
        "upload" => Fault::InterruptedUpload,
        "result" => Fault::LostResult,
        "check-result" => Fault::LostCheckResult,
        "slow-retrieve" => Fault::SlowRetrieve,
        "none" => Fault::None,
        _ => panic!("unexpected test fault"),
    };
    let mut node = Node::with_factory(
        Path::new(field("identity")),
        Arc::new(FaultFactory {
            actual: owner.endpoint,
            fault,
            root: PathBuf::from(field("root")),
        }),
        signal.clone(),
    )
    .unwrap();
    println!("STORAGE_READY {}", node.address);
    std::io::stdout().flush().unwrap();
    let deadline = Instant::now() + Duration::from_secs(90);
    while !Path::new(field("stop")).exists() {
        assert!(Instant::now() < deadline);
        node.poll().unwrap();
        signal.wait(Duration::from_millis(20)).unwrap();
    }
}

use talksphere::modules::{
    peer_protocol::{
        self as peer,
        engine::bridge::{Closure, Factory, Handler, Output, Scope},
        exchange::Direction,
        message::{Kind, Message},
        record::Record,
    },
    transport::api::StreamOutcome,
};

struct FaultFactory {
    actual: owners::Endpoint,
    fault: Fault,
    root: PathBuf,
}
impl Factory for FaultFactory {
    // Preserve actual authentication and association, then wrap just the test-controlled stream delivery.
    fn open(
        &self,
        scope: Scope,
        opener: Direction,
        output: &mut Output,
    ) -> Result<Box<dyn Handler>, peer::Error> {
        let inner = self.actual.open(scope, opener, output)?;
        Ok(Box::new(FaultStream {
            inner,
            fault: self.fault,
            store: false,
            check: false,
            retrieve: false,
            root: self.root.clone(),
        }))
    }
}
struct FaultStream {
    inner: Box<dyn Handler>,
    fault: Fault,
    store: bool,
    check: bool,
    retrieve: bool,
    root: PathBuf,
}
impl Handler for FaultStream {
    // Interrupt after receiving the first real detached body record for the paid store.
    fn record(&mut self, record: Record, output: &mut Output) -> Result<(), peer::Error> {
        match &record {
            Record::Control(bytes) => {
                let message = Message::parse(bytes, 4096)?;
                if message.kind() == Kind::Propose {
                    self.check = message.value()["mode"] == "check";
                    self.store = message.value()["input"]["operation"] == "store";
                    self.retrieve = message.value()["input"]["operation"] == "retrieve";
                }
            }
            Record::Body { .. } if self.store && self.fault == Fault::InterruptedUpload => {
                self.inner.record(record, output)?;
                return Err(peer::Error::Unknown);
            }
            _ => {}
        }
        self.inner.record(record, output)
    }

    // Drop delivery after the actual provider's durable work, before queued success is delivered to Transport.
    fn remote_end(&mut self, output: &mut Output) -> Result<(), peer::Error> {
        self.inner.remote_end(output)?;
        if self.retrieve && self.fault == Fault::SlowRetrieve {
            std::fs::write(self.root.with_extension("result-ready"), b"ready")
                .map_err(|_| peer::Error::Unknown)?;
            let deadline = Instant::now() + Duration::from_secs(5);
            while !self.root.with_extension("allow-result").exists() {
                if Instant::now() >= deadline {
                    return Err(peer::Error::Unknown);
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        if self.check && self.fault == Fault::LostCheckResult {
            std::thread::sleep(Duration::from_millis(5200));
            return Err(peer::Error::Unknown);
        }
        if self.store && self.fault == Fault::LostResult {
            return Err(peer::Error::Unknown);
        }
        Ok(())
    }

    // Keep normal flow control and bounded output paths.
    fn writable(&mut self, output: &mut Output) -> Result<(), peer::Error> {
        self.inner.writable(output)
    }

    // Preserve the true original closure time and local END observation.
    fn closed_observed(&mut self, observation: Closure) -> Result<(), peer::Error> {
        self.inner.closed_observed(observation)
    }

    // Let the real handler release or retain its original reservations.
    fn closed(&mut self, outcome: StreamOutcome) -> Result<(), peer::Error> {
        self.inner.closed(outcome)
    }
}
