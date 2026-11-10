//! Deterministic authenticated event and dependency substitutes, never production protocol registration.
use super::Error;
use std::collections::VecDeque;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Event {
    Data(Vec<u8>),
    End,
    Cancel,
    LostReply,
}
pub(super) struct Stream {
    events: VecDeque<Event>,
    pub(super) sent: Vec<Vec<u8>>,
    pub(super) capacity: usize,
}
impl Stream {
    // Deliver exact ordered DATA fragments followed by an explicitly selected terminal event.
    pub(super) fn new(bytes: &[u8], cuts: &[usize], terminal: Event) -> Self {
        let mut events = VecDeque::new();
        let mut start = 0;
        for &end in cuts {
            assert!(end >= start && end <= bytes.len());
            if end > start {
                events.push_back(Event::Data(bytes[start..end].to_vec()));
            }
            start = end;
        }
        if start < bytes.len() {
            events.push_back(Event::Data(bytes[start..].to_vec()));
        }
        events.push_back(terminal);
        Self {
            events,
            sent: vec![],
            capacity: 0,
        }
    }
    // An exhausted source is unavailable, never a synthesized normal END.
    pub(super) fn next(&mut self) -> Result<Event, Error> {
        self.events.pop_front().ok_or(Error::Unavailable)
    }
    // Backpressure refuses before retaining a byte; retrying after capacity changes is caller-controlled.
    pub(super) fn send(&mut self, bytes: &[u8]) -> Result<(), Error> {
        if bytes.len() > self.capacity {
            return Err(Error::Limit);
        }
        self.capacity -= bytes.len();
        self.sent.push(bytes.to_vec());
        Ok(())
    }
}
pub(super) struct Script<T> {
    replies: VecDeque<Result<T, Error>>,
    pub(super) calls: usize,
}
impl<T> Script<T> {
    // Preset Board/Services/Ledger/entropy replies are independent of production algorithms.
    pub(super) fn new(replies: impl IntoIterator<Item = Result<T, Error>>) -> Self {
        Self {
            replies: replies.into_iter().collect(),
            calls: 0,
        }
    }
    // Calls are counted even on failure so tests detect hidden retries and duplicated effects.
    pub(super) fn next(&mut self) -> Result<T, Error> {
        self.calls += 1;
        self.replies.pop_front().unwrap_or(Err(Error::Unavailable))
    }
}
// Fragmentation, terminal variants, blocked sends and explicit dependency failures preserve their exact meaning.
#[test]
fn deterministic_boundaries() {
    for terminal in [Event::End, Event::Cancel, Event::LostReply] {
        let mut s = Stream::new(b"abc", &[1, 2], terminal);
        assert_eq!(s.next(), Ok(Event::Data(vec![b'a'])));
        assert_eq!(s.next(), Ok(Event::Data(vec![b'b'])));
        assert_eq!(s.next(), Ok(Event::Data(vec![b'c'])));
        assert!(s.next().is_ok());
        assert_eq!(s.next(), Err(Error::Unavailable));
        assert_eq!(s.send(b"abc"), Err(Error::Limit));
        assert!(s.sent.is_empty());
        s.capacity = 3;
        assert_eq!(s.send(b"abc"), Ok(()));
        assert_eq!(s.sent, vec![b"abc".to_vec()]);
    }
    let mut script = Script::new([Ok(7u64), Err(Error::Unknown)]);
    assert_eq!(script.next(), Ok(7));
    assert_eq!(script.next(), Err(Error::Unknown));
    assert_eq!(script.next(), Err(Error::Unavailable));
    assert_eq!(script.calls, 3);
}

pub(super) mod board;
pub(super) mod fixtures;
