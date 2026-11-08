//! Versioned bounded local call binding; trusted composition supplies authority outside encoded requests.
use super::{
    Error, codec,
    contract::{CONTRACT_VERSION, Context, Limits, OperationKey},
    persistence::Persistence,
    runtime::{Coordinator, Decision},
    value::{Budget, Value},
};
use crate::modules::identity::api::IdentityId;

/// One complete local request; accepted quote admission remains a trusted typed composition operation.
pub enum Request {
    /// Supply the complete owned input for an already accepted attempt.
    Input(OperationKey, Value),
    /// Dispatch original provider work once.
    Start(OperationKey),
    /// Retain the provider's complete result.
    Stage(OperationKey),
    /// Prepare local effects.
    Prepare(OperationKey),
    /// Persist one immutable decision.
    Decide(OperationKey, Decision),
    /// Deliver an existing commit.
    Commit(OperationKey),
    /// Deliver an existing abort.
    Abort(OperationKey),
    /// Inspect retained phase without effects.
    Status(OperationKey),
    /// Copy the complete committed result, exact bytes and final price.
    Result(OperationKey),
}
/// Bounded synchronous adapter; callers own request/reply buffers and authenticated local session context.
pub struct Adapter<'a, P: Persistence> {
    coordinator: &'a Coordinator<P>,
    limits: Limits,
}
impl<'a, P: Persistence> Adapter<'a, P> {
    /// Bind a coordinator with finite message budgets; no socket, session or authority is created.
    /// # Errors
    /// Rejects invalid limits; smaller budgets may refuse replies while retained results remain available.
    pub fn new(coordinator: &'a Coordinator<P>, limits: Limits) -> Result<Self, Error> {
        tracing::trace!("Creating bounded local Services call adapter");
        limits.validate()?;
        Ok(Self {
            coordinator,
            limits,
        })
    }
    /// Encode one exact request under the local version and finite aggregate budgets.
    /// # Errors
    /// Oversized values and malformed objects fail before returning a buffer.
    pub fn encode(&self, request: Request) -> Result<Vec<u8>, Error> {
        tracing::trace!("Encoding one complete local Services request");
        let (tag, key, body) = match request {
            Request::Input(k, v) => (0, k, v),
            Request::Start(k) => (1, k, Value::Bool(false)),
            Request::Stage(k) => (2, k, Value::Bool(false)),
            Request::Prepare(k) => (3, k, Value::Bool(false)),
            Request::Decide(k, d) => (4, k, Value::Bool(d == Decision::Commit)),
            Request::Commit(k) => (5, k, Value::Bool(false)),
            Request::Abort(k) => (6, k, Value::Bool(false)),
            Request::Status(k) => (7, k, Value::Bool(false)),
            Request::Result(k) => (8, k, Value::Bool(false)),
        };
        codec::encode(
            &Value::Array(vec![
                Value::Uint32(CONTRACT_VERSION),
                Value::Uint32(tag),
                Value::Bytes(key.requester.0.to_vec()),
                Value::Bytes(key.connection.to_vec()),
                Value::Uint32(key.stream),
                body,
            ]),
            self.budget(),
            self.limits.state_bytes,
        )
    }
    /// Dispatch a complete request through the same public coordinator methods as direct calls.
    /// Context is supplied out of band by trusted local composition and cannot be decoded from peer bytes.
    /// # Errors
    /// Rejects unsupported versions, malformed/truncated messages, foreign grants and underlying contract failures.
    /// Reply allocation failure after a mutation returns Unknown: inspect original-key status, never rerun work.
    pub fn dispatch(&self, context: &Context<'_>, bytes: &[u8]) -> Result<Vec<u8>, Error> {
        tracing::trace!("Dispatching one bounded authenticated local Services request");
        if context.version != CONTRACT_VERSION {
            return Err(Error::Unsupported);
        }
        let value = codec::decode(bytes, self.budget(), self.limits.state_bytes)?;
        let Value::Array(fields) = value else {
            return Err(Error::Invalid);
        };
        let [
            Value::Uint32(version),
            Value::Uint32(tag),
            Value::Bytes(requester),
            Value::Bytes(connection),
            Value::Uint32(stream),
            body,
        ] = fields.as_slice()
        else {
            return Err(Error::Invalid);
        };
        if *version != CONTRACT_VERSION {
            return Err(Error::Unsupported);
        }
        let key = OperationKey {
            requester: IdentityId(
                requester
                    .as_slice()
                    .try_into()
                    .map_err(|_| Error::Invalid)?,
            ),
            connection: connection
                .as_slice()
                .try_into()
                .map_err(|_| Error::Invalid)?,
            stream: *stream,
        };
        if key.stream == 0 || (*tag != 0 && *tag != 4 && body != &Value::Bool(false)) {
            return Err(Error::Invalid);
        }
        let reply = match tag {
            0 => {
                self.coordinator.supply_input(context, key, body.clone())?;
                Value::Bool(true)
            }
            1 => {
                self.coordinator.start(context, key)?;
                Value::Bool(true)
            }
            2 => {
                self.coordinator.stage(context, key)?;
                Value::Bool(true)
            }
            3 => {
                self.coordinator.prepare(context, key)?;
                Value::Bool(true)
            }
            4 => {
                let Value::Bool(commit) = body else {
                    return Err(Error::Invalid);
                };
                self.coordinator.decide(
                    context,
                    key,
                    if *commit {
                        Decision::Commit
                    } else {
                        Decision::Abort
                    },
                )?;
                Value::Bool(true)
            }
            5 => {
                self.coordinator.commit(context, key)?;
                Value::Bool(true)
            }
            6 => {
                self.coordinator.abort(context, key)?;
                Value::Bool(true)
            }
            7 => Value::String(
                self.coordinator
                    .status(context, key)?
                    .map_or_else(|| "absent".into(), |phase| format!("{phase:?}")),
            ),
            8 => {
                let c = self.coordinator.result(context, key)?;
                Value::Array(vec![
                    c.output,
                    Value::Bytes(c.result.bytes().to_vec()),
                    Value::Uint64(c.price),
                ])
            }
            _ => return Err(Error::Unsupported),
        };
        codec::encode(&reply, self.budget(), self.limits.state_bytes)
            .map_err(|e| if *tag <= 6 { Error::Unknown } else { e })
    }
    /// Decode one owned complete reply; no borrowed transport buffer outlives the call.
    /// # Errors
    /// Applies the same finite value, message and exact-width decoding rules as requests.
    pub fn decode(&self, bytes: &[u8]) -> Result<Value, Error> {
        tracing::trace!("Decoding complete owned local Services reply");
        codec::decode(bytes, self.budget(), self.limits.state_bytes)
    }
    // Keep structural and message budgets independent and bounded before any decode allocation.
    fn budget(&self) -> Budget {
        Budget {
            depth: 31,
            nodes: self.limits.nodes,
            bytes: self.limits.state_bytes,
        }
    }
}
