//! Worker-owned claim redemption joins a real issuer Ledger and an explicitly installed protected decryption port.
use crate::modules::{ledger, peer_protocol as peer};
use std::{sync::Arc, time::Instant};

/// Protected incoming claim capability retained by trusted node composition.
/// Clones of its dependencies share the original Ledger and protected key owner.
pub struct Claims<P: ledger::persistence::Persistence, D: ledger::redemption::Decryptor> {
    owner: Arc<ledger::state::Ledger<P>>,
    ledger: Arc<ledger::authority::Authority>,
    peer: Arc<peer::authority::Authority>,
    decryptor: Arc<D>,
    limits: peer::contract::Limits,
}
impl<P: ledger::persistence::Persistence, D: ledger::redemption::Decryptor> Claims<P, D> {
    /// Join original local owners without decrypting, issuing credits or opening a stream.
    /// # Errors
    /// Invalid finite wire limits fail before the adapter can be installed.
    pub fn new(
        owner: Arc<ledger::state::Ledger<P>>,
        ledger: Arc<ledger::authority::Authority>,
        peer: Arc<peer::authority::Authority>,
        decryptor: Arc<D>,
        limits: peer::contract::Limits,
    ) -> Result<Self, peer::Error> {
        tracing::trace!("Composing protected issuer claim redemption dependencies");
        limits.validate()?;
        Ok(Self {
            owner,
            ledger,
            peer,
            decryptor,
            limits,
        })
    }
}
impl<P: ledger::persistence::Persistence, D: ledger::redemption::Decryptor + Send + Sync>
    peer::engine::endpoint::Claims for Claims<P, D>
{
    // The bridge invokes this on a worker; caller identity and lifetime come from the authenticated original stream.
    fn redeem(
        &self,
        association: peer::contract::Association,
        deadline: Instant,
        message: &peer::message::Message,
    ) -> Result<peer::record::Record, peer::Error> {
        tracing::trace!(
            "Redeeming a claim with original issuer and protected decryption authority"
        );
        let local = self.owner.local_identity();
        let pg = self.peer.grant(
            local,
            peer::authority::Action::Invoke,
            self.limits.state_bytes,
        )?;
        let lg = self.ledger.grant(
            local,
            ledger::authority::Action::Control,
            self.limits.state_bytes,
        )?;
        peer::ledger_control::claim::respond(
            &peer::contract::Context {
                version: 1,
                grant: &pg,
                deadline,
                cancelled: false,
            },
            &self.peer,
            association,
            message,
            peer::ledger_control::claim::Ports {
                owner: &self.owner,
                control: &ledger::contract::Context {
                    version: 1,
                    grant: &lg,
                    deadline,
                    cancelled: false,
                },
                decryptor: self.decryptor.as_ref(),
            },
            self.limits,
        )
    }
}
