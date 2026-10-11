use crate::modules::transport::api::Event;

/// Records delivered events in order; the harness routes them to this consumer.
/// A full recorder returns ownership of the event, simulating a stalled reader.
/// Capacity counts events, not bytes; production resource policy is unchanged.
pub struct RecordingConsumer {
    capacity: usize,
    events: Vec<Event>,
}

impl RecordingConsumer {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "consumer capacity must be positive");
        Self {
            capacity,
            events: Vec::new(),
        }
    }

    pub fn try_record(&mut self, event: Event) -> Result<(), Box<Event>> {
        if self.events.len() == self.capacity {
            return Err(Box::new(event));
        }
        self.events.push(event);
        Ok(())
    }

    pub fn events(&self) -> &[Event] {
        &self.events
    }
    pub fn take(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::transport::{api::StreamOutcome, handles::test_fixtures};

    #[test]
    fn records_exact_delivery_order_and_bytes_without_interpreting_them() {
        let connection = test_fixtures::connection();
        let owner = test_fixtures::owner();
        let stream = test_fixtures::stream(&connection, &owner);
        let mut consumer = RecordingConsumer::new(3);
        consumer
            .try_record(Event::Data {
                stream: stream.clone(),
                bytes: vec![0, 255, 1],
            })
            .unwrap();
        consumer
            .try_record(Event::RemoteEnd {
                stream: stream.clone(),
            })
            .unwrap();
        consumer
            .try_record(Event::StreamClosed {
                stream: stream.clone(),
                outcome: StreamOutcome::Completed,
            })
            .unwrap();
        assert!(
            matches!(&consumer.events()[0], Event::Data { stream: actual, bytes } if actual == &stream && bytes == &[0, 255, 1])
        );
        assert!(
            matches!(&consumer.events()[1], Event::RemoteEnd { stream: actual } if actual == &stream)
        );
        assert!(
            matches!(&consumer.events()[2], Event::StreamClosed { stream: actual, outcome: StreamOutcome::Completed } if actual == &stream)
        );
        assert_eq!(consumer.take().len(), 3);
        assert!(consumer.events().is_empty());
    }

    #[test]
    fn a_full_consumer_returns_the_undelivered_event_and_recovers_after_drain() {
        let connection = test_fixtures::connection();
        let mut consumer = RecordingConsumer::new(1);
        consumer
            .try_record(Event::IncomingConnection(connection.clone()))
            .unwrap();
        let event = consumer
            .try_record(Event::ConnectionClosed {
                connection: connection.id().clone(),
                failure: None,
            })
            .unwrap_err();
        assert_eq!(consumer.events().len(), 1);
        assert!(
            matches!(&*event, Event::ConnectionClosed { connection: id, failure: None } if id == connection.id())
        );
        consumer.take();
        consumer.try_record(*event).unwrap();
        assert!(matches!(
            &consumer.events()[0],
            Event::ConnectionClosed { .. }
        ));
    }
}
