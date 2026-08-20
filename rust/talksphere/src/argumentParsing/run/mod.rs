//! Execution of the `run` command.

use std::io;
use std::thread;
use std::time::Duration;

use clap::Args;
use talksphere::network::frame::{FRAME_HEADER_LENGTH, FrameHeader};
use talksphere::network::socket::{SocketChannel, SocketListener};

const CR2SE_VERSION: u8 = 1;
const HELLO_FRAME_TYPE: u8 = 2;
const NO_FRAME_FLAGS: u16 = 0;
const HELLO_STREAM_ID: u32 = 1;
const CONNECTION_RETRY_INTERVAL_SECONDS: u64 = 1;
const MAXIMUM_ACCEPTED_PAYLOAD_LENGTH: u32 = 1_048_576;

/// Arguments and behavior belonging exclusively to the `run` command.
#[derive(Debug, Args)]
pub struct RunArguments {
    /// Port on which this instance accepts its peer's connection.
    #[arg(value_name = "clientPort")]
    client_port: u16,

    /// Port on which the other instance accepts this instance's connection.
    #[arg(value_name = "serverPort")]
    server_port: u16,
}

impl RunArguments {
    /// Runs one peer until it exchanges a greeting with another local instance.
    pub fn execute(self) -> io::Result<()> {
        tracing::trace!(">RunArguments::execute(): starting both sides of peer communication");

        let socket_listener = match SocketListener::new(self.client_port) {
            Ok(socket_listener) => socket_listener,
            Err(binding_error) => {
                tracing::trace!("<RunArguments::execute(): returning a listener binding error");
                return Err(binding_error);
            }
        };
        let receiver_thread = thread::spawn(move || receive_frame(socket_listener));
        let mut socket_channel = connect_to_peer(self.server_port);
        if let Err(write_error) = send_greeting(&mut socket_channel) {
            tracing::trace!("<RunArguments::execute(): returning a greeting write error");
            return Err(write_error);
        }
        let received_payload = match receiver_thread.join() {
            Ok(receive_result) => match receive_result {
                Ok(received_payload) => received_payload,
                Err(receive_error) => {
                    tracing::trace!("<RunArguments::execute(): returning a greeting receive error");
                    return Err(receive_error);
                }
            },
            Err(_thread_panic) => {
                tracing::trace!("<RunArguments::execute(): returning a receiver thread error");
                return Err(io::Error::other("The frame receiver thread panicked"));
            }
        };

        tracing::info!(
            payload = %String::from_utf8_lossy(&received_payload),
            "Received a framed message from the peer"
        );
        tracing::trace!("<RunArguments::execute(): both peers exchanged greeting frames");
        Ok(())
    }
}

/// Keeps trying to connect because the peer may start after TalkSphere.
fn connect_to_peer(server_port: u16) -> SocketChannel {
    tracing::trace!(">connect_to_peer(): trying until the peer accepts a connection");

    loop {
        match SocketChannel::new("127.0.0.1", server_port) {
            Ok(socket_channel) => {
                tracing::trace!("<connect_to_peer(): returning the connected socket channel");
                return socket_channel;
            }
            Err(connection_error) => {
                tracing::warn!(
                    %connection_error,
                    server_port,
                    "Could not connect to the peer; retrying in one second"
                );
                thread::sleep(Duration::from_secs(CONNECTION_RETRY_INTERVAL_SECONDS));
            }
        }
    }
}

/// Sends a complete CR2SE frame containing the greeting payload.
fn send_greeting(socket_channel: &mut SocketChannel) -> io::Result<()> {
    tracing::trace!(">send_greeting(): encoding and sending the greeting frame");

    let payload = b"hello world";
    let payload_length = match u32::try_from(payload.len()) {
        Ok(payload_length) => payload_length,
        Err(length_error) => {
            tracing::trace!("<send_greeting(): returning a payload length error");
            return Err(io::Error::new(io::ErrorKind::InvalidInput, length_error));
        }
    };
    let frame_header = FrameHeader {
        version: CR2SE_VERSION,
        frame_type: HELLO_FRAME_TYPE,
        flags: NO_FRAME_FLAGS,
        stream_id: HELLO_STREAM_ID,
        payload_length,
    };

    if let Err(write_error) = socket_channel.send(&frame_header.encode()) {
        tracing::trace!("<send_greeting(): returning a frame header write error");
        return Err(write_error);
    }
    if let Err(write_error) = socket_channel.send(payload) {
        tracing::trace!("<send_greeting(): returning a frame payload write error");
        return Err(write_error);
    }

    tracing::trace!("<send_greeting(): the complete greeting frame was sent");
    Ok(())
}

/// Accepts one peer and receives one complete bounded CR2SE frame.
fn receive_frame(socket_listener: SocketListener) -> io::Result<Vec<u8>> {
    tracing::trace!(">receive_frame(): waiting for and receiving a peer frame");

    let mut socket_channel = match socket_listener.accept() {
        Ok(socket_channel) => socket_channel,
        Err(accept_error) => {
            tracing::trace!("<receive_frame(): returning a peer acceptance error");
            return Err(accept_error);
        }
    };
    let mut encoded_header = [0_u8; FRAME_HEADER_LENGTH];
    if let Err(read_error) = socket_channel.receive(&mut encoded_header) {
        tracing::trace!("<receive_frame(): returning a frame header read error");
        return Err(read_error);
    }
    let frame_header = FrameHeader::decode(encoded_header);
    if frame_header.payload_length > MAXIMUM_ACCEPTED_PAYLOAD_LENGTH {
        tracing::trace!("<receive_frame(): returning an excessive payload length error");
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "The peer declared a payload larger than the accepted maximum",
        ));
    }
    let payload_length = match usize::try_from(frame_header.payload_length) {
        Ok(payload_length) => payload_length,
        Err(length_error) => {
            tracing::trace!("<receive_frame(): returning an unsupported payload length error");
            return Err(io::Error::new(io::ErrorKind::InvalidData, length_error));
        }
    };
    let mut payload = vec![0_u8; payload_length];
    if let Err(read_error) = socket_channel.receive(&mut payload) {
        tracing::trace!("<receive_frame(): returning a frame payload read error");
        return Err(read_error);
    }

    tracing::trace!("<receive_frame(): returning the received frame payload");
    Ok(payload)
}
