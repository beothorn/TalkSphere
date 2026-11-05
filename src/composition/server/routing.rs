//! Authorized local stream routing; production starts with no advertised application handlers.
use super::Failure;
use crate::modules::transport::{
    api::{Accepted, CancellationReason, Event, Protocol, Request, StreamAction, TransportControl},
    handles::StreamOwner,
};
/// Trusted local stream consumer; events have already passed Transport authentication and ownership checks.
pub trait Consumer {
    /// Consume one owned observation without retaining unbounded peer data.
    /// # Errors
    /// Failure requests cancellation of this stream; it never authorizes a retry or service rollback.
    fn event(&mut self, event: Event) -> Result<(), Failure>;
}
/// At most 256 locally registered consumers; no peer input registers a handler.
#[derive(Default)]
pub struct Router {
    consumers: Vec<(StreamOwner, Box<dyn Consumer>)>,
}
impl Router {
    /// Register a consumer through Transport and return its local stream authority.
    /// # Errors
    /// Invalid/duplicate/excess registrations return Runtime without adding a consumer.
    pub fn register(
        &mut self,
        driver: &mut impl TransportControl,
        protocol: Protocol,
        consumer: Box<dyn Consumer>,
    ) -> Result<StreamOwner, Failure> {
        tracing::trace!("Registering a trusted application stream consumer");
        if self.consumers.len() >= 256 {
            return Err(Failure::Runtime);
        }
        let Accepted::Registration(owner) = driver
            .submit(Request::RegisterProtocol { protocol })
            .map_err(|_| Failure::Runtime)?
        else {
            return Err(Failure::Runtime);
        };
        self.consumers.push((owner.clone(), consumer));
        Ok(owner)
    }
    /// Route stream events only to their registered owner; return lifecycle observations to composition.
    /// # Errors
    /// An event with no matching local owner indicates an invalid runtime boundary and returns Runtime.
    pub fn route(
        &mut self,
        driver: &mut impl TransportControl,
        event: Event,
    ) -> Result<Option<Event>, Failure> {
        tracing::trace!("Routing an authenticated Transport observation");
        let stream = match &event {
            Event::StreamOpened { stream, .. }
            | Event::Data { stream, .. }
            | Event::RemoteEnd { stream }
            | Event::StreamClosed { stream, .. } => stream.clone(),
            _ => return Ok(Some(event)),
        };
        let Some((owner, consumer)) = self
            .consumers
            .iter_mut()
            .find(|(owner, _)| stream.authorize(stream.connection_id(), owner).is_ok())
        else {
            return Err(Failure::Runtime);
        };
        if consumer.event(event).is_err() {
            let _ = driver.submit(Request::Stream {
                owner: owner.clone(),
                stream,
                action: StreamAction::Cancel(CancellationReason::InternalError),
            });
            tracing::warn!("Local stream consumer failed; cancellation requested without retry");
        }
        Ok(None)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::transport::{
        TransportError,
        handles::{ConnectionHandle, StreamHandle},
    };
    use std::{
        num::NonZeroU32,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };
    struct Control {
        owner: StreamOwner,
        cancelled: usize,
    }
    impl TransportControl for Control {
        // Return explicit local registration authority and count consumer-failure cancellation requests.
        fn submit(&mut self, request: Request) -> Result<Accepted, TransportError> {
            match request {
                Request::RegisterProtocol { .. } => Ok(Accepted::Registration(self.owner.clone())),
                Request::Stream {
                    action: StreamAction::Cancel(_),
                    ..
                } => {
                    self.cancelled += 1;
                    Ok(Accepted::Queued)
                }
                _ => Err(TransportError::InvalidRequest),
            }
        }
        // These tests inject observations at the trusted routing boundary, not network authentication.
        fn poll_event(&mut self, _: &mut std::task::Context<'_>) -> std::task::Poll<Option<Event>> {
            std::task::Poll::Pending
        }
    }
    struct Sink {
        count: Arc<AtomicUsize>,
        fail: bool,
    }
    impl Consumer for Sink {
        // Count delivery without copying payloads and optionally simulate a local consumer failure.
        fn event(&mut self, _: Event) -> Result<(), Failure> {
            self.count.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                Err(Failure::Runtime)
            } else {
                Ok(())
            }
        }
    }
    // Foreign owner events never reach a consumer; consumer failure cancels only its own stream.
    #[test]
    fn consumers_are_isolated_and_failure_requests_cancellation() {
        let owner = StreamOwner::new();
        let connection = ConnectionHandle::new();
        let count = Arc::new(AtomicUsize::new(0));
        let mut driver = Control {
            owner: owner.clone(),
            cancelled: 0,
        };
        let mut router = Router::default();
        router
            .register(
                &mut driver,
                Protocol {
                    id: "org.example.test".into(),
                    version: 1,
                },
                Box::new(Sink {
                    count: count.clone(),
                    fail: true,
                }),
            )
            .unwrap();
        let foreign = StreamHandle::new(
            &connection,
            &StreamOwner::new(),
            NonZeroU32::new(1).unwrap(),
        );
        assert!(
            router
                .route(
                    &mut driver,
                    Event::Data {
                        stream: foreign,
                        bytes: vec![1]
                    }
                )
                .is_err()
        );
        assert_eq!(count.load(Ordering::SeqCst), 0);
        let stream = StreamHandle::new(&connection, &owner, NonZeroU32::new(1).unwrap());
        assert!(
            router
                .route(
                    &mut driver,
                    Event::Data {
                        stream,
                        bytes: vec![1]
                    }
                )
                .unwrap()
                .is_none()
        );
        assert_eq!(driver.cancelled, 1);
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert!(
            router
                .route(
                    &mut driver,
                    Event::ConnectionClosed {
                        connection: connection.id().clone(),
                        failure: None
                    }
                )
                .unwrap()
                .is_some()
        );
    }
}
