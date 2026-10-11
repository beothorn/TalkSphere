//! In-memory connect/listen/accept substitutes; they never bind or connect TCP.

use std::{
    collections::VecDeque,
    io,
    net::SocketAddr,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};

use super::script::Script;
use crate::modules::transport::ports::io::{Connecting, Listener, Socket, SocketProvider};

type SocketResult = io::Result<Box<dyn Socket>>;

#[derive(Default)]
struct QueueState {
    results: VecDeque<SocketResult>,
    waker: Option<Waker>,
    closed: bool,
    single_result: bool,
}

struct SocketQueue(Arc<Mutex<QueueState>>);

#[derive(Clone)]
pub struct QueueControl(Arc<Mutex<QueueState>>);

impl SocketQueue {
    fn new(single_result: bool) -> (Self, QueueControl) {
        let state = Arc::new(Mutex::new(QueueState {
            single_result,
            ..QueueState::default()
        }));
        (Self(state.clone()), QueueControl(state))
    }

    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<SocketResult> {
        let mut state = self.0.lock().unwrap();
        if state.closed {
            return Poll::Ready(Err(io::ErrorKind::NotConnected.into()));
        }
        match state.results.pop_front() {
            Some(result) => Poll::Ready(result),
            None => {
                state.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }

    fn close(&mut self) {
        let (wake, pending) = {
            let mut state = self.0.lock().unwrap();
            state.closed = true;
            (state.waker.take(), std::mem::take(&mut state.results))
        };
        drop(pending);
        if let Some(waker) = wake {
            waker.wake();
        }
    }
}

impl Drop for SocketQueue {
    fn drop(&mut self) {
        self.close();
    }
}

impl QueueControl {
    pub fn push(&self, result: SocketResult) {
        let wake = {
            let mut state = self.0.lock().unwrap();
            if state.closed {
                drop(state);
                panic!("completion added after closure");
            }
            if state.single_result && !state.results.is_empty() {
                drop(state);
                panic!("connection attempt already has a completion");
            }
            state.results.push_back(result);
            state.waker.take()
        };
        if let Some(waker) = wake {
            waker.wake();
        }
    }

    pub fn is_closed(&self) -> bool {
        self.0.lock().unwrap().closed
    }
}

pub struct ScriptedConnecting {
    queue: SocketQueue,
    done: bool,
}

impl ScriptedConnecting {
    pub fn new() -> (Self, QueueControl) {
        let (queue, control) = SocketQueue::new(true);
        (Self { queue, done: false }, control)
    }
}

impl Connecting for ScriptedConnecting {
    fn poll_connected(&mut self, cx: &mut Context<'_>) -> Poll<SocketResult> {
        if self.done {
            return Poll::Ready(Err(io::ErrorKind::NotConnected.into()));
        }
        let result = self.queue.poll(cx);
        if result.is_ready() {
            self.done = true;
            self.queue.close();
        }
        result
    }
}

pub struct ScriptedListener {
    address: SocketAddr,
    queue: SocketQueue,
}

impl ScriptedListener {
    pub fn new(address: SocketAddr) -> (Self, QueueControl) {
        let (queue, control) = SocketQueue::new(false);
        (Self { address, queue }, control)
    }
}

impl Listener for ScriptedListener {
    fn local_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.address)
    }
    fn poll_accept(&mut self, cx: &mut Context<'_>) -> Poll<SocketResult> {
        self.queue.poll(cx)
    }
    fn close(&mut self) -> io::Result<()> {
        self.queue.close();
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkCall {
    Connect(SocketAddr),
    Listen(SocketAddr),
}

pub enum NetworkReply {
    Connect(io::Result<Box<dyn Connecting>>),
    Listen(io::Result<Box<dyn Listener>>),
}

pub struct ScriptedNetwork(pub Script<NetworkCall, NetworkReply>);

impl SocketProvider for ScriptedNetwork {
    fn connect(&mut self, endpoint: SocketAddr) -> io::Result<Box<dyn Connecting>> {
        let NetworkReply::Connect(result) = self.0.call(NetworkCall::Connect(endpoint)) else {
            panic!("expected connect reply")
        };
        result
    }
    fn listen(&mut self, endpoint: SocketAddr) -> io::Result<Box<dyn Listener>> {
        let NetworkReply::Listen(result) = self.0.call(NetworkCall::Listen(endpoint)) else {
            panic!("expected listen reply")
        };
        result
    }
}

#[cfg(test)]
mod tests;
