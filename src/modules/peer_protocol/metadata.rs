//! Uncharged local metadata responses over immutable public Board snapshots.
use super::{
    message::{Kind, Message},
    record::Record,
};
use crate::modules::board::{self, contract::Context, state::Provider};
/// Produce a complete bounded response under Board Read authority, retaining exact original documents.
/// Peer composition separately enforces authenticated stream association and control-rate limits.
/// # Errors
/// Board authority, cancellation, storage uncertainty and bounds remain typed; missing metadata returns not_available.
/// This function neither dispatches Services nor touches Ledger balances or network I/O.
pub fn respond(
    owner: &Provider,
    context: &Context<'_>,
    request: &Message,
) -> Result<Record, board::Error> {
    tracing::trace!("Serving uncharged bounded peer metadata through Board contracts");
    if !matches!(request.kind(), Kind::BoardGet | Kind::ServiceGet) {
        return Err(board::Error::Unsupported);
    }
    let result = (|| {
        let snapshot = owner.read(context)?;
        match request.kind() {
            Kind::BoardGet => board::response::encode_snapshot(&snapshot),
            Kind::ServiceGet => {
                let id = request.value()["offeringId"]
                    .as_str()
                    .ok_or(board::Error::Invalid)?;
                let definition = owner.definition(context, &snapshot, id)?;
                board::response::encode("definition", &definition.document)
            }
            _ => Err(board::Error::Unsupported),
        }
    })();
    match result {
        Ok(bytes) => Ok(Record::Control(bytes)),
        Err(board::Error::Missing) => Ok(Record::Control(
            br#"{"type":"failure","code":"not_available"}"#.to_vec(),
        )),
        Err(error) => Err(error),
    }
}
