//! Durable test-family wrapper; every acknowledged transition snapshots bytes before returning.
use super::family::Echo;
use crate::modules::{
    board::{definition::Definition, offering::Offering},
    services::{Error, contract::OperationKey, handler::*, validation::Semantics, value::Value},
};
use std::sync::{Arc, Mutex};
pub(in crate::modules::services) struct Durable {
    echo: Echo,
    image: Arc<Mutex<Vec<u8>>>,
    #[cfg(target_os = "linux")]
    disk: Option<Mutex<crate::modules::services::linux::FileStore>>,
}
impl Durable {
    // New instances recover from bytes only; no old execution heap or lock is retained.
    pub(in crate::modules::services) fn open(image: Arc<Mutex<Vec<u8>>>) -> Result<Self, Error> {
        let bytes = image.lock().unwrap().clone();
        let echo = if bytes.is_empty() {
            Echo::new()
        } else {
            Echo::restore(&bytes)?
        };
        Ok(Self {
            echo,
            image,
            #[cfg(target_os = "linux")]
            disk: None,
        })
    }
    // Recover an independently locked protected test-family store for real process-boundary evidence.
    #[cfg(target_os = "linux")]
    pub(in crate::modules::services) fn open_disk(path: &std::path::Path) -> Result<Self, Error> {
        use crate::modules::services::persistence::Persistence;
        let mut store =
            crate::modules::services::linux::FileStore::open(path, Default::default(), false)?;
        let record = store.load()?.ok_or(Error::Unavailable)?;
        if record.identity != crate::modules::identity::api::IdentityId([2; 32]) {
            return Err(Error::Denied);
        }
        let mut family = Self::open(Arc::new(Mutex::new(record.payload)))?;
        family.disk = Some(Mutex::new(store));
        Ok(family)
    }
    // Acknowledge a complete new family image, independently of Services and Ledger stores.
    fn save(&self) -> Result<(), Error> {
        let bytes = self.echo.snapshot()?;
        #[cfg(target_os = "linux")]
        if let Some(disk) = &self.disk {
            use crate::modules::services::persistence::Persistence;
            disk.lock()
                .unwrap()
                .commit(&crate::modules::services::persistence::Record {
                    identity: crate::modules::identity::api::IdentityId([2; 32]),
                    payload: bytes.clone(),
                })?;
        }
        *self.image.lock().unwrap() = bytes;
        Ok(())
    }
    // Durable count proves recovery did not repeat execution to retrieve a lost reply.
    pub(in crate::modules::services) fn executions(&self) -> usize {
        self.echo
            .executions
            .load(std::sync::atomic::Ordering::SeqCst)
    }
}
impl Semantics for Durable {
    // Fixture semantics remain exactly the installed echo contract.
    fn validate(&self, s: &serde_json::Value, v: &Value) -> Result<(), Error> {
        self.echo.validate(s, v)
    }
    // No nonfinite values are valid echo text.
    fn nonfinite(&self) -> bool {
        false
    }
}
impl Requester for Durable {
    // Recovering implementation instances preserve the same exact contract definition.
    fn definition(&self) -> &Definition {
        self.echo.definition()
    }
    // Economic rules remain fixed7 after restart.
    fn terms(&self, o: &Offering) -> Result<(), Error> {
        self.echo.terms(o)
    }
    // Verification remains requester-local and never changes accounting.
    fn verify(&self, i: &Value, t: &Blob, r: &Value, e: Option<&Value>) -> Result<Check, Error> {
        self.echo.verify(i, t, r, e)
    }
}
impl Provider for Durable {
    // Assessment has no durable effects and cannot execute.
    fn assess(&self, p: Peer, o: &Offering, m: &Value) -> Result<Assessment, Error> {
        self.echo.assess(p, o, m)
    }
    // Persist staged completion and counter before acknowledging execution.
    fn execute(&self, i: &Invocation, v: &Value) -> Result<Completion, Error> {
        let result = self.echo.execute(i, v)?;
        self.save()?;
        Ok(result)
    }
    // Retrieve retained completion without re-execution.
    fn completion(&self, k: OperationKey) -> Result<Completion, Error> {
        self.echo.completion(k)
    }
    // Opaque prepared tokens and exclusivity survive recovery from snapshot bytes.
    fn prepare(&self, k: OperationKey, e: &Blob) -> Result<Blob, Error> {
        let t = self.echo.prepare(k, e)?;
        self.save()?;
        Ok(t)
    }
    // Persist commitment before an acknowledgement can be lost.
    fn commit(&self, k: OperationKey, t: &Blob) -> Result<(), Error> {
        self.echo.commit(k, t)?;
        self.save()
    }
    // Persist explicit abort and retained terminal guards.
    fn abort(&self, k: OperationKey, t: Option<&Blob>) -> Result<(), Error> {
        self.echo.abort(k, t)?;
        self.save()
    }
    // Status describes only reconstructed durable participant data.
    fn status(&self, k: OperationKey) -> Result<Status, Error> {
        self.echo.status(k)
    }
    // This custom fixture has no remote evidence operation.
    fn evidence(
        &self,
        i: &crate::modules::services::checks::Invocation,
        v: &Value,
    ) -> Result<Value, Error> {
        self.echo.evidence(i, v)
    }
}
