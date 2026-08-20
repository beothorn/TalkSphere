use std::io::Read;
use std::net::{Ipv4Addr, TcpListener};
use std::thread;

use talksphere::network::socket::SocketChannel;

const LOOPBACK_ADDRESS: Ipv4Addr = Ipv4Addr::LOCALHOST;
const OPERATING_SYSTEM_SELECTED_PORT: u16 = 0;

#[test]
fn channel_connects_and_sends_bytes() {
    tracing::trace!(">channel_connects_and_sends_bytes(): starting a local test server");

    let tcp_listener =
        TcpListener::bind((LOOPBACK_ADDRESS, OPERATING_SYSTEM_SELECTED_PORT)).unwrap();
    let server_port = tcp_listener.local_addr().unwrap().port();
    let bytes = b"already encoded frame bytes";
    let server_thread = thread::spawn(move || {
        let (mut tcp_stream, _peer_address) = tcp_listener.accept().unwrap();
        let mut received_bytes = [0_u8; b"already encoded frame bytes".len()];
        tcp_stream.read_exact(&mut received_bytes).unwrap();
        received_bytes
    });

    let mut socket_channel =
        SocketChannel::new(&LOOPBACK_ADDRESS.to_string(), server_port).unwrap();
    socket_channel.send(bytes).unwrap();

    assert_eq!(socket_channel.peer_address().unwrap().port(), server_port);
    assert_eq!(server_thread.join().unwrap(), *bytes);

    tracing::trace!("<channel_connects_and_sends_bytes(): the server received every byte");
}

#[test]
fn constructor_returns_an_error_when_the_connection_is_refused() {
    tracing::trace!(
        ">constructor_returns_an_error_when_the_connection_is_refused(): reserving a closed port"
    );

    let tcp_listener =
        TcpListener::bind((LOOPBACK_ADDRESS, OPERATING_SYSTEM_SELECTED_PORT)).unwrap();
    let closed_server_port = tcp_listener.local_addr().unwrap().port();
    drop(tcp_listener);

    let connection_result = SocketChannel::new(&LOOPBACK_ADDRESS.to_string(), closed_server_port);

    assert!(connection_result.is_err());

    tracing::trace!(
        "<constructor_returns_an_error_when_the_connection_is_refused(): the connection error was returned"
    );
}
