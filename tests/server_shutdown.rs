//! Real signal forwarding is isolated in its own test process, separate from runtime unit tests.
#![cfg(target_os = "linux")]
use talksphere::composition::server::{pump::Signal, shutdown::Signals};
// SIGTERM wakes local control through the iterator thread; repeated requests force closure and dropping joins it.
#[test]
fn termination_signal_is_forwarded_without_protocol_work_in_handler() {
    let control = Signal::default();
    let signals = Signals::install(control.clone()).unwrap();
    rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::TERM)
        .unwrap();
    control.wait(std::time::Duration::from_secs(5)).unwrap();
    assert!(control.stopping());
    assert!(!control.forcing());
    control.shutdown();
    assert!(control.forcing());
    drop(signals);
}
