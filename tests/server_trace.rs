//! Capture full executable TRACE stderr independently of stdout so diagnostic volume cannot block the test.
#![cfg(target_os = "linux")]
use std::{
    fs,
    io::{BufRead, BufReader, Read},
    os::unix::fs::PermissionsExt,
    process::{Child, Command, Stdio},
};
struct ChildGuard(Child);
impl Drop for ChildGuard {
    // Always terminate and reap this owned child, including when a privacy assertion fails.
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
// Real startup/shutdown traces exclude both stored private inputs and complete protected-record representations.
#[test]
fn executable_trace_does_not_expose_protected_records() {
    let root = std::env::temp_dir().join(format!("talksphere-trace-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let path = root.join("identity");
    assert!(
        Command::new(env!("CARGO_BIN_EXE_talksphere"))
            .args(["identity-init", "--identity-store"])
            .arg(&path)
            .output()
            .unwrap()
            .status
            .success()
    );
    let mut child = ChildGuard(
        Command::new(env!("CARGO_BIN_EXE_talksphere"))
            .env("RUST_LOG", "talksphere=trace")
            .args(["serve", "--identity-store"])
            .arg(&path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let stderr = child.0.stderr.take().unwrap();
    let capture = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.take(2_097_153).read_to_end(&mut bytes).unwrap();
        assert!(bytes.len() <= 2_097_152, "bounded trace capture overflow");
        String::from_utf8(bytes).unwrap()
    });
    let mut stdout = BufReader::new(child.0.stdout.take().unwrap());
    let mut line = String::new();
    stdout.read_line(&mut line).unwrap();
    assert!(line.starts_with("ready "));
    let pid = rustix::process::Pid::from_raw(i32::try_from(child.0.id()).unwrap()).unwrap();
    rustix::process::kill_process(pid, rustix::process::Signal::TERM).unwrap();
    line.clear();
    stdout.read_line(&mut line).unwrap();
    assert_eq!(line.trim(), "stopped");
    assert!(child.0.wait().unwrap().success());
    let logs = capture.join().unwrap();
    assert!(logs.contains("TRACE"));
    assert!(!logs.contains("TSID0001"));
    let record = zeroize::Zeroizing::new(fs::read(path.join("identity.bin")).unwrap());
    for bytes in [&record[8..40], &record[40..72]] {
        let hex = zeroize::Zeroizing::new(
            bytes
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
        );
        let array = zeroize::Zeroizing::new(format!("{bytes:?}"));
        assert!(!logs.contains(hex.as_str()));
        assert!(!logs.contains(array.as_str()));
    }
    fs::remove_dir_all(root).unwrap();
}
