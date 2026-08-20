use std::io;
use std::net::{Ipv4Addr, TcpListener};

use tracing::{debug, trace};

use super::SocketChannel;

const LOCAL_LISTEN_ADDRESS: Ipv4Addr = Ipv4Addr::LOCALHOST;

/// A local TCP endpoint that accepts connections from another TalkSphere peer.
pub struct SocketListener {
    tcp_listener: TcpListener,
}

impl SocketListener {
    /// Binds a listener to the loopback interface and the supplied port.
    ///
    /// Restricting this early implementation to loopback prevents it from
    /// unintentionally exposing the unfinished protocol to other machines.
    ///
    /// # Errors
    ///
    /// Returns an [`io::Error`] when the port cannot be bound.
    pub fn new(client_port: u16) -> io::Result<Self> {
        trace!(">SocketListener::new(): binding the local peer listener");
        debug!(client_port, "Binding local peer listener");

        let tcp_listener = match TcpListener::bind((LOCAL_LISTEN_ADDRESS, client_port)) {
            Ok(tcp_listener) => tcp_listener,
            Err(binding_error) => {
                trace!("<SocketListener::new(): returning a listener binding error");
                return Err(binding_error);
            }
        };

        trace!("<SocketListener::new(): returning the bound peer listener");
        Ok(Self { tcp_listener })
    }

    /// Waits for one peer connection and wraps it in a [`SocketChannel`].
    ///
    /// # Errors
    ///
    /// Returns an [`io::Error`] when the operating system cannot accept the
    /// incoming connection.
    pub fn accept(&self) -> io::Result<SocketChannel> {
        trace!(">SocketListener::accept(): waiting for a peer connection");

        let (tcp_stream, peer_address) = match self.tcp_listener.accept() {
            Ok(connection) => connection,
            Err(accept_error) => {
                trace!("<SocketListener::accept(): returning a connection acceptance error");
                return Err(accept_error);
            }
        };
        debug!(%peer_address, "Accepted peer connection");

        trace!("<SocketListener::accept(): returning the accepted socket channel");
        Ok(SocketChannel::from_tcp_stream(tcp_stream))
    }
}
