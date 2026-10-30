//! Protected contact snapshots share the writer lock but never the private credential record.
use super::file::FileStore;
use crate::modules::identity::{
    api::Contact,
    authority::{Administration, Capability},
    contact,
    contacts::ContactBook,
    error::IdentityError,
};

const HEADER: &str = "TSCON001\n";

impl FileStore {
    /// Load a complete bounded contact snapshot after administrative authorization.
    /// # Errors
    /// Rejects unavailable, insecure, malformed, duplicate or oversized snapshots without repair.
    pub fn read_contacts(
        &self,
        admin: &Capability<Administration>,
    ) -> Result<Vec<Contact>, IdentityError> {
        tracing::trace!("Loading a protected configured-contact snapshot");
        self.authority
            .check_administration(admin, self.limits.record_bytes())?;
        let bytes = self.read_bytes("contacts.bin", self.limits.record_bytes())?;
        let text = std::str::from_utf8(&bytes).map_err(|_| IdentityError::InvalidInput)?;
        let body = text
            .strip_prefix(HEADER)
            .ok_or(IdentityError::Unsupported)?;
        if !body.is_empty() && !body.ends_with('\n') {
            return Err(IdentityError::InvalidInput);
        }
        let mut contacts = Vec::new();
        for line in body.split_terminator('\n') {
            if contacts.len() >= self.limits.contacts() {
                return Err(IdentityError::ResourceLimit);
            }
            let entry = contact::parse(line, self.limits.text_bytes())?;
            if contact::format(entry)? != line {
                return Err(IdentityError::InvalidInput);
            }
            contacts.push(entry);
        }
        let (mut book, access) = ContactBook::new(self.limits);
        book.replace(&access, &contacts)?;
        book.snapshot(&access)
    }

    /// Atomically replace the entire configured-contact snapshot under the exclusive store lock.
    /// # Errors
    /// Denies foreign authority and invalid or excessive snapshots before writing; uncertain durability
    /// returns OutcomeUnknown and requires reconciliation before deciding whether to retry.
    pub fn replace_contacts(
        &mut self,
        admin: &Capability<Administration>,
        contacts: &[Contact],
    ) -> Result<(), IdentityError> {
        tracing::trace!("Persisting a complete configured-contact snapshot");
        self.authority
            .check_administration(admin, self.limits.record_bytes())?;
        let (mut book, access) = ContactBook::new(self.limits);
        book.replace(&access, contacts)?;
        let mut record = String::from(HEADER);
        for entry in book.snapshot(&access)? {
            let line = contact::format(entry)?;
            if record.len().saturating_add(line.len()).saturating_add(1)
                > self.limits.record_bytes()
            {
                return Err(IdentityError::ResourceLimit);
            }
            record.push_str(&line);
            record.push('\n');
        }
        self.write_record("contacts.bin", record.as_bytes(), true)
    }

    /// Validate the currently installed snapshot and finish directory durability after an uncertain write.
    /// # Errors
    /// Preserves read failures and returns OutcomeUnknown if directory synchronization fails.
    pub fn reconcile_contacts(
        &self,
        admin: &Capability<Administration>,
    ) -> Result<Vec<Contact>, IdentityError> {
        tracing::trace!("Reconciling configured-contact snapshot durability");
        let contacts = self.read_contacts(admin)?;
        self.directory
            .sync_all()
            .map_err(|_| IdentityError::OutcomeUnknown)?;
        Ok(contacts)
    }
}
