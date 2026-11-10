//! Structural control message fields; exchange order and economic authorization remain separate contracts.
use super::{Error, base64, control::Control, scalar};
use crate::modules::identity::text;
use serde_json::Value;
/// Version-one known control discriminators, separate from diagnostic strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Wire board.get message.
    BoardGet,
    /// Wire service.get message.
    ServiceGet,
    /// Wire board message.
    Board,
    /// Wire definition message.
    Definition,
    /// Wire offer message.
    Offer,
    /// Wire propose message.
    Propose,
    /// Wire quote message.
    Quote,
    /// Wire accept message.
    Accept,
    /// Wire input.end message.
    InputEnd,
    /// Wire result message.
    Result,
    /// Wire result.end message.
    ResultEnd,
    /// Wire failure message.
    Failure,
    /// Wire ledger.balance message.
    LedgerBalance,
    /// Wire ledger.balance.result message.
    LedgerBalanceResult,
    /// Wire ledger.transfer message.
    LedgerTransfer,
    /// Wire ledger.transfer.quote message.
    LedgerTransferQuote,
    /// Wire ledger.transfer.accept message.
    LedgerTransferAccept,
    /// Wire ledger.transfer.get message.
    LedgerTransferGet,
    /// Wire ledger.transfer.result message.
    LedgerTransferResult,
    /// Wire ledger.claim.redeem message.
    LedgerClaimRedeem,
    /// Wire ledger.claim.result message.
    LedgerClaimResult,
}
impl Kind {
    // Resolve only specified wire discriminators; an unknown tag never selects a handler.
    fn parse(tag: &str) -> Result<Self, Error> {
        tracing::trace!("Resolving specified peer control discriminator");
        match tag {
            "board.get" => Ok(Self::BoardGet),
            "service.get" => Ok(Self::ServiceGet),
            "board" => Ok(Self::Board),
            "definition" => Ok(Self::Definition),
            "offer" => Ok(Self::Offer),
            "propose" => Ok(Self::Propose),
            "quote" => Ok(Self::Quote),
            "accept" => Ok(Self::Accept),
            "input.end" => Ok(Self::InputEnd),
            "result" => Ok(Self::Result),
            "result.end" => Ok(Self::ResultEnd),
            "failure" => Ok(Self::Failure),
            "ledger.balance" => Ok(Self::LedgerBalance),
            "ledger.balance.result" => Ok(Self::LedgerBalanceResult),
            "ledger.transfer" => Ok(Self::LedgerTransfer),
            "ledger.transfer.quote" => Ok(Self::LedgerTransferQuote),
            "ledger.transfer.accept" => Ok(Self::LedgerTransferAccept),
            "ledger.transfer.get" => Ok(Self::LedgerTransferGet),
            "ledger.transfer.result" => Ok(Self::LedgerTransferResult),
            "ledger.claim.redeem" => Ok(Self::LedgerClaimRedeem),
            "ledger.claim.result" => Ok(Self::LedgerClaimResult),
            _ => Err(Error::Unsupported),
        }
    }
}
/// Validated wire control message with exact original source ownership.
pub struct Message {
    control: Control,
    kind: Kind,
}
impl Message {
    /// Independently validate required message fields and known optional representations.
    /// # Errors
    /// Invalid rejects required/null/malformed values; Unsupported rejects unknown message types.
    pub fn parse(bytes: &[u8], nodes: usize) -> Result<Self, Error> {
        tracing::trace!("Validating peer control message fields");
        let control = Control::parse(bytes, nodes)?;
        let v = control.value();
        let kind = v["type"].as_str().ok_or(Error::Invalid)?;
        let fields: &[(&str, Field)] = match kind {
            "board.get" => &[],
            "service.get" => &[("offeringId", Field::Text)],
            "board" => &[("board", Field::Object)],
            "definition" => &[("definition", Field::Object)],
            "offer" => &[
                ("offeringId", Field::Text),
                ("service", Field::Text),
                ("serviceVersion", Field::U32),
            ],
            "propose" => &[
                ("offeringId", Field::Text),
                ("publisherId", Field::Identity),
                ("service", Field::Text),
                ("serviceVersion", Field::U32),
                ("mode", Field::Mode),
                ("input", Field::Any),
                ("creditIssuer", Field::Identity),
                ("maximumPrice", Field::U64),
            ],
            "quote" => &[
                ("quoteId", Field::Fixed(32)),
                ("offering", Field::Object),
                ("creditIssuer", Field::Identity),
                ("priceMode", Field::PriceMode),
                ("maximumPrice", Field::U64),
                ("pricingInputs", Field::Object),
            ],
            "accept" => &[
                ("quoteId", Field::Fixed(32)),
                ("quoteHash", Field::Fixed(32)),
            ],
            "input.end" => &[],
            "result" => &[("output", Field::Any), ("finalPrice", Field::U64)],
            "result.end" => &[],
            "failure" => &[("code", Field::Text)],
            "ledger.balance" => &[],
            "ledger.balance.result" => &[("balance", Field::U64)],
            "ledger.transfer" => &[
                ("order", Field::Fixed(121)),
                ("ownerPublicKey", Field::Fixed(32)),
                ("signature", Field::Fixed(64)),
                ("maximumFee", Field::U64),
            ],
            "ledger.transfer.quote" => &[("transferId", Field::Fixed(16)), ("fee", Field::U64)],
            "ledger.transfer.accept" => &[("transferId", Field::Fixed(16)), ("fee", Field::U64)],
            "ledger.transfer.get" => &[
                ("ownerId", Field::Identity),
                ("transferId", Field::Fixed(16)),
            ],
            "ledger.transfer.result" => &[
                ("ownerId", Field::Identity),
                ("newOwnerId", Field::Identity),
                ("transferId", Field::Fixed(16)),
                ("amount", Field::U64),
                ("fee", Field::U64),
                ("receivedAmount", Field::U64),
            ],
            "ledger.claim.redeem" => &[("envelope", Field::Bytes)],
            "ledger.claim.result" => &[("claimId", Field::Fixed(32)), ("amount", Field::U64)],
            _ => return Err(Error::Unsupported),
        };
        for (name, field) in fields {
            validate(v.get(*name).ok_or(Error::Invalid)?, *field)?;
        }
        if matches!(kind, "propose" | "quote") {
            for (name, field) in [
                ("transferId", Field::Fixed(16)),
                ("transferFee", Field::U64),
            ] {
                if let Some(value) = v.get(name) {
                    validate(value, field)?;
                }
            }
            if v.get("transferId").is_some() != v.get("transferFee").is_some() {
                return Err(Error::Invalid);
            }
        }
        if kind == "quote" {
            if let Some(release) = v.get("releaseCredits") {
                validate(release, Field::U64)?;
            }
            for (name, value) in v["pricingInputs"].as_object().ok_or(Error::Invalid)? {
                if matches!(
                    name.as_str(),
                    "manifestBytes" | "downloadBytes" | "sizeBytes" | "payloadSize"
                ) {
                    scalar::uint64(value)?;
                }
            }
        }
        if kind == "failure"
            && v.get("message")
                .is_some_and(|m| !m.as_str().is_some_and(|s| s.len() <= 1024))
        {
            return Err(Error::Invalid);
        }
        let kind = Kind::parse(kind)?;
        Ok(Self { control, kind })
    }
    /// Read the validated discriminator; no diagnostic text controls this decision.
    pub fn kind(&self) -> Kind {
        tracing::trace!("Reading validated peer message discriminator");
        self.kind
    }
    /// Borrow exact immutable fields for the owning typed exchange adapter.
    pub fn value(&self) -> &Value {
        tracing::trace!("Borrowing structurally validated peer message");
        self.control.value()
    }
    /// Borrow a complete top-level field's exact source without reserializing its document.
    /// # Errors
    /// Absent fields return None; internal structural failure remains explicit.
    pub fn field_bytes(&self, name: &str) -> Result<Option<&[u8]>, Error> {
        tracing::trace!("Reading an exact peer message subdocument");
        self.control.field_bytes(name)
    }
    /// Borrow exact received source for quote hashing and immutable association.
    pub fn bytes(&self) -> &[u8] {
        tracing::trace!("Borrowing exact validated peer message source");
        self.control.bytes()
    }
}
#[derive(Clone, Copy)]
enum Field {
    Text,
    Object,
    Any,
    Identity,
    U32,
    U64,
    Fixed(usize),
    Bytes,
    Mode,
    PriceMode,
}
// Required control representations are checked without interpreting unknown optional siblings as authority.
fn validate(value: &Value, field: Field) -> Result<(), Error> {
    tracing::trace!("Checking one required peer control field");
    match field {
        Field::U64 => {
            scalar::uint64(value)?;
        }
        Field::U32 => {
            scalar::uint32(value)?;
        }
        Field::Text if value.as_str().is_some_and(|s| !s.is_empty()) => {}
        Field::Object if value.is_object() => {}
        Field::Any if !value.is_null() => {}
        Field::Identity => {
            let s = value.as_str().ok_or(Error::Invalid)?;
            let id = text::parse(s).map_err(|_| Error::Invalid)?;
            if text::format(id) != s {
                return Err(Error::Invalid);
            }
        }
        Field::Fixed(size) => {
            let bytes = base64::decode(value.as_str().ok_or(Error::Invalid)?, size)?;
            if bytes.len() != size {
                return Err(Error::Invalid);
            }
        }
        Field::Bytes => {
            base64::decode(
                value.as_str().ok_or(Error::Invalid)?,
                super::record::CONTROL_MAX,
            )?;
        }
        Field::Mode if matches!(value.as_str(), Some("invoke" | "check")) => {}
        Field::PriceMode if matches!(value.as_str(), Some("fixed" | "metered")) => {}
        _ => return Err(Error::Invalid),
    }
    Ok(())
}
