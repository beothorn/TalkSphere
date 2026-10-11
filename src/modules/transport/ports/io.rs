//! Nonblocking polling interfaces using the standard task/waker contract.
//!
//! `Pending` must arrange a wakeup when progress is possible. Calls must not
//! block the connection driver. A single owner drives each socket, listener,
//! connection attempt and clock; adapters may use an executor internally.

use std::{
    io,
    net::SocketAddr,
    task::{Context, Poll},
    time::Instant,
};

pub trait Socket: Send {
    /// Return the bytes read; zero means EOF when `buffer` is nonempty.
    fn poll_read(&mut self, cx: &mut Context<'_>, buffer: &mut [u8]) -> Poll<io::Result<usize>>;
    /// Partial writes are valid; zero progress with nonempty input is a failure.
    fn poll_write(&mut self, cx: &mut Context<'_>, bytes: &[u8]) -> Poll<io::Result<usize>>;
    /// Stop both directions without waiting for peer activity; Drop also closes.
    fn close(&mut self) -> io::Result<()>;
}

/// One outgoing attempt; drop cancels it and a successful poll consumes its socket.
pub trait Connecting: Send {
    fn poll_connected(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<Box<dyn Socket>>>;
}

pub trait Listener: Send {
    /// Includes the selected port when the local bind request used port zero.
    fn local_addr(&self) -> io::Result<SocketAddr>;
    fn poll_accept(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<Box<dyn Socket>>>;
    /// Stop accepting; Drop must also release the listener.
    fn close(&mut self) -> io::Result<()>;
}

pub trait SocketProvider: Send {
    /// Initiate an attempt without waiting for it to connect.
    fn connect(&mut self, endpoint: SocketAddr) -> io::Result<Box<dyn Connecting>>;
    /// Bind a local listener; address validation and admission precede this call.
    fn listen(&mut self, endpoint: SocketAddr) -> io::Result<Box<dyn Listener>>;
}

/// A monotonic time domain local to one driver, never wall-clock or wire time.
pub trait Clock: Send {
    fn now(&self) -> Instant;
    /// Register the driver's next deadline, replacing its earlier registration.
    /// Return Ready only when the deadline has been reached. The driver must
    /// select its earliest pending timer and use checked deadline arithmetic.
    fn poll_deadline(&mut self, cx: &mut Context<'_>, deadline: Instant) -> Poll<()>;
}
