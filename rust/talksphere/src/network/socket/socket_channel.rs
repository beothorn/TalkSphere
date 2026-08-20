use std::io::{self, Read, Write};
use std::net::TcpStream;

use tracing::{debug, trace};

/// A persistent TCP connection used to exchange bytes with one peer.
///
/// `SocketChannel` owns its [`TcpStream`]. Rust therefore closes the operating
/// system socket automatically when the channel goes out of scope and is
/// dropped.
pub struct SocketChannel {
    tcp_stream: TcpStream,
}

impl SocketChannel {
    /// Wraps an already connected stream accepted by [`super::SocketListener`].
    pub(super) fn from_tcp_stream(tcp_stream: TcpStream) -> Self {
        trace!(">SocketChannel::from_tcp_stream(): wrapping an accepted TCP stream");

        let socket_channel = Self { tcp_stream };

        trace!("<SocketChannel::from_tcp_stream(): returning the accepted socket channel");
        socket_channel
    }

    /// Connects to a peer.
    ///
    /// The supplied port is passed directly to [`TcpStream::connect`], so the
    /// caller decides which service endpoint to use.
    ///
    /// # Errors
    ///
    /// Returns an [`io::Error`] when the connection cannot be established.
    pub fn new(server_address: &str, server_port: u16) -> io::Result<Self> {
        trace!(">SocketChannel::new(): connecting to a peer");
        debug!(server_address, server_port, "Connecting to peer");

        let tcp_stream = match TcpStream::connect((server_address, server_port)) {
            Ok(tcp_stream) => tcp_stream,
            Err(connection_error) => {
                trace!("<SocketChannel::new(): returning a TCP connection error");
                return Err(connection_error);
            }
        };

        trace!("<SocketChannel::new(): returning a connected socket channel");
        Ok(Self { tcp_stream })
    }

    /// Sends every byte in the supplied buffer to the connected peer.
    ///
    /// [`Write::write_all`] handles partial TCP writes by continuing until the
    /// complete buffer has been accepted or an error occurs. Framing remains a
    /// separate responsibility: callers pass already encoded frame bytes here.
    ///
    /// # Errors
    ///
    /// Returns an [`io::Error`] when the complete buffer cannot be written.
    pub fn send(&mut self, bytes: &[u8]) -> io::Result<()> {
        trace!(">SocketChannel::send(): sending bytes to the connected peer");
        debug!(byte_count = bytes.len(), "Sending bytes to peer");

        if let Err(write_error) = self.tcp_stream.write_all(bytes) {
            trace!("<SocketChannel::send(): returning a TCP write error");
            return Err(write_error);
        }

        trace!("<SocketChannel::send(): all bytes were sent");
        Ok(())
    }

    /// Receives exactly enough bytes to fill the supplied buffer.
    ///
    /// [`Read::read_exact`] handles partial TCP reads. A successful return means
    /// the caller owns one complete requested protocol section, such as a frame
    /// header or its declared payload.
    ///
    /// # Errors
    ///
    /// Returns an [`io::Error`] if the connection closes early or a read fails.
    pub fn receive(&mut self, bytes: &mut [u8]) -> io::Result<()> {
        trace!(">SocketChannel::receive(): receiving an exact number of bytes");
        debug!(byte_count = bytes.len(), "Receiving bytes from peer");

        if let Err(read_error) = self.tcp_stream.read_exact(bytes) {
            trace!("<SocketChannel::receive(): returning a TCP read error");
            return Err(read_error);
        }

        trace!("<SocketChannel::receive(): the requested bytes were received");
        Ok(())
    }

    /// Returns the socket address of the connected peer.
    ///
    /// This provides useful diagnostic information while keeping ownership of
    /// the underlying stream inside the channel.
    ///
    /// # Errors
    ///
    /// Returns an [`io::Error`] if the operating system cannot provide the
    /// remote address.
    pub fn peer_address(&self) -> io::Result<std::net::SocketAddr> {
        trace!(">SocketChannel::peer_address(): reading the connected peer address");

        let peer_address = match self.tcp_stream.peer_addr() {
            Ok(peer_address) => peer_address,
            Err(address_error) => {
                trace!("<SocketChannel::peer_address(): returning a peer address error");
                return Err(address_error);
            }
        };

        trace!("<SocketChannel::peer_address(): returning the connected peer address");
        Ok(peer_address)
    }
}
