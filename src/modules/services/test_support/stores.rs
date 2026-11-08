//! Byte-image-only persistence substitutes for fresh coordinator and real Ledger instances.
use super::Cut;
use crate::modules::{
    ledger,
    services::{
        self,
        persistence::{Persistence, Record},
    },
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
#[derive(Clone, Default)]
pub(in crate::modules::services) struct Services {
    record: Arc<Mutex<Option<Record>>>,
    cuts: Arc<Mutex<VecDeque<Cut>>>,
}
impl Services {
    // Export only a copied durable image for independent-process protected-store tests.
    #[cfg(target_os = "linux")]
    pub(in crate::modules::services) fn image(&self) -> Option<Record> {
        self.record.lock().unwrap().clone()
    }
    // Configure exact next write boundaries; remaining writes acknowledge normally.
    pub(in crate::modules::services) fn script(&self, cuts: impl IntoIterator<Item = Cut>) {
        *self.cuts.lock().unwrap() = cuts.into_iter().collect();
    }
}
impl Persistence for Services {
    // Each fresh owner receives only a complete independent durable byte image.
    fn load(&mut self) -> Result<Option<Record>, services::Error> {
        Ok(self.record.lock().unwrap().clone())
    }
    // Possible commitment installs bytes before dropping the scripted acknowledgement.
    fn commit(&mut self, r: &Record) -> Result<(), services::Error> {
        let cut = self.cuts.lock().unwrap().pop_front().unwrap_or(Cut::None);
        if matches!(cut, Cut::Before) {
            return Err(services::Error::Storage);
        }
        *self.record.lock().unwrap() = Some(r.clone());
        if matches!(cut, Cut::After) {
            Err(services::Error::Unknown)
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Default)]
pub(in crate::modules::services) struct Ledger {
    record: Arc<Mutex<Option<ledger::persistence::Record>>>,
    cuts: Arc<Mutex<VecDeque<Cut>>>,
}
impl Ledger {
    // Export only a copied Ledger byte image, never private balances or locks.
    #[cfg(target_os = "linux")]
    pub(in crate::modules::services) fn image(&self) -> Option<ledger::persistence::Record> {
        self.record.lock().unwrap().clone()
    }
    // Faults affect only the real Ledger persistence boundary, never private accounting state.
    pub(in crate::modules::services) fn script(&self, cuts: impl IntoIterator<Item = Cut>) {
        *self.cuts.lock().unwrap() = cuts.into_iter().collect();
    }
}
impl ledger::persistence::Persistence for Ledger {
    // Reopening the real module decodes its own private invariant-checked bytes.
    fn load(&mut self) -> Result<Option<ledger::persistence::Record>, ledger::Error> {
        Ok(self.record.lock().unwrap().clone())
    }
    // A known refusal preserves old bytes; possible commitment requires reload in a fresh Ledger owner.
    fn commit(&mut self, r: &ledger::persistence::Record) -> Result<(), ledger::Error> {
        let cut = self.cuts.lock().unwrap().pop_front().unwrap_or(Cut::None);
        if matches!(cut, Cut::Before) {
            return Err(ledger::Error::Storage);
        }
        *self.record.lock().unwrap() = Some(r.clone());
        if matches!(cut, Cut::After) {
            Err(ledger::Error::Unknown)
        } else {
            Ok(())
        }
    }
}
