use std::{
    collections::VecDeque,
    io,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};

use crate::modules::transport::ports::io::Socket;

#[derive(Debug)]
pub enum ReadStep {
    /// One read chunk; a small destination buffer may split it further.
    Bytes(Vec<u8>),
    Error(io::ErrorKind),
    Eof,
}

#[derive(Debug)]
pub enum WriteStep {
    /// Accept at most this many bytes in one poll, including deliberate zero progress.
    Accept(usize),
    Error(io::ErrorKind),
}

#[derive(Default)]
struct State {
    reads: VecDeque<ReadStep>,
    writes: VecDeque<WriteStep>,
    written: Vec<u8>,
    read_waker: Option<Waker>,
    write_waker: Option<Waker>,
    eof_queued: bool,
    eof: bool,
    closed: bool,
}

pub struct ScriptedSocket(Arc<Mutex<State>>);

#[derive(Clone)]
pub struct SocketControl(Arc<Mutex<State>>);

impl ScriptedSocket {
    /// Empty scripts mean Pending, not EOF or a successful write.
    pub fn new() -> (Self, SocketControl) {
        let state = Arc::new(Mutex::new(State::default()));
        (Self(state.clone()), SocketControl(state))
    }
}

impl SocketControl {
    pub fn push_read(&self, step: ReadStep) {
        if let ReadStep::Bytes(bytes) = &step {
            assert!(!bytes.is_empty(), "use Eof for EOF");
        }
        let wake = {
            let mut state = self.0.lock().unwrap();
            if state.closed || state.eof_queued {
                drop(state);
                panic!("read added after closure or EOF");
            }
            state.eof_queued = matches!(step, ReadStep::Eof);
            state.reads.push_back(step);
            state.read_waker.take()
        };
        if let Some(waker) = wake {
            waker.wake();
        }
    }

    pub fn push_write(&self, step: WriteStep) {
        let wake = {
            let mut state = self.0.lock().unwrap();
            if state.closed {
                drop(state);
                panic!("write added after closure");
            }
            state.writes.push_back(step);
            state.write_waker.take()
        };
        if let Some(waker) = wake {
            waker.wake();
        }
    }

    pub fn written(&self) -> Vec<u8> {
        self.0.lock().unwrap().written.clone()
    }
    pub fn is_closed(&self) -> bool {
        self.0.lock().unwrap().closed
    }
    pub fn assert_finished(&self) {
        let finished = {
            let state = self.0.lock().unwrap();
            state.reads.is_empty() && state.writes.is_empty()
        };
        // A failed assertion must still allow the socket to close during unwinding.
        assert!(finished, "unconsumed socket steps");
    }
}

impl Socket for ScriptedSocket {
    fn poll_read(&mut self, cx: &mut Context<'_>, buffer: &mut [u8]) -> Poll<io::Result<usize>> {
        let mut state = self.0.lock().unwrap();
        if state.closed {
            return Poll::Ready(Err(io::ErrorKind::NotConnected.into()));
        }
        if buffer.is_empty() || state.eof {
            return Poll::Ready(Ok(0));
        }
        match state.reads.pop_front() {
            Some(ReadStep::Bytes(mut bytes)) => {
                let count = buffer.len().min(bytes.len());
                buffer[..count].copy_from_slice(&bytes[..count]);
                if count < bytes.len() {
                    state
                        .reads
                        .push_front(ReadStep::Bytes(bytes.split_off(count)));
                }
                Poll::Ready(Ok(count))
            }
            Some(ReadStep::Error(error)) => Poll::Ready(Err(error.into())),
            Some(ReadStep::Eof) => {
                state.eof = true;
                Poll::Ready(Ok(0))
            }
            None => {
                state.read_waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }

    fn poll_write(&mut self, cx: &mut Context<'_>, bytes: &[u8]) -> Poll<io::Result<usize>> {
        let mut state = self.0.lock().unwrap();
        if state.closed {
            return Poll::Ready(Err(io::ErrorKind::NotConnected.into()));
        }
        if bytes.is_empty() {
            return Poll::Ready(Ok(0));
        }
        match state.writes.pop_front() {
            Some(WriteStep::Accept(limit)) => {
                let count = limit.min(bytes.len());
                state.written.extend_from_slice(&bytes[..count]);
                Poll::Ready(Ok(count))
            }
            Some(WriteStep::Error(error)) => Poll::Ready(Err(error.into())),
            None => {
                state.write_waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }

    fn close(&mut self) -> io::Result<()> {
        let (read, write) = {
            let mut state = self.0.lock().unwrap();
            state.closed = true;
            (state.read_waker.take(), state.write_waker.take())
        };
        for waker in [read, write].into_iter().flatten() {
            waker.wake();
        }
        Ok(())
    }
}

impl Drop for ScriptedSocket {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

#[cfg(test)]
mod tests;
