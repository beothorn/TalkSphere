//! Test-owned exact custom capability for isolated Board/Peer metadata composition; no executable family is installed.
use crate::modules::{
    board::{
        self,
        authority::{Authority, Grant},
        contract::Context,
        definition::Definition,
        offering::Offering,
        semantics::Capabilities,
        state::Provider,
    },
    identity::api::IdentityId,
};
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};
pub(in crate::modules::peer_protocol) const DEFINITION:&[u8]=br#"{ "id":"fixture", "service":"org.example.peer-test", "serviceVersion":1, "description":"Metadata test.", "input":{"type":"string"}, "output":{"type":"string"}, "check":{"description":"Compare locally.","input":{"type":"string"},"output":{"type":"bool"}} }"#;
struct Caps;
impl Capabilities for Caps {
    // Only the literal custom metadata fixture contract is understood.
    fn understands(&self, o: &Offering) -> bool {
        o.service == "org.example.peer-test" && o.version == 1
    }
    // Schema structure cannot redefine the independently installed fixture contract.
    fn contract(&self, _: &Offering, d: &Definition) -> Result<(), board::Error> {
        let v: serde_json::Value = serde_json::from_slice(DEFINITION).unwrap();
        if d.document.value() == &v {
            Ok(())
        } else {
            Err(board::Error::Unsupported)
        }
    }
    // The fixture provides no alternative pricing formula.
    fn terms(&self, o: &Offering) -> Result<(), board::Error> {
        if board::offering::integer(&o.source["price"]) == Some(7) {
            Ok(())
        } else {
            Err(board::Error::Unsupported)
        }
    }
    // Test authentication is explicit local context, never metadata input.
    fn precondition(&self, name: &str, authenticated: bool) -> bool {
        name == "cr2se.identity" && authenticated
    }
    // This test substitute supplies metadata only and is never used for real invocation or production registration.
    fn local(&self, _: &Offering) -> bool {
        true
    }
}
// Fixture contexts remain finite and owner issued.
pub(in crate::modules::peer_protocol) fn context(grant: &Grant) -> Context<'_> {
    Context {
        grant,
        deadline: Instant::now() + Duration::from_secs(30),
        cancelled: false,
    }
}
// Publish exact original documents through the real Board module with independent capability semantics.
pub(in crate::modules::peer_protocol) fn published() -> (Provider, Arc<Authority>, IdentityId) {
    let id = IdentityId([2; 32]);
    let a = Arc::new(Authority::new());
    let owner = Provider::new(id, a.clone(), Default::default(), Arc::new(Caps)).unwrap();
    let issuer = crate::modules::identity::text::format(id);
    let raw = format!(
        "{{\"version\":1,\"providedServices\":[{{\"id\":\"fixture\",\"service\":\"org.example.peer-test\",\"serviceVersion\":1,\"description\":\"Metadata test.\",\"price\":7,\"creditIssuer\":\"{issuer}\"}}],\"wantedServices\":[],\"unknown\":18446744073709551615}}"
    );
    let p = board::prepared::Prepared::new(
        id,
        raw.as_bytes(),
        &BTreeMap::from([("fixture".into(), DEFINITION.to_vec())]),
        Default::default(),
        &Caps,
    )
    .unwrap();
    let g = a
        .grant(id, board::authority::Action::Publish, 1_000_000)
        .unwrap();
    owner.publish(&context(&g), p).unwrap();
    (owner, a, id)
}
