//! Test-only bounded output observations; these acknowledgements are not real network delivery.
use super::*;
// Construct a finite bridge queue without issuing or forging any Transport capability.
pub(in crate::modules::peer_protocol) fn output(maximum: usize) -> Output {
    Output {
        queue: Outbound::new(maximum).unwrap(),
        end: false,
        received_at: Instant::now(),
        drained_at: None,
    }
}
// Consume complete locally queued records for handler unit tests, independent of socket authentication evidence.
pub(in crate::modules::peer_protocol) fn drain(output: &mut Output) -> Vec<Record> {
    let mut records = Vec::new();
    while let Some(bytes) = output.queue.front(usize::MAX) {
        let count = bytes.len();
        let mut decoder = crate::modules::peer_protocol::record::Decoder::default();
        let (consumed, record) = decoder.push(bytes).unwrap();
        assert_eq!(consumed, count);
        records.push(record.unwrap());
        output.queue.advance(count).unwrap();
        if output.queue.pending() == 0 {
            output.drained_at = Some(Instant::now());
        }
    }
    records
}
// Observe requested END only; actual normal completion is tested through the real Transport adapter separately.
pub(in crate::modules::peer_protocol) fn ended(output: &Output) -> bool {
    output.end
}
