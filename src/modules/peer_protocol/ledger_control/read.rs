//! Caller-scoped balance and protected transfer-result queries perform no Services work or accounting mutation.
use crate::modules::{
    identity::text,
    ledger::{self, persistence::Persistence, state::Ledger, transfer_state::TransferResult},
    peer_protocol::{
        Error,
        authority::{Action, Authority},
        contract::{Association, Context, Limits},
        failure::{Code, Failure},
        message::{Kind, Message},
        record::Record,
    },
};
use serde_json::json;
/// Serve one independent authenticated balance or transfer-result request through exact Ledger Control authority.
/// # Errors
/// Foreign associations, wrong purpose, invalid fields, uncertain storage or response limits fail without effects; protected absence is concealed.
pub fn respond<P: Persistence>(
    context: &Context<'_>,
    authority: &Authority,
    owner: &Ledger<P>,
    ledger_context: &ledger::contract::Context<'_>,
    association: Association,
    message: &Message,
    limits: Limits,
) -> Result<Record, Error> {
    tracing::trace!("Serving a caller-scoped independent Ledger read control");
    context.check(
        authority,
        association.local,
        Action::Invoke,
        message.bytes().len(),
    )?;
    limits.validate()?;
    if owner.local_identity() != association.local
        || association.operation.requester != association.peer
        || association.operation.stream == 0
    {
        return Err(Error::Denied);
    }
    owner.check_control(ledger_context, 0)?;
    let v = message.value();
    match message.kind() {
        Kind::LedgerBalance => {
            let balance = owner.balance(ledger_context, association.peer)?;
            encode(
                &json!({"type":"ledger.balance.result","balance":balance.to_string()}),
                limits,
            )
        }
        Kind::LedgerTransferGet => {
            let original = text::parse(v["ownerId"].as_str().ok_or(Error::Invalid)?)
                .map_err(|_| Error::Invalid)?;
            let id =
                super::super::base64::decode(v["transferId"].as_str().ok_or(Error::Invalid)?, 16)?
                    .try_into()
                    .map_err(|_| Error::Invalid)?;
            match owner.transfer_result(ledger_context, association.peer, original, id) {
                Ok(Some(result)) => transfer_result(&result, limits),
                Ok(None) => Failure {
                    code: Code::NotAvailable,
                    message: None,
                }
                .record(),
                Err(ledger::Error::Denied) => {
                    owner.check_control(ledger_context, 0)?;
                    Failure {
                        code: Code::NotAvailable,
                        message: None,
                    }
                    .record()
                }
                Err(error) => Err(error.into()),
            }
        }
        _ => Err(Error::Unsupported),
    }
}
// Both query replies and committed transfer replies preserve original exact gross/fee/net and canonical identities.
pub(crate) fn transfer_result(result: &TransferResult, limits: Limits) -> Result<Record, Error> {
    tracing::trace!("Encoding an exact retained Ledger transfer result");
    result.order.encode()?;
    let net = result.received()?;
    encode(
        &json!({"type":"ledger.transfer.result","ownerId":text::format(result.order.owner),"newOwnerId":text::format(result.order.new_owner),"transferId":super::super::base64::encode(&result.order.transfer_id),"amount":result.order.amount.to_string(),"fee":result.fee.to_string(),"receivedAmount":net.to_string()}),
        limits,
    )
}
// Validate the complete control envelope before returning an owned record to bounded transmission.
pub(crate) fn encode(value: &serde_json::Value, limits: Limits) -> Result<Record, Error> {
    tracing::trace!("Encoding one bounded Ledger control response");
    let bytes = serde_json::to_vec(value).map_err(|_| Error::Invalid)?;
    Message::parse(&bytes, limits.nodes)?;
    Ok(Record::Control(bytes))
}
