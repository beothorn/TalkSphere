//! Owner-only Unix control socket server with bounded connections and request lines.
//!
//! Callers: the node runtime, which starts it once the node is ready. The socket lives in the
//! owner-only node root with mode 0600; startup refuses wider root permissions and refuses to
//! replace a socket another node still answers on. Each connection runs on its own worker
//! thread, up to the configured limit; requests on one connection are served in order.
use super::protocol::{self, Code, ControlError, Line};
use crate::composition::{
    node::{config::Control, layout::Layout},
    server::pump::Signal,
};
use std::{
    io::Write,
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
    },
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread::JoinHandle,
    time::Duration,
};

/// Executes one parsed request; implemented by the command handler and by test doubles.
pub trait Execute: Send + Sync + 'static {
    /// Run the request and return its result or an explicit refusal.
    fn execute(&self, request: &protocol::Request) -> Result<serde_json::Value, ControlError>;
}

/// Delay between accept polls, bounding how quickly a stop request is noticed.
const ACCEPT_POLL: Duration = Duration::from_millis(20);
/// Read timeout used so idle connections notice shutdown.
const READ_POLL: Duration = Duration::from_millis(200);

/// A running control server; dropping it stops accepting, joins workers and removes the socket.
pub struct Server {
    stop: Arc<AtomicBool>,
    path: PathBuf,
    acceptor: Option<JoinHandle<()>>,
}

/// Bind the control socket and start serving.
/// # Errors
/// A node root readable by others, a live socket owned by another node, or a bind failure.
pub fn start(
    layout: &Layout,
    limits: Control,
    signal: Signal,
    handler: Arc<dyn Execute>,
) -> Result<Server, String> {
    tracing::trace!("Starting the local control socket");
    let mode = std::fs::metadata(layout.root())
        .map_err(|_| "node root is unavailable".to_string())?
        .permissions()
        .mode();
    if mode & 0o077 != 0 {
        return Err("node root permissions are wider than owner-only".into());
    }
    let path = layout.control();
    if path.exists() {
        if UnixStream::connect(&path).is_ok() {
            return Err("another node already serves this control socket".into());
        }
        std::fs::remove_file(&path)
            .map_err(|_| "stale control socket could not be removed".to_string())?;
    }
    let listener =
        UnixListener::bind(&path).map_err(|_| "control socket could not be bound".to_string())?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
        .map_err(|_| "control socket permissions could not be set".to_string())?;
    listener
        .set_nonblocking(true)
        .map_err(|_| "control socket could not be configured".to_string())?;
    let stop = Arc::new(AtomicBool::new(false));
    let acceptor = {
        let stop = stop.clone();
        std::thread::Builder::new()
            .name("control-accept".into())
            .spawn(move || accept(listener, limits, signal, handler, stop))
            .map_err(|_| "control worker could not be started".to_string())?
    };
    Ok(Server {
        stop,
        path,
        acceptor: Some(acceptor),
    })
}

impl Drop for Server {
    // Stop accepting, wait for every connection worker and remove the socket file.
    fn drop(&mut self) {
        tracing::trace!("Stopping the local control socket");
        self.stop.store(true, Ordering::SeqCst);
        if let Some(acceptor) = self.acceptor.take() {
            let _ = acceptor.join();
        }
        let _ = std::fs::remove_file(&self.path);
    }
}

// Accept connections until stopped, handing each to a bounded worker.
fn accept(
    listener: UnixListener,
    limits: Control,
    signal: Signal,
    handler: Arc<dyn Execute>,
    stop: Arc<AtomicBool>,
) {
    tracing::trace!("Accepting control connections");
    let active = Arc::new(AtomicUsize::new(0));
    let mut workers: Vec<JoinHandle<()>> = Vec::new();
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                workers.retain(|w| !w.is_finished());
                if signal.stopping() || active.load(Ordering::SeqCst) >= limits.connections {
                    refuse(stream, signal.stopping());
                    continue;
                }
                active.fetch_add(1, Ordering::SeqCst);
                let (handler, stop, active) = (handler.clone(), stop.clone(), active.clone());
                let signal = signal.clone();
                match std::thread::Builder::new()
                    .name("control-worker".into())
                    .spawn(move || {
                        serve(stream, limits, &signal, handler.as_ref(), &stop);
                        active.fetch_sub(1, Ordering::SeqCst);
                    }) {
                    Ok(worker) => workers.push(worker),
                    Err(_) => {
                        tracing::warn!("A control worker could not be started");
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(ACCEPT_POLL),
            Err(_) => std::thread::sleep(ACCEPT_POLL),
        }
    }
    for worker in workers {
        let _ = worker.join();
    }
}

