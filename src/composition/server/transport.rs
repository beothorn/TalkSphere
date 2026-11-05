//! Real server Transport composition; constructing the driver does not bind listeners.
use super::{Failure, Stage, config::Config, credentials::Credentials};
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
    /// Application lifecycle stage; Ready follows successful listener binding.
    pub stage: Stage,
    pub(super) connections: Vec<crate::modules::transport::handles::ConnectionHandle>,
    pub(super) contacts_started: bool,
    pub(super) shutdown_deadline: Option<std::time::Instant>,
    pub(super) listener: Option<crate::modules::transport::handles::ListenerHandle>,
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
        stage: Stage::Starting,
        listener: None,
        connections: Vec::new(),
        contacts_started: false,
        shutdown_deadline: None,
    })
}

/// Complete public readiness result; addresses are the actual OS-bound endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ready {
    /// Canonical local public identity.
    pub local_id: IdentityId,
    /// Actual listening IP and assigned port.
    pub address: std::net::SocketAddr,
}
impl Transport {
    /// Bind the configured listener and report readiness only after successful admission.
    /// # Errors
    /// Repeated/late startup returns Runtime; bind or listener admission failure returns Network.
    pub fn listen(&mut self) -> Result<Ready, Failure> {
        use crate::modules::transport::api::{Accepted, Request, TransportControl};
        tracing::trace!("Binding the configured server listener");
        if self.stage != Stage::Starting || self.listener.is_some() {
            return Err(Failure::Runtime);
        }
        let Accepted::Listener { handle, local_addr } = self
            .driver
            .submit(Request::Listen {
                endpoint: self.config.options().listen,
            })
            .map_err(|_| Failure::Network)?
        else {
            return Err(Failure::Runtime);
        };
        self.listener = Some(handle);
        self.stage = Stage::Ready;
        tracing::info!(address=%local_addr,"Server listener is ready");
        Ok(Ready {
            local_id: self.local_id,
            address: local_addr,
        })
    }
}
