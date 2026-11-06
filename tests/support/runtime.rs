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
            .env("RUST_LOG", "warn")
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
    // Deliver a signal only to this owned unreaped child; already exited children need no signal.
    #[cfg(target_os = "linux")]
    pub fn signal(&mut self, signal: rustix::process::Signal) -> std::io::Result<()> {
        if self.child.try_wait()?.is_none() {
            let raw = i32::try_from(self.child.id()).map_err(std::io::Error::other)?;
            let pid = rustix::process::Pid::from_raw(raw)
                .ok_or_else(|| std::io::Error::other("invalid owned child ID"))?;
            rustix::process::kill_process(pid, signal)?;
        }
        Ok(())
    }
    // Reap a child after observed completion; fixture drop first kills unfinished children.
    pub fn finish(&mut self) -> std::io::Result<std::process::ExitStatus> {
        self.child.wait()
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
        #[cfg(target_os = "linux")]
        let _ = self.signal(rustix::process::Signal::KILL);
        #[cfg(not(target_os = "linux"))]
        let _ = self.child.kill();
        let _ = self.finish();
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
