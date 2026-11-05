//! Real server Transport composition; constructing the driver does not bind listeners.
use super::{Failure, config::Config, credentials::Credentials};
use crate::{
    composition::{cryptography::TransportCrypto, identity::TransportIdentity},
    modules::{
        cryptography::{provider::Provider, random::SystemEntropy},
        identity::{api::Contact, storage::FileStore},
        transport::{
            adapters::{SystemClock, TcpProvider},
            driver::Driver,
            ports::{Dependencies, identity::IdentityId},
        },
    },
};
/// Real protected Identity, Cryptography, TCP and monotonic-clock driver for this process.
pub type RealDriver = Driver<
    TcpProvider,
    SystemClock,
    TransportIdentity<FileStore, SystemEntropy>,
    TransportCrypto<SystemEntropy>,
>;
/// Trusted application owns this driver and its scoped local control authority.
pub struct Transport {
    /// Real driver; submitting requests requires trusted local composition.
    pub driver: RealDriver,
    /// Validated runtime policy retained for startup and shutdown.
    pub config: Config,
    /// Public local identity, independent of transient connection handles.
    pub local_id: IdentityId,
    /// Copied startup contacts with pinned expected IDs.
    pub contacts: Vec<Contact>,
}
/// Compose real dependencies after successful protected identity loading.
/// # Errors
/// Returns Runtime if readiness/clock initialization fails; constructor failure drops owned credentials.
/// Provider configuration failure also returns Runtime, without binding sockets.
pub fn compose(config: Config, credentials: Credentials) -> Result<Transport, Failure> {
    tracing::trace!("Composing real authenticated Transport dependencies");
    let sockets = TcpProvider::new().map_err(|_| Failure::Runtime)?;
    let clock = sockets.clock().map_err(|_| Failure::Runtime)?;
    let crypto = TransportCrypto::new(Provider::system(65548).map_err(|_| Failure::Runtime)?);
    let driver = Driver::new(
        config.options().transport,
        Dependencies {
            sockets,
            clock,
            identity: credentials.identity,
            cryptography: crypto,
        },
    );
    Ok(Transport {
        driver,
        config,
        local_id: credentials.local_id,
        contacts: credentials.contacts,
    })
}
