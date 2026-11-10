//! Independent pinned record framing, fragmentation and malformed-header tests.
use super::test_support::fixtures::{BODY, CONTROL};
use super::{
    Error,
    record::{CONTROL_MAX, DATA_MAX, Decoder, Record},
};
// Every boundary split and bytewise delivery yields the same exact records, including coalesced DATA.
#[test]
fn wire_vectors_and_all_splits() {
    let expected = [
        Record::Control(br#"{"type":"board.get"}"#.to_vec()),
        Record::Body {
            id: 0,
            offset: 0,
            data: b"abc".to_vec(),
        },
    ];
    assert_eq!(expected[0].encode().unwrap(), CONTROL);
    assert_eq!(expected[1].encode().unwrap(), BODY);
    let bytes = [CONTROL, BODY].concat();
    for split in 0..=bytes.len() {
        let mut d = Decoder::default();
        let mut records = vec![];
        for fragment in [&bytes[..split], &bytes[split..]] {
            let mut pos = 0;
            while pos < fragment.len() {
                let (count, r) = d.push(&fragment[pos..]).unwrap();
                assert!(count > 0);
                pos += count;
                if let Some(r) = r {
                    records.push(r);
                }
            }
        }
        assert_eq!(records, expected);
        assert_eq!(d.finish(), Ok(()));
        assert_eq!(d.push(&[]), Err(Error::Conflict));
    }
    let mut d = Decoder::default();
    let mut records = vec![];
    for byte in &bytes {
        if let (_, Some(r)) = d.push(&[*byte]).unwrap() {
            records.push(r);
        }
    }
    assert_eq!(records, expected);
    d.finish().unwrap();
}
// Malicious lengths are refused at the fixed header; truncation never returns a complete record.
#[test]
fn invalid_and_maximum_records() {
    for (kind, length) in [
        (0, 1),
        (3, 1),
        (1, 0),
        (1, CONTROL_MAX + 1),
        (2, 12),
        (2, DATA_MAX + 13),
    ] {
        let mut d = Decoder::default();
        let mut bytes = vec![kind];
        bytes.extend_from_slice(&(length as u32).to_be_bytes());
        assert_eq!(d.push(&bytes), Err(Error::Invalid));
        assert_eq!(d.push(CONTROL), Err(Error::Conflict));
    }
    for end in 1..CONTROL.len() {
        let mut d = Decoder::default();
        assert_eq!(d.push(&CONTROL[..end]).unwrap().1, None);
        assert_eq!(d.finish(), Err(Error::Invalid));
    }
    for record in [
        Record::Control(vec![b' '; CONTROL_MAX]),
        Record::Body {
            id: u32::MAX,
            offset: u64::MAX,
            data: vec![0; DATA_MAX],
        },
    ] {
        let bytes = record.encode().unwrap();
        let mut d = Decoder::default();
        assert_eq!(d.push(&bytes), Ok((bytes.len(), Some(record))));
        d.finish().unwrap();
    }
    for record in [
        Record::Control(vec![]),
        Record::Control(vec![0; CONTROL_MAX + 1]),
        Record::Body {
            id: 0,
            offset: 0,
            data: vec![],
        },
    ] {
        assert_eq!(record.encode(), Err(Error::Invalid));
    }
}
