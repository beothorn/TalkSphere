//! Outbound ownership and progress tests under finite Transport admission.
use super::{Error, outbound::Outbound, record::Record};
// A blocked consumer can inspect the same bytes repeatedly; only explicit admission advances them.
#[test]
fn partial_send_and_backpressure() {
    let record = Record::Control(br#"{"type":"board.get"}"#.to_vec());
    let bytes = record.encode().unwrap();
    let mut q = Outbound::new(bytes.len()).unwrap();
    q.enqueue(&record).unwrap();
    assert_eq!(q.enqueue(&record), Err(Error::Limit));
    assert_eq!(q.pending(), bytes.len());
    assert_eq!(q.front(0), None);
    assert_eq!(q.front(1), Some(&bytes[..1]));
    assert_eq!(q.front(1), Some(&bytes[..1]));
    assert_eq!(q.advance(0), Err(Error::Invalid));
    assert_eq!(q.advance(bytes.len() + 1), Err(Error::Invalid));
    let mut received = vec![];
    while let Some(fragment) = q.front(3) {
        let count = fragment.len();
        received.extend_from_slice(fragment);
        q.advance(count).unwrap();
    }
    assert_eq!(received, bytes);
    assert_eq!(q.pending(), 0);
    assert_eq!(q.advance(1), Err(Error::Invalid));
    q.enqueue(&record).unwrap();
    q.cancel();
    assert_eq!(q.pending(), 0);
    assert_eq!(q.front(1), None);
    assert!(matches!(Outbound::new(0), Err(Error::Invalid)));
    assert_eq!(q.enqueue(&Record::Control(vec![])), Err(Error::Invalid));
}
