//! Worker-owned framing and callback execution, with no Transport control authority.
use super::*;
use crate::modules::peer_protocol::record::Decoder;
/// Owned observations transferred once; input bytes remain charged until worker completion.
pub(super) enum Input {
    Open,
    Data(Vec<u8>, Instant),
    End,
    Writable,
    Closed(StreamOutcome),
}
pub(super) struct State {
    factory: Arc<dyn Factory>,
    scope: Scope,
    opener: Direction,
    handler: Option<Box<dyn Handler>>,
    decoder: Decoder,
    pub(super) output: Output,
    controls: usize,
    window: Instant,
    rate: usize,
}
pub(super) struct Returned {
    pub(super) id: u64,
    pub(super) state: Option<State>,
    pub(super) result: Result<(), Error>,
    pub(super) cleanup: bool,
}
impl State {
    // Allocate only after the reactor reserves this stream's entire framing/input/output budget.
    pub(super) fn new(
        factory: Arc<dyn Factory>,
        scope: Scope,
        opener: Direction,
        config: Config,
    ) -> Result<Self, Error> {
        tracing::trace!("Creating worker-owned peer exchange framing");
        Ok(Self {
            factory,
            scope,
            opener,
            handler: None,
            decoder: Decoder::default(),
            output: Output {
                queue: Outbound::new(config.output_bytes)?,
                end: false,
                received_at: Instant::now(),
                drained_at: None,
            },
            controls: 0,
            window: Instant::now(),
            rate: config.controls_per_second,
        })
    }
    // Return state even on a typed callback error so later actual closure still reaches its owner.
    pub(super) fn run(mut self, id: u64, input: Input) -> Returned {
        tracing::trace!("Executing one owned peer observation outside the Transport reactor");
        let cleanup = matches!(input, Input::Closed(_));
        let result = self.apply(input);
        let state = if cleanup { None } else { Some(self) };
        Returned {
            id,
            state,
            result,
            cleanup,
        }
    }
    // Check complete framing and local control quotas before invoking a trusted application handler.
    fn apply(&mut self, input: Input) -> Result<(), Error> {
        tracing::trace!("Applying a bounded authenticated peer observation");
        if !matches!(input, Input::Closed(_)) && Instant::now() >= self.scope.deadline {
            return Err(Error::Cancelled);
        }
        if let Input::Open = input {
            if self.handler.is_some() {
                return Err(Error::Conflict);
            }
            self.handler = Some(
                self.factory
                    .open(self.scope, self.opener, &mut self.output)?,
            );
            return Ok(());
        }
        if let Input::Closed(outcome) = input {
            return match &mut self.handler {
                Some(handler) => handler.closed(outcome),
                None => Ok(()),
            };
        }
        let handler = self.handler.as_mut().ok_or(Error::Conflict)?;
        match input {
            Input::Data(bytes, received_at) => {
                self.output.received_at = received_at;
                let mut offset = 0;
                while offset < bytes.len() {
                    let (consumed, record) = self.decoder.push(&bytes[offset..])?;
                    offset += consumed;
                    if let Some(record) = record {
                        if matches!(record, Record::Control(_)) {
                            let now = Instant::now();
                            if now.duration_since(self.window) >= Duration::from_secs(1) {
                                self.window = now;
                                self.controls = 0;
                            }
                            self.controls = self.controls.checked_add(1).ok_or(Error::Limit)?;
                            if self.controls > self.rate {
                                return Err(Error::Limit);
                            }
                        }
                        handler.record(record, &mut self.output)?;
                    }
                }
                Ok(())
            }
            Input::End => {
                self.decoder.finish()?;
                handler.remote_end(&mut self.output)
            }
            Input::Writable => handler.writable(&mut self.output),
            Input::Open | Input::Closed(_) => Err(Error::Conflict),
        }
    }
}
