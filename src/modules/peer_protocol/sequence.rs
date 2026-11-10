//! Shared bounded sequencing for the two actual body-bearing input/result transmission paths.
use super::{Error, outbound::Outbound, record::Record, sender::Sender};
enum Stage {
    First,
    Bodies,
    Done,
}
pub(crate) struct Sequence {
    first: Record,
    last: Record,
    bodies: Sender,
    stage: Stage,
}
impl Sequence {
    // Retain controls by ownership so backpressure checks precede any full-control encoding/copy.
    pub(crate) fn new(first: Vec<u8>, bodies: Sender, last: &[u8]) -> Self {
        tracing::trace!("Creating ordered control/body/terminal sequence");
        Self {
            first: Record::Control(first),
            last: Record::Control(last.to_vec()),
            bodies,
            stage: Stage::First,
        }
    }
    // Advance only after whole-record queue admission; retries cannot duplicate controls, bodies or markers.
    pub(crate) fn enqueue_next(
        &mut self,
        queue: &mut Outbound,
        chunk: usize,
    ) -> Result<bool, Error> {
        tracing::trace!("Queueing the next ordered value transfer record");
        match self.stage {
            Stage::First => {
                queue.enqueue(&self.first)?;
                self.stage = Stage::Bodies;
                Ok(true)
            }
            Stage::Bodies => {
                if self.bodies.enqueue_next(queue, chunk)? {
                    return Ok(true);
                }
                queue.enqueue(&self.last)?;
                self.stage = Stage::Done;
                Ok(true)
            }
            Stage::Done => Ok(false),
        }
    }
    // Local queue completion says nothing about remote receipt; composition drains before Transport END.
    pub(crate) fn queued(&self) -> bool {
        tracing::trace!("Checking ordered value transfer queue completion");
        matches!(self.stage, Stage::Done)
    }
}
