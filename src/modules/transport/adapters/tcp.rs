//! Nonblocking outgoing/incoming TCP sockets using Mio readiness task wakeups.
use super::{SystemClock, reactor::Reactor};
use crate::modules::transport::ports::io::{Connecting, Listener, Socket, SocketProvider};
use mio::{
    Interest,
    net::{TcpListener, TcpStream},
};
use std::{
    io::{self, Read, Write},
    net::{Shutdown, SocketAddr},
    task::{Context, Poll},
};

/// TCP factory with one shared readiness worker, independent of protocol authentication.
pub struct TcpProvider {
    reactor: Reactor,
}
impl TcpProvider {
    /// Start the provider's readiness worker without binding any network endpoint.
    /// # Errors
    /// Propagates OS readiness or worker startup failures.
    pub fn new() -> io::Result<Self> {
        tracing::trace!(
            function = "TcpProvider::new",
            "Creating TCP readiness services"
        );
        Ok(Self {
            reactor: Reactor::new()?,
        })
    }
    /// Create a monotonic driver clock sharing this provider's readiness worker.
    /// # Errors
    /// Fails if the process-local readiness token space is exhausted.
    pub fn clock(&self) -> io::Result<SystemClock> {
        tracing::trace!(
            function = "TcpProvider::clock",
            "Sharing TCP readiness with driver timers"
        );
        SystemClock::shared(self.reactor.clone())
    }
}
impl SocketProvider for TcpProvider {
    /// Start a nonblocking connect; dropping the attempt closes its socket immediately.
    fn connect(&mut self, endpoint: SocketAddr) -> io::Result<Box<dyn Connecting>> {
        tracing::trace!(
            function = "TcpProvider::connect",
            "Starting nonblocking TCP connection"
        );
        Ok(Box::new(Attempt(Some(TcpSocket::new(
            TcpStream::connect(endpoint)?,
            self.reactor.clone(),
        )?))))
    }
    /// Bind a listener, including port zero for OS-assigned ephemeral ports.
    fn listen(&mut self, endpoint: SocketAddr) -> io::Result<Box<dyn Listener>> {
        tracing::trace!(
            function = "TcpProvider::listen",
            "Binding a nonblocking TCP listener"
        );
        let mut socket = TcpListener::bind(endpoint)?;
        let token = self.reactor.register(&mut socket, Interest::READABLE)?;
        Ok(Box::new(TcpAcceptor {
            socket: Some(socket),
            token,
            reactor: self.reactor.clone(),
        }))
    }
}
struct TcpSocket {
    socket: Option<TcpStream>,
    token: usize,
    reactor: Reactor,
}
impl TcpSocket {
    // Register readiness before allowing connect completion or stream I/O.
    fn new(mut socket: TcpStream, reactor: Reactor) -> io::Result<Self> {
        tracing::trace!(function = "TcpSocket::new", "Registering a TCP stream");
        let token = reactor.register(&mut socket, Interest::READABLE | Interest::WRITABLE)?;
        Ok(Self {
            socket: Some(socket),
            token,
            reactor,
        })
    }
}
impl Socket for TcpSocket {
    /// Attempt one read after arming its waker; WouldBlock waits for OS readiness.
    fn poll_read(&mut self, cx: &mut Context<'_>, buffer: &mut [u8]) -> Poll<io::Result<usize>> {
        tracing::trace!(function = "TcpSocket::poll_read", "Polling TCP input");
        self.reactor.arm(self.token, false, cx.waker())?;
        let result = self
            .socket
            .as_mut()
            .ok_or(io::ErrorKind::NotConnected)?
            .read(buffer);
        pending(result)
    }
    /// Attempt one write after arming its waker, retaining short-write semantics.
    fn poll_write(&mut self, cx: &mut Context<'_>, bytes: &[u8]) -> Poll<io::Result<usize>> {
        tracing::trace!(function = "TcpSocket::poll_write", "Polling TCP output");
        self.reactor.arm(self.token, true, cx.waker())?;
        let result = self
            .socket
            .as_mut()
            .ok_or(io::ErrorKind::NotConnected)?
            .write(bytes);
        pending(result)
    }
    /// Shut down both directions and unregister readiness; repeated close is harmless.
    fn close(&mut self) -> io::Result<()> {
        tracing::trace!(
            function = "TcpSocket::close",
            "Closing TCP stream ownership"
        );
        if let Some(mut socket) = self.socket.take() {
            self.reactor.deregister(&mut socket, self.token);
            let _ = socket.shutdown(Shutdown::Both);
        }
        Ok(())
    }
}
impl Drop for TcpSocket {
    // Dropping an attempt or established stream always closes its OS handle.
    fn drop(&mut self) {
        tracing::trace!(
            function = "adapter::drop",
            "Releasing an owned platform adapter"
        );
        let _ = self.close();
    }
}
struct Attempt(Option<TcpSocket>);
impl Connecting for Attempt {
    /// Resolve the actual connect result; readiness alone never proves success.
    fn poll_connected(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<Box<dyn Socket>>> {
        tracing::trace!(
            function = "Attempt::poll_connected",
            "Checking TCP connect completion"
        );
        let socket = self.0.as_mut().ok_or(io::ErrorKind::NotConnected)?;
        socket.reactor.arm(socket.token, true, cx.waker())?;
        let stream = socket.socket.as_ref().ok_or(io::ErrorKind::NotConnected)?;
        if let Some(error) = stream.take_error()? {
            self.0 = None;
            return Poll::Ready(Err(error));
        }
        match stream.peer_addr() {
            Ok(_) => match self.0.take() {
                Some(socket) => Poll::Ready(Ok(Box::new(socket))),
                None => Poll::Ready(Err(io::ErrorKind::NotConnected.into())),
            },
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotConnected | io::ErrorKind::WouldBlock
                ) =>
            {
                Poll::Pending
            }
            Err(error) => {
                self.0 = None;
                Poll::Ready(Err(error))
            }
        }
    }
}
struct TcpAcceptor {
    socket: Option<TcpListener>,
    token: usize,
    reactor: Reactor,
}
impl Listener for TcpAcceptor {
    /// Return the bound endpoint including an OS-selected port.
    fn local_addr(&self) -> io::Result<SocketAddr> {
        tracing::trace!(
            function = "TcpAcceptor::local_addr",
            "Reading the bound TCP endpoint"
        );
        self.socket
            .as_ref()
            .ok_or(io::ErrorKind::NotConnected)?
            .local_addr()
    }
    /// Accept one ready socket without waiting for peer authentication.
    fn poll_accept(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<Box<dyn Socket>>> {
        tracing::trace!(
            function = "TcpAcceptor::poll_accept",
            "Polling incoming TCP connections"
        );
        self.reactor.arm(self.token, false, cx.waker())?;
        match self
            .socket
            .as_mut()
            .ok_or(io::ErrorKind::NotConnected)?
            .accept()
        {
            Ok((socket, _)) => Poll::Ready(
                TcpSocket::new(socket, self.reactor.clone())
                    .map(|socket| Box::new(socket) as Box<dyn Socket>),
            ),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => Poll::Pending,
            Err(error) => Poll::Ready(Err(error)),
        }
    }
    /// Release the listening socket and its retained readiness state immediately.
    fn close(&mut self) -> io::Result<()> {
        tracing::trace!(function = "TcpAcceptor::close", "Closing a TCP listener");
        if let Some(mut socket) = self.socket.take() {
            self.reactor.deregister(&mut socket, self.token);
        }
        Ok(())
    }
}
impl Drop for TcpAcceptor {
    // Listener lifetime is bounded by its owner even when stop-listening is omitted.
    fn drop(&mut self) {
        tracing::trace!(
            function = "adapter::drop",
            "Releasing an owned platform adapter"
        );
        let _ = self.close();
    }
}

// WouldBlock is a readiness wait; all other results preserve the OS byte count or failure.
fn pending<T>(result: io::Result<T>) -> Poll<io::Result<T>> {
    tracing::trace!(
        function = "tcp::pending",
        "Interpreting nonblocking I/O readiness"
    );
    match result {
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => Poll::Pending,
        other => Poll::Ready(other),
    }
}
