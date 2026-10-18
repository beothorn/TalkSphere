//! Whole-frame round-robin selection with stream ordering and OPEN sequence barriers.
use super::*;
use std::collections::BTreeSet;

impl Session {
    // Select one head per stream, rotating IDs without moving any frame within its own stream.
    pub(super) fn next_output(&mut self) -> Option<WireFrame> {
        tracing::trace!(
            function = "Session::next_output",
            "Scheduling the next ready stream frame"
        );
        let first_open = self
            .outbound
            .iter()
            .position(|frame| frame.header().frame_type() == FrameType::Open);
        let mut seen = BTreeSet::new();
        let mut eligible = Vec::new();
        for (index, frame) in self.outbound.iter().enumerate() {
            let id = frame.header().stream_id();
            if !seen.insert(id) {
                continue;
            }
            let kind = frame.header().frame_type();
            if kind == FrameType::Open && first_open != Some(index) {
                continue;
            }
            if kind == FrameType::GoAway && first_open.is_some_and(|open| open < index) {
                continue;
            }
            eligible.push((id, index));
        }
        let next = eligible
            .iter()
            .filter(|(id, _)| self.last_output.is_none_or(|last| *id > last))
            .min_by_key(|(id, _)| *id)
            .or_else(|| eligible.iter().min_by_key(|(id, _)| *id))?;
        self.last_output = Some(next.0);
        self.outbound.remove(next.1)
    }

    /// Pop the next event for one authorized consumer without waiting for another consumer.
    /// Global connection events remain available through pop_event; stream event order is preserved.
    pub fn pop_stream_event(&mut self, owner: &StreamOwner) -> Option<Event> {
        tracing::trace!(
            function = "Session::pop_stream_event",
            "Delivering an independently ready consumer event"
        );
        let index = self.events.iter().position(|event| match event {
            Event::StreamOpened { stream, .. }
            | Event::Data { stream, .. }
            | Event::RemoteEnd { stream }
            | Event::StreamClosed { stream, .. } => {
                stream.authorize(self.handle.id(), owner).is_ok()
            }
            _ => false,
        })?;
        self.events.remove(index)
    }
}
