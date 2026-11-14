//! Fair local output admission and correlated asynchronous dependency completion.
use super::*;
impl Bridge {
    /// Progress bounded completions, output admissions and fair dependency scheduling.
    /// Call after each Transport poll, even if no application event was emitted.
    /// # Errors
    /// Lost/corrupt worker correlation is Unknown; inspect original durable owners before recovery.
    pub fn poll(
        &mut self,
        driver: &mut impl TransportControl,
        context: &mut Context<'_>,
    ) -> Result<(), Error> {
        tracing::trace!("Advancing bounded peer workers and Transport output");
        for index_in_turn in 0..64 {
            let Poll::Ready(reply) = self.pool.poll(context) else {
                break;
            };
            if index_in_turn == 63 {
                context.waker().wake_by_ref();
            }
            let completion = reply.map_err(|_| Error::Unknown)?;
            let Some(index) = self
                .entries
                .iter()
                .position(|e| e.flight == Some(completion.ticket))
            else {
                return Err(Error::Unknown);
            };
            let entry = &mut self.entries[index];
            entry.flight = None;
            entry.input_bytes = entry
                .input_bytes
                .checked_sub(entry.flight_bytes)
                .ok_or(Error::Unknown)?;
            entry.flight_bytes = 0;
            match completion.result {
                Ok(returned) if returned.id == entry.id => {
                    if returned.cleanup {
                        if let Err(error) = returned.result {
                            tracing::warn!(
                                ?error,
                                "Peer closure reconciliation failed; consult the original durable owners before recovery"
                            );
                        }
                        self.entries.remove(index);
                        continue;
                    }
                    entry.state = returned.state;
                    if let Err(error) = returned.result {
                        entry.pending.clear();
                        entry.input_bytes = 0;
                        entry.closing = true;
                        let reason = match error {
                            Error::Invalid | Error::Conflict => Stop::Protocol,
                            Error::Limit => Stop::Resource,
                            Error::Cancelled => Stop::Cancelled,
                            _ => Stop::Internal,
                        };
                        entry.termination = Some(reason);
                        cancel(driver, &self.owner, &entry.stream, reason.action());
                    }
                }
                _ => {
                    tracing::error!(
                        "Peer dependency worker lost transient stream state; consult durable owners for possible committed effects"
                    );
                    entry.closing = true;
                    entry.termination = Some(Stop::Internal);
                    cancel(
                        driver,
                        &self.owner,
                        &entry.stream,
                        StreamAction::Cancel(CancellationReason::InternalError),
                    );
                    // A panic can lose transient state after durable effects; retain no invented rollback.
                    if entry.closed.is_some() {
                        self.entries.remove(index);
                    }
                }
            }
        }
        self.entries.retain(|entry| {
            !(entry.closed.is_some() && entry.state.is_none() && entry.flight.is_none())
        });
        let now = Instant::now();
        for entry in &mut self.entries {
            if now >= entry.deadline && !entry.closing && entry.closed.is_none() {
                entry.closing = true;
                entry.termination = Some(Stop::Cancelled);
            }
            if entry.closed.is_none()
                && let Some(reason) = entry.termination
            {
                match driver.submit(Request::Stream {
                    owner: self.owner.clone(),
                    stream: entry.stream.clone(),
                    action: reason.action(),
                }) {
                    Ok(Accepted::Queued) => entry.termination = None,
                    Err(TransportError::Backpressure) => {}
                    _ => {}
                }
            }
            let Some(mut state) = entry.state.take() else {
                continue;
            };
            if entry.closed.is_some() {
                state.output.queue.cancel();
            } else if !entry.closing {
                flush(driver, &self.owner, entry, &mut state, context)?;
            }
            if entry.flight.is_some() {
                return Err(Error::Unknown);
            }
            if entry.closed.is_none() && (entry.closing || state.output.queue.pending() != 0) {
                entry.state = Some(state);
                continue;
            }
            let input = entry
                .pending
                .pop_front()
                .or_else(|| entry.closed.map(Input::Closed));
            let Some(input) = input else {
                entry.state = Some(state);
                continue;
            };
            if let Input::Data(bytes, _) = &input {
                entry.flight_bytes = bytes.len();
            }
            let id = entry.id;
            match self.pool.submit(Box::new(move || Ok(state.run(id, input)))) {
                Ok(ticket) => entry.flight = Some(ticket),
                Err((_error, _owned_job)) => {
                    entry.closing = true;
                    entry.termination = Some(Stop::Internal);
                    cancel(
                        driver,
                        &self.owner,
                        &entry.stream,
                        StreamAction::Cancel(CancellationReason::InternalError),
                    );
                }
            }
        }
        Ok(())
    }
}
// Give each stream at most one bounded DATA admission per turn, then queue END only after complete drain.
fn flush(
    driver: &mut impl TransportControl,
    owner: &StreamOwner,
    entry: &mut Entry,
    state: &mut State,
    context: &Context<'_>,
) -> Result<(), Error> {
    tracing::trace!("Admitting a fair bounded peer output fragment");
    let mut progressed = false;
    if let Some(bytes) = state.output.queue.front(65_536) {
        let count = bytes.len();
        match driver.submit(Request::Stream {
            owner: owner.clone(),
            stream: entry.stream.clone(),
            action: StreamAction::Data(bytes.to_vec()),
        }) {
            Ok(Accepted::Queued) => {
                state.output.queue.advance(count)?;
                if state.output.queue.pending() == 0 {
                    state.output.drained_at = Some(Instant::now());
                }
                progressed = true;
            }
            Err(TransportError::Backpressure) => return Ok(()),
            _ => {
                entry.closing = true;
                entry.termination = Some(Stop::Internal);
                cancel(
                    driver,
                    owner,
                    &entry.stream,
                    StreamAction::Cancel(CancellationReason::InternalError),
                );
                return Ok(());
            }
        }
    }
    if state.output.queue.pending() != 0 {
        context.waker().wake_by_ref();
        return Ok(());
    }
    if state.output.end && !entry.end_sent {
        match driver.submit(Request::Stream {
            owner: owner.clone(),
            stream: entry.stream.clone(),
            action: StreamAction::End,
        }) {
            Ok(Accepted::Queued) => {
                entry.end_sent = true;
                progressed = true;
            }
            Err(TransportError::Backpressure) => {}
            _ => {
                entry.closing = true;
                entry.termination = Some(Stop::Internal);
                cancel(
                    driver,
                    owner,
                    &entry.stream,
                    StreamAction::Cancel(CancellationReason::InternalError),
                );
            }
        }
    } else if progressed
        && !state.output.end
        && !entry
            .pending
            .iter()
            .any(|input| matches!(input, Input::Writable))
    {
        entry.pending.push_back(Input::Writable);
    }
    // Transport may have no application event for these newly admitted writes.
    if progressed {
        context.waker().wake_by_ref();
    }
    Ok(())
}
