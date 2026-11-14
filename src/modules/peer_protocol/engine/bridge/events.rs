//! Authenticated observation routing and original stream/connection correlation.
use super::*;
impl Bridge {
    /// Route one verified observation, returning events belonging to other composition responsibilities.
    /// # Errors
    /// Invalid authentication correlations or exhausted connection limits require composition handling;
    /// malformed owned streams are isolated with a stream-scoped Transport error instead.
    pub fn event(
        &mut self,
        driver: &mut impl TransportControl,
        event: Event,
    ) -> Result<Option<Event>, Error> {
        tracing::trace!("Routing a verified observation through the bounded peer adapter");
        match event {
            Event::Authenticated(context) => {
                if self.stopped || self.connections.len() >= self.config.connections {
                    return Err(Error::Limit);
                }
                if self
                    .connections
                    .iter()
                    .any(|c| c.context.connection == context.connection)
                {
                    return Err(Error::Conflict);
                }
                self.connection_sequence = self
                    .connection_sequence
                    .checked_add(1)
                    .ok_or(Error::Limit)?;
                let mut nonce = [0; 32];
                nonce[..24].copy_from_slice(&self.domain);
                nonce[24..].copy_from_slice(&self.connection_sequence.to_be_bytes());
                self.connections.push(Connection {
                    context: context.clone(),
                    nonce,
                });
                Ok(Some(Event::Authenticated(context)))
            }
            Event::StreamOpened {
                context,
                stream,
                protocol: selected,
            } if selected == protocol() => {
                if stream
                    .authorize(stream.connection_id(), &self.owner)
                    .is_err()
                {
                    return Err(Error::Denied);
                }
                // Transport acknowledges locally opened streams through the same event contract.
                if let Some(entry) = self.entries.iter_mut().find(|entry| entry.stream == stream) {
                    if entry.opened
                        || !self
                            .connections
                            .iter()
                            .any(|connection| connection.context == context)
                    {
                        return Err(Error::Conflict);
                    }
                    entry.opened = true;
                    return Ok(None);
                }
                if self
                    .admit(context, stream.clone(), Direction::Remote)
                    .is_err()
                {
                    cancel(
                        driver,
                        &self.owner,
                        &stream,
                        StreamAction::Cancel(CancellationReason::ResourceLimit),
                    );
                }
                Ok(None)
            }
            Event::Data { stream, bytes } if self.owns(&stream) => {
                if let Some(entry) = self.entries.iter_mut().find(|e| e.stream == stream) {
                    if entry.closing || entry.closed.is_some() {
                        return Ok(None);
                    }
                    if bytes.len() > self.config.input_bytes.saturating_sub(entry.input_bytes)
                        || entry.pending.len() >= 64
                    {
                        entry.closing = true;
                        entry.termination = Some(Stop::Resource);
                        cancel(
                            driver,
                            &self.owner,
                            &stream,
                            StreamAction::Cancel(CancellationReason::ResourceLimit),
                        );
                    } else {
                        entry.input_bytes += bytes.len();
                        entry.pending.push_back(Input::Data(bytes));
                    }
                }
                Ok(None)
            }
            Event::RemoteEnd { stream } if self.owns(&stream) => {
                if let Some(entry) = self
                    .entries
                    .iter_mut()
                    .find(|e| e.stream == stream && !e.closing && e.closed.is_none())
                {
                    if entry.pending.len() >= 64 {
                        entry.closing = true;
                        entry.termination = Some(Stop::Resource);
                        cancel(
                            driver,
                            &self.owner,
                            &stream,
                            StreamAction::Cancel(CancellationReason::ResourceLimit),
                        );
                    } else {
                        entry.pending.push_back(Input::End);
                    }
                }
                Ok(None)
            }
            Event::StreamClosed { stream, outcome } if self.owns(&stream) => {
                if let Some(entry) = self.entries.iter_mut().find(|e| e.stream == stream) {
                    close(entry, outcome);
                }
                Ok(None)
            }
            Event::ConnectionClosed {
                connection,
                failure,
            } => {
                self.connections
                    .retain(|c| c.context.connection != connection);
                for entry in &mut self.entries {
                    if entry.stream.connection_id() == &connection && entry.closed.is_none() {
                        close(
                            entry,
                            StreamOutcome::Failed(failure.unwrap_or(
                                crate::modules::transport::api::Failure {
                                    origin: crate::modules::transport::api::FailureOrigin::Local,
                                    code:
                                        crate::modules::transport::api::FailureCode::ConnectionLost,
                                },
                            )),
                        );
                    }
                }
                Ok(Some(Event::ConnectionClosed {
                    connection,
                    failure,
                }))
            }
            other => Ok(Some(other)),
        }
    }
}
