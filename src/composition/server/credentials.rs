//! Linux protected identity startup before any network capability is constructed.
use super::{
    Failure,
    config::{Config, Options},
};
use crate::{
    composition::identity::TransportIdentity,
    modules::{
        cryptography::random::SystemEntropy,
        identity::{
            api::{Contact, IdentityProvider},
            authority::{ReadPublic, Sign},
            config::Limits,
            provider::Identity,
            storage::FileStore,
        },
        transport::ports::identity::IdentityId,
    },
};

/// Loaded server authority retains the exclusive credential-store lock and immutable contact inputs.
pub struct Credentials {
    /// Transport-specific public-read and authentication-signing adapter.
    pub identity: TransportIdentity<FileStore, SystemEntropy>,
    /// Public local identity for readiness/status output.
    pub local_id: IdentityId,
    /// Validated endpoint-bearing startup contacts, independent of mutable storage.
    pub contacts: Vec<Contact>,
}
/// Load existing protected keys and scoped authority before binding sockets.
/// # Errors
/// Missing/corrupt/inaccessible credentials or invalid stored contacts fail as Identity; no keys are generated.
pub fn load(config: &Config, stored_contacts: bool) -> Result<Credentials, Failure> {
    tracing::trace!("Loading protected server credentials before network startup");
    let options = config.options();
    if !options.identity_store.is_dir() {
        return Err(Failure::Identity);
    }
    let limits = Limits::new(1024, 64, 65548, 65536, 65536).map_err(|_| Failure::Identity)?;
    let (store, admin) =
        FileStore::open(&options.identity_store, limits).map_err(|_| Failure::Identity)?;
    let contacts = if stored_contacts {
        if !options.contacts.is_empty() {
            return Err(Failure::Identity);
        }
        let contacts = store.read_contacts(&admin).map_err(|_| Failure::Identity)?;
        let mut validation =
            Options::local(options.identity_store.clone()).map_err(|_| Failure::Identity)?;
        validation.contacts = contacts.clone();
        validation.maximum_contacts = options.maximum_contacts;
        Config::new(validation).map_err(|_| Failure::Identity)?;
        contacts
    } else {
        options.contacts.clone()
    };
    let mut provider =
        Identity::new(store, SystemEntropy, limits, 1).map_err(|_| Failure::Identity)?;
    let (handle, public) = provider.load(&admin).map_err(|_| Failure::Identity)?;
    let read = provider
        .grant::<ReadPublic>(&admin, &handle, 1)
        .map_err(|_| Failure::Identity)?;
    let sign = provider
        .grant::<Sign>(&admin, &handle, 65548)
        .map_err(|_| Failure::Identity)?;
    let identity =
        TransportIdentity::new(provider, handle, read, sign).map_err(|_| Failure::Identity)?;
    Ok(Credentials {
        identity,
        local_id: IdentityId(public.identity.id().0),
        contacts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    // A missing identity fails before storage creation or network construction.
    #[test]
    fn missing_identity_does_not_create_state() {
        let path =
            std::env::temp_dir().join(format!("talksphere-missing-server-{}", std::process::id()));
        assert!(!path.exists());
        let config = Config::new(Options::local(path.clone()).unwrap()).unwrap();
        assert!(matches!(load(&config, false), Err(Failure::Identity)));
        assert!(!path.exists());
    }
}