// Tell an over-limit or late client why it is refused, then close.
fn refuse(mut stream: UnixStream, stopping: bool) {
    tracing::trace!("Refusing a control connection");
    let message = if stopping {
        "the node is stopping"
    } else {
        "too many control connections"
    };
    let _ = stream.write_all(&protocol::error(0, &ControlError::new(Code::Busy, message)));
}

// Serve requests on one connection until it closes, a line is oversized or the server stops.
fn serve(
    mut stream: UnixStream,
    limits: Control,
    signal: &Signal,
    handler: &dyn Execute,
    stop: &AtomicBool,
) {
    tracing::trace!("Serving one control connection");
    if stream.set_nonblocking(false).is_err() || stream.set_read_timeout(Some(READ_POLL)).is_err() {
        return;
    }
    loop {
        let mut reader = &stream;
        let line = protocol::read_line(&mut reader, limits.line_bytes, || {
            !stop.load(Ordering::SeqCst)
        });
        let bytes = match line {
            Line::Closed => return,
            Line::TooLong => {
                let _ = stream.write_all(&protocol::error(
                    0,
                    &ControlError::new(Code::Invalid, "request line is too long"),
                ));
                return;
            }
            Line::Complete(bytes) => bytes,
        };
        let reply = match protocol::parse(&bytes, limits.line_bytes) {
            Err(e) => protocol::error(0, &e),
            Ok(request) if signal.stopping() => protocol::error(
                request.id,
                &ControlError::new(Code::Busy, "the node is stopping"),
            ),
            Ok(request) => match handler.execute(&request) {
                Ok(result) => protocol::ok(request.id, result),
                Err(e) => protocol::error(request.id, &e),
            },
        };
        if stream.write_all(&reply).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    struct Echo;
    impl Execute for Echo {
        fn execute(&self, request: &protocol::Request) -> Result<serde_json::Value, ControlError> {
            match request.op.as_str() {
                "echo" => Ok(serde_json::json!({"n": request.u64("n")?})),
                "slow" => {
                    std::thread::sleep(Duration::from_millis(300));
                    Ok(serde_json::json!({}))
                }
                _ => Err(ControlError::new(Code::Unsupported, "no such operation")),
            }
        }
    }
    fn layout() -> Layout {
        use std::os::unix::fs::DirBuilderExt;
        let mut nonce = [0; 8];
        getrandom::fill(&mut nonce).unwrap();
        let path = std::env::temp_dir().join(format!("control-{}", u64::from_le_bytes(nonce)));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .unwrap();
        Layout::new(path)
    }
    fn limits() -> Control {
        Control {
            connections: 2,
            line_bytes: 256,
            request_seconds: 5,
        }
    }
    fn exchange(stream: &mut UnixStream, line: &str) -> serde_json::Value {
        stream.write_all(line.as_bytes()).unwrap();
        stream.write_all(b"\n").unwrap();
        let mut reply = String::new();
        BufReader::new(&*stream).read_line(&mut reply).unwrap();
        serde_json::from_str(&reply).unwrap()
    }
    // The socket is owner-only, answers requests in order and is removed when the server stops.
    #[test]
    fn serves_requests_with_owner_only_permissions_and_cleans_up() {
        let layout = layout();
        let server = start(&layout, limits(), Signal::default(), Arc::new(Echo)).unwrap();
        let mode = std::fs::metadata(layout.control())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        let mut stream = UnixStream::connect(layout.control()).unwrap();
        assert_eq!(
            exchange(&mut stream, r#"{"id":1,"op":"echo","n":5}"#)["result"]["n"],
            5
        );
        assert_eq!(
            exchange(&mut stream, r#"{"id":2,"op":"nope"}"#)["error"]["code"],
            "unsupported"
        );
        assert_eq!(exchange(&mut stream, "garbage")["error"]["code"], "invalid");
        drop(stream);
        drop(server);
        assert!(!layout.control().exists());
    }
    // An oversized line gets one explicit error and the connection is closed.
    #[test]
    fn oversized_lines_close_the_connection() {
        let layout = layout();
        let _server = start(&layout, limits(), Signal::default(), Arc::new(Echo)).unwrap();
        let mut stream = UnixStream::connect(layout.control()).unwrap();
        let long = format!("{{\"id\":1,\"op\":\"{}\"}}", "x".repeat(400));
        assert_eq!(
            exchange(&mut stream, &long)["error"]["message"],
            "request line is too long"
        );
        let mut rest = String::new();
        // The server closed with unread input pending, which surfaces as end of stream or a reset.
        assert_eq!(BufReader::new(&stream).read_line(&mut rest).unwrap_or(0), 0);
    }
    // Connections above the limit are refused explicitly while existing ones keep working.
    #[test]
    fn connection_limit_is_enforced() {
        let layout = layout();
        let _server = start(&layout, limits(), Signal::default(), Arc::new(Echo)).unwrap();
        let mut a = UnixStream::connect(layout.control()).unwrap();
        let mut b = UnixStream::connect(layout.control()).unwrap();
        assert_eq!(
            exchange(&mut a, r#"{"id":1,"op":"echo","n":1}"#)["ok"],
            true
        );
        assert_eq!(
            exchange(&mut b, r#"{"id":1,"op":"echo","n":2}"#)["ok"],
            true
        );
        let mut c = UnixStream::connect(layout.control()).unwrap();
        let mut reply = String::new();
        BufReader::new(&c).read_line(&mut reply).unwrap();
        assert!(reply.contains("busy") && reply.contains("too many"));
        assert_eq!(
            exchange(&mut a, r#"{"id":3,"op":"echo","n":3}"#)["ok"],
            true
        );
        let _ = c.flush();
    }
    // A client disconnect mid-request does not stop the server or other clients.
    #[test]
    fn disconnects_do_not_affect_other_clients() {
        let layout = layout();
        let _server = start(&layout, limits(), Signal::default(), Arc::new(Echo)).unwrap();
        {
            let mut gone = UnixStream::connect(layout.control()).unwrap();
            gone.write_all(b"{\"id\":1,\"op\":\"slow\"}\n").unwrap();
        }
        let mut live = UnixStream::connect(layout.control()).unwrap();
        assert_eq!(
            exchange(&mut live, r#"{"id":2,"op":"echo","n":9}"#)["result"]["n"],
            9
        );
    }
    // Wider root permissions and a socket another node serves both stop startup; a stale file is replaced.
    #[test]
    fn startup_refuses_unsafe_roots_and_live_sockets() {
        let layout = layout();
        std::fs::set_permissions(layout.root(), std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(start(&layout, limits(), Signal::default(), Arc::new(Echo)).is_err());
        std::fs::set_permissions(layout.root(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let first = start(&layout, limits(), Signal::default(), Arc::new(Echo)).unwrap();
        assert!(start(&layout, limits(), Signal::default(), Arc::new(Echo)).is_err());
        drop(first);
        std::fs::write(layout.control(), b"stale").unwrap();
        let again = start(&layout, limits(), Signal::default(), Arc::new(Echo)).unwrap();
        drop(again);
    }
    // While the node is stopping, new requests and connections are refused with a busy error.
    #[test]
    fn stopping_nodes_refuse_requests() {
        let layout = layout();
        let signal = Signal::default();
        let _server = start(&layout, limits(), signal.clone(), Arc::new(Echo)).unwrap();
        let mut stream = UnixStream::connect(layout.control()).unwrap();
        assert_eq!(
            exchange(&mut stream, r#"{"id":1,"op":"echo","n":1}"#)["ok"],
            true
        );
        signal.shutdown();
        let reply = exchange(&mut stream, r#"{"id":2,"op":"echo","n":1}"#);
        assert_eq!(reply["error"]["code"], "busy");
    }
}
