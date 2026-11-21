//! Client for the local control socket, used by the `node-ctl` command.
//!
//! Sends one request and reads one reply line; the reply bound equals the largest object
//! plus metadata, so a misbehaving node cannot exhaust client memory.
use super::protocol::{self, Line};
use serde_json::Value;
use std::{io::Write, os::unix::net::UnixStream, path::Path, time::Duration};

/// Largest reply line a client accepts.
const MAXIMUM_REPLY_BYTES: usize = 4 * 1_048_576;

/// A decoded reply.
#[derive(Debug, Clone, PartialEq)]
pub enum Reply {
    /// Successful result.
    Ok(Value),
    /// Explicit refusal with its code and message.
    Error {
        /// Stable failure class name.
        code: String,
        /// Explanation without secrets.
        message: String,
    },
}

/// Send `request` (a JSON object with `id` and `op`) and wait up to `timeout` for the reply.
/// # Errors
/// The socket is absent or unreachable, the node closes without replying, or the reply is
/// oversized or malformed.
pub fn call(socket: &Path, request: &Value, timeout: Duration) -> Result<Reply, String> {
    tracing::trace!("Calling the local control socket");
    let mut stream = UnixStream::connect(socket)
        .map_err(|_| "the node is not running or its control socket is unreachable".to_string())?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|_| "control socket could not be configured".to_string())?;
    let mut bytes =
        serde_json::to_vec(request).map_err(|_| "request could not be encoded".to_string())?;
    bytes.push(b'\n');
    stream
        .write_all(&bytes)
        .map_err(|_| "the node closed the control connection".to_string())?;
    let mut reader = &stream;
    let deadline = std::time::Instant::now() + timeout;
    let line = protocol::read_line(&mut reader, MAXIMUM_REPLY_BYTES, || {
        std::time::Instant::now() < deadline
    });
    let Line::Complete(line) = line else {
        return Err("the node did not return a complete reply".into());
    };
    let value: Value = serde_json::from_slice(&line)
        .map_err(|_| "the node returned a malformed reply".to_string())?;
    if value["ok"].as_bool() == Some(true) {
        return Ok(Reply::Ok(value["result"].clone()));
    }
    let error = &value["error"];
    match (error["code"].as_str(), error["message"].as_str()) {
        (Some(code), Some(message)) => Ok(Reply::Error {
            code: code.to_string(),
            message: message.to_string(),
        }),
        _ => Err("the node returned a malformed reply".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;
    // Serve one canned reply on a temporary socket and return its path.
    fn serve(reply: &'static [u8]) -> std::path::PathBuf {
        let mut nonce = [0; 8];
        getrandom::fill(&mut nonce).unwrap();
        let path = std::env::temp_dir().join(format!("client-{}.sock", u64::from_le_bytes(nonce)));
        let listener = UnixListener::bind(&path).unwrap();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0u8; 256];
                let _ = std::io::Read::read(&mut stream, &mut buffer);
                let _ = stream.write_all(reply);
            }
        });
        path
    }
    // Success and error replies decode; malformed or missing replies are explicit failures.
    #[test]
    fn decodes_replies_and_reports_failures() {
        let request = serde_json::json!({"id":1,"op":"status"});
        let t = Duration::from_secs(2);
        let ok = call(
            &serve(b"{\"id\":1,\"ok\":true,\"result\":{\"a\":1}}\n"),
            &request,
            t,
        )
        .unwrap();
        assert_eq!(ok, Reply::Ok(serde_json::json!({"a":1})));
        let failed = call(
            &serve(b"{\"id\":1,\"ok\":false,\"error\":{\"code\":\"busy\",\"message\":\"m\"}}\n"),
            &request,
            t,
        )
        .unwrap();
        assert_eq!(
            failed,
            Reply::Error {
                code: "busy".into(),
                message: "m".into()
            }
        );
        assert!(call(&serve(b"nonsense\n"), &request, t).is_err());
        assert!(call(&serve(b"{\"ok\":false}\n"), &request, t).is_err());
        assert!(call(&serve(b""), &request, t).is_err());
        assert!(call(Path::new("/nonexistent/control.sock"), &request, t).is_err());
    }
}
