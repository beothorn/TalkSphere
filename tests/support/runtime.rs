//! Isolated server process fixtures with bounded output reads and cleanup on every exit path.
use std::{
    fs,
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};
static NEXT: AtomicU64 = AtomicU64::new(0);

pub struct ProcessFixture {
    pub root: PathBuf,
    child: Child,
    lines: mpsc::Receiver<String>,
    errors: mpsc::Receiver<String>,
}
impl ProcessFixture {
    // Spawn the real executable with a private unique working directory and bounded stdout records.
    pub fn spawn(arguments: &[&str]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "talksphere-process-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir(&root).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let mut child = Command::new(env!("CARGO_BIN_EXE_talksphere"))
            .args(arguments)
            .current_dir(&root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let lines = capture(child.stdout.take().unwrap());
        let errors = capture(child.stderr.take().unwrap());
        Self {
            root,
            child,
            lines,
            errors,
        }
    }
    // Capture bounded stderr diagnostics independently so neither output pipe can fill silently.
    pub fn error(&self) -> Result<String, mpsc::RecvTimeoutError> {
        self.errors.recv_timeout(Duration::from_secs(5))
    }
    // Wait for a bounded stdout record without relying on fixed sleeps or a shared network port.
    pub fn line(&self) -> Result<String, mpsc::RecvTimeoutError> {
        self.lines.recv_timeout(Duration::from_secs(5))
    }
}
impl Drop for ProcessFixture {
    // Kill an unfinished child, reap it and remove only this fixture's owned tree.
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.root);
    }
}

// Drain one child pipe into bounded records; overflow terminates capture instead of unbounded allocation.
fn capture(output: impl std::io::Read + Send + 'static) -> mpsc::Receiver<String> {
    let (sender, lines) = mpsc::sync_channel(32);
    std::thread::spawn(move || {
        let mut reader = BufReader::new(output);
        loop {
            let mut bytes = Vec::new();
            // Bound each line independently; a malicious or broken child cannot allocate without limit.
            use std::io::Read;
            let read = reader.by_ref().take(4097).read_until(b'\n', &mut bytes);
            match read {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            if bytes.len() > 4096 {
                break;
            }
            let Ok(line) = String::from_utf8(bytes) else {
                break;
            };
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    lines
}
