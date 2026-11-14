//! Bounded emission retains sender progress and advances exact exchange ordering before Transport sees bytes.
use super::*;
impl<P: services::persistence::Persistence, L: ledger::persistence::Persistence> Stream<P, L> {
    // Preserve exact validated response bytes and phase before admitting them to the bridge's bounded queue.
    pub(super) fn emit(&mut self, output: &mut Output, record: &Record) -> Result<(), Error> {
        tracing::trace!("Emitting one validated endpoint response record");
        match record {
            Record::Control(bytes) => self.exchange.control(
                Direction::Local,
                Message::parse(bytes, self.owner.config.limits.nodes)?.kind(),
            )?,
            Record::Body { .. } => self.exchange.body(Direction::Local)?,
        }
        output.enqueue(record)
    }
    // Encode one source record within remaining output capacity, without advancing on local backpressure.
    pub(super) fn pump(&mut self, output: &mut Output) -> Result<(), Error> {
        tracing::trace!("Filling bounded endpoint output from committed sources");
        let Phase::Sending(source) = &mut self.phase else {
            return Ok(());
        };
        if output.available() == 0 {
            return Ok(());
        }
        let mut buffer = peer::outbound::Outbound::new(output.available())?;
        let cached_first = matches!(source, Source::Cached(_)) && !self.exchange.accepted();
        let result = match source {
            Source::Work(sender) => sender.enqueue_next(&mut buffer, peer::record::DATA_MAX),
            Source::Check(sender) => sender.enqueue_next(&mut buffer, peer::record::DATA_MAX),
            Source::Cached(sender) => sender.enqueue_next(&mut buffer, peer::record::DATA_MAX),
        };
        match result {
            Err(Error::Limit) => return Ok(()),
            Err(error) => return Err(error),
            Ok(_) => {}
        }
        if let Some(bytes) = buffer.front(usize::MAX) {
            let mut decoder = peer::record::Decoder::default();
            let (count, record) = decoder.push(bytes)?;
            if count != bytes.len() {
                return Err(Error::Invalid);
            }
            let record = record.ok_or(Error::Invalid)?;
            if cached_first {
                let Source::Cached(sender) = source else {
                    return Err(Error::Invalid);
                };
                self.exchange.cached_local(sender.proof())?;
                output.enqueue(&record)?;
            } else {
                self.emit(output, &record)?;
            }
        }
        let Phase::Sending(source) = &self.phase else {
            return Err(Error::Conflict);
        };
        let queued = match source {
            Source::Work(sender) => sender.queued(),
            Source::Check(sender) => sender.queued(),
            Source::Cached(sender) => sender.queued(),
        };
        if queued && !self.end_requested {
            self.exchange.end(Direction::Local)?;
            output.end();
            self.end_requested = true;
        }
        Ok(())
    }
}
