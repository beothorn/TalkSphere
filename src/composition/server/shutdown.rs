//! Controlled admission stop and finite drain; OS signal delivery is handled by a separate adapter thread.
use super::{Failure, Stage, pump::Signal, transport::Transport};
use crate::modules::transport::{
    TransportError,
    api::{Accepted, Event, Request, ShutdownReason, TransportControl},
};
use std::{thread::JoinHandle, time::Instant};
impl TransportControl for Transport {
    /// Reject new admission while stopping, preserving terminal stream/control operations.
    fn submit(&mut self, request: Request) -> Result<Accepted, TransportError> {
        tracing::trace!("Submitting server-owned Transport control");
        if self.stage == Stage::Stopping
            && matches!(
                &request,
                Request::Connect(_)
                    | Request::Listen { .. }
                    | Request::OpenStream { .. }
                    | Request::Accept { .. }
            )
        {
            return Err(TransportError::Closed);
        }
        let accepted = self.driver.submit(request)?;
        if let Accepted::Connection(handle) = &accepted
            && !self.connections.contains(handle)
        {
            self.connections.push(handle.clone());
        }
        Ok(accepted)
    }
    /// Forward driver polling while retaining the server lifecycle and configuration.
    fn poll_event(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Event>> {
        tracing::trace!("Polling server-owned Transport observations");
        self.driver.poll_event(cx)
    }
}
impl Transport {
    /// Retain incoming local control and release completed connection handles from lifecycle observations.
    /// # Errors
    /// Unexpected listener failure while Ready returns Network; stopping listener closure is normal.
    pub fn observe(&mut self, event: &Event) -> Result<(), Failure> {
        tracing::trace!("Updating server-owned connection lifecycle");
        match event {
            Event::IncomingConnection(handle) => {
                if self.stage == Stage::Stopping {
                    let _ = self.driver.submit(Request::Close {
                        connection: handle.clone(),
                    });
                } else {
                    if !self.connections.contains(handle) {
                        self.connections.push(handle.clone());
                    }
                }
            }
            Event::ConnectionClosed { connection, .. } => {
                self.connections.retain(|handle| handle.id() != connection)
            }
            Event::ListenerClosed { .. } if self.stage == Stage::Ready => {
                return Err(Failure::Network);
            }
            _ => {}
        }
        Ok(())
    }
    /// Stop admission and begin draining established connections; incomplete handshakes are closed.
    /// # Errors
    /// Invalid lifecycle or an unrepresentable deadline returns Runtime.
    pub fn begin_shutdown(&mut self, now: Instant) -> Result<(), Failure> {
        tracing::trace!("Stopping admission and beginning bounded server drain");
        if self.stage == Stage::Stopping {
            return Ok(());
        }
        if self.stage != Stage::Ready {
            return Err(Failure::Runtime);
        }
        self.shutdown_deadline = Some(
            now.checked_add(self.config.options().shutdown_timeout)
                .ok_or(Failure::Runtime)?,
        );
        self.stage = Stage::Stopping;
        if let Some(listener) = self.listener.take() {
            let _ = self.driver.submit(Request::StopListening { listener });
        }
        for connection in &self.connections {
            if self
                .driver
                .submit(Request::Drain {
                    connection: connection.clone(),
                    reason: ShutdownReason::Normal,
                })
                .is_err()
            {
                let _ = self.driver.submit(Request::Close {
                    connection: connection.clone(),
                });
            }
        }
        Ok(())
    }
    /// Report completed draining or force-close on repeated signal/deadline expiry.
    /// The caller must drop the driver before reporting Stopped and releasing the identity lock.
    /// # Errors
    /// Returns ShutdownTimeout for forced closure and Runtime when shutdown has not begun.
    pub fn shutdown_progress(&mut self, now: Instant, force: bool) -> Result<bool, Failure> {
        tracing::trace!("Checking server graceful shutdown progress");
        let deadline = self.shutdown_deadline.ok_or(Failure::Runtime)?;
        if self.connections.is_empty() {
            return Ok(true);
        }
        if force || now >= deadline {
            for connection in &self.connections {
                let _ = self.driver.submit(Request::Close {
                    connection: connection.clone(),
                });
            }
            return Err(Failure::ShutdownTimeout);
        }
        Ok(false)
    }
}
/// Owned signal forwarding thread; dropping it closes the iterator and joins the worker.
pub struct Signals {
    handle: signal_hook::iterator::Handle,
    worker: Option<JoinHandle<()>>,
}
impl Signals {
    /// Forward SIGINT/SIGTERM through a safe signal iterator into local runtime notification.
    /// # Errors
    /// Signal registration or helper-thread failure returns Runtime before listener startup.
    pub fn install(control: Signal) -> Result<Self, Failure> {
        tracing::trace!("Installing controlled process termination forwarding");
        let mut signals = signal_hook::iterator::Signals::new([
            signal_hook::consts::SIGINT,
            signal_hook::consts::SIGTERM,
        ])
        .map_err(|_| Failure::Runtime)?;
        let handle = signals.handle();
        let worker = std::thread::Builder::new()
            .name("server-signals".into())
            .spawn(move || {
                for _ in signals.forever() {
                    control.shutdown();
                }
            })
            .map_err(|_| Failure::Runtime)?;
        Ok(Self {
            handle,
            worker: Some(worker),
        })
    }
}
impl Drop for Signals {
    // Closing the iterator wakes its worker; no blocking operation runs in the OS signal handler.
    fn drop(&mut self) {
        tracing::trace!("Releasing server signal forwarding");
        self.handle.close();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
