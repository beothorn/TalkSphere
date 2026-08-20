//! TCP connection management for communication with a single peer.

mod socket_channel;
mod socket_listener;

pub use socket_channel::SocketChannel;
pub use socket_listener::SocketListener;
