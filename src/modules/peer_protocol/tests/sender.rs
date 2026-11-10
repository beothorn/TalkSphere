//! Ordered body data remains stable under tiny queues and explicit Transport backpressure.
use super::{
    Error,
    contract::Limits,
    outbound::Outbound,
    record::{DATA_MAX, Decoder, Record},
    sender::Sender,
    value::Descriptor,
};
use crate::modules::services::handler::Blob;
// A one-data-byte queue forces body splitting without duplicate offsets or source reordering.
#[test]
fn tiny_queue_preserves_source_order() {
    let bodies = vec![
        Blob::new(b"abc".to_vec(), 3).unwrap(),
        Blob::new(b"de".to_vec(), 2).unwrap(),
    ];
    let ds = [
        Descriptor {
            id: 0,
            length: 3,
            text: false,
        },
        Descriptor {
            id: 1,
            length: 2,
            text: false,
        },
    ];
    let mut sender = Sender::new(bodies, &ds, Limits::default()).unwrap();
    let mut queue = Outbound::new(18).unwrap();
    let mut records = vec![];
    while !sender.queued() {
        sender.enqueue_next(&mut queue, DATA_MAX).unwrap();
        if !sender.queued() {
            assert_eq!(sender.enqueue_next(&mut queue, DATA_MAX), Err(Error::Limit));
        }
        let bytes = queue.front(100).unwrap().to_vec();
        queue.advance(bytes.len()).unwrap();
        let mut decoder = Decoder::default();
        let (_, record) = decoder.push(&bytes).unwrap();
        records.push(record.unwrap());
        decoder.finish().unwrap();
    }
    assert_eq!(
        records,
        vec![
            Record::Body {
                id: 0,
                offset: 0,
                data: b"a".to_vec()
            },
            Record::Body {
                id: 0,
                offset: 1,
                data: b"b".to_vec()
            },
            Record::Body {
                id: 0,
                offset: 2,
                data: b"c".to_vec()
            },
            Record::Body {
                id: 1,
                offset: 0,
                data: b"d".to_vec()
            },
            Record::Body {
                id: 1,
                offset: 1,
                data: b"e".to_vec()
            }
        ]
    );
    assert_eq!(sender.enqueue_next(&mut queue, 1), Ok(false));
    assert_eq!(sender.enqueue_next(&mut queue, 0), Err(Error::Invalid));
}
// Sources are validated before any queue operation, and a chunk never exceeds the fixed wire maximum.
#[test]
fn source_admission_and_chunk_limits() {
    let ds = [Descriptor {
        id: 0,
        length: (DATA_MAX + 1) as u64,
        text: false,
    }];
    let source = Blob::new(vec![7; DATA_MAX + 1], DATA_MAX + 1).unwrap();
    let mut s = Sender::new(vec![source.clone()], &ds, Limits::default()).unwrap();
    let mut queue = Outbound::new(DATA_MAX + 17).unwrap();
    s.enqueue_next(&mut queue, usize::MAX).unwrap();
    let bytes = queue.front(usize::MAX).unwrap();
    let mut d = Decoder::default();
    let (_, record) = d.push(bytes).unwrap();
    assert!(matches!(record,Some(Record::Body{data,..}) if data.len()==DATA_MAX));
    assert!(!s.queued());
    assert!(Sender::new(vec![source], &[], Limits::default()).is_err());
    assert!(
        Sender::new(
            vec![Blob::new(vec![255], 1).unwrap()],
            &[Descriptor {
                id: 0,
                length: 1,
                text: true
            }],
            Limits::default()
        )
        .is_err()
    );
    assert!(
        Sender::new(
            vec![Blob::new(vec![], 1).unwrap()],
            &[Descriptor {
                id: 0,
                length: 0,
                text: false
            }],
            Limits::default()
        )
        .is_err()
    );
}
