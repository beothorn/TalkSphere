//! Authorized bounded in-memory contact ownership; durable snapshots are supplied by a later adapter.
use super::{
    api::{Contact, IdentityId},
    config::Limits,
    error::IdentityError,
    ports::ContactStore,
};
use std::{collections::BTreeMap, sync::Arc};

/// Opaque local contact authority issued only by the matching book's constructor.
#[derive(Clone)]
pub struct ContactAccess(Arc<()>);
/// Configured contacts keyed by expected binary ID, independent of Discovery or trust.
pub struct ContactBook {
    access: Arc<()>,
    limits: Limits,
    entries: BTreeMap<IdentityId, Contact>,
}
impl ContactBook {
    /// Construct an empty local book and its authority for trusted application composition.
    /// The authority is not a peer credential; keep it inside authorized local adapters.
    pub fn new(limits: Limits) -> (Self, ContactAccess) {
        tracing::trace!("Creating an authorized in-memory contact book");
        let access = Arc::new(());
        (
            Self {
                access: access.clone(),
                limits,
                entries: BTreeMap::new(),
            },
            ContactAccess(access),
        )
    }
    /// Return immutable copied contacts in binary-ID order after authorization.
    /// # Errors
    /// Denies foreign authority without disclosing entries.
    pub fn snapshot(&self, authority: &ContactAccess) -> Result<Vec<Contact>, IdentityError> {
        tracing::trace!("Reading an authorized contact snapshot");
        self.authorize(authority)?;
        Ok(self.entries.values().copied().collect())
    }
    /// Import a complete contact snapshot with no duplicate IDs or partial replacement.
    /// # Errors
    /// Denies foreign authority, duplicate IDs, invalid endpoints or exceeded limits.
    pub fn replace(
        &mut self,
        authority: &ContactAccess,
        contacts: &[Contact],
    ) -> Result<(), IdentityError> {
        tracing::trace!("Replacing a complete bounded contact snapshot");
        self.authorize(authority)?;
        if contacts.len() > self.limits.contacts() {
            return Err(IdentityError::ResourceLimit);
        }
        let mut next = BTreeMap::new();
        for contact in contacts {
            self.validate(*contact)?;
            if next.insert(contact.expected_id, *contact).is_some() {
                return Err(IdentityError::Conflict);
            }
        }
        self.entries = next;
        Ok(())
    }
    // Reject a capability from another book before checking its entries or capacity.
    fn authorize(&self, authority: &ContactAccess) -> Result<(), IdentityError> {
        tracing::trace!("Checking contact-book authority");
        if !Arc::ptr_eq(&self.access, &authority.0) {
            return Err(IdentityError::AccessDenied);
        }
        Ok(())
    }
    // Check the exact public contact shape and bounded canonical textual representation.
    fn validate(&self, contact: Contact) -> Result<(), IdentityError> {
        tracing::trace!("Validating a configured contact hint");
        if super::contact::format(contact)?.len() > self.limits.text_bytes() {
            return Err(IdentityError::ResourceLimit);
        }
        Ok(())
    }
}
impl ContactStore for ContactBook {
    type Authority = ContactAccess;
    /// Return a copied optional hint after checking book-specific local authority.
    fn get(
        &self,
        authority: &ContactAccess,
        id: IdentityId,
    ) -> Result<Option<Contact>, IdentityError> {
        tracing::trace!("Looking up an authorized configured contact");
        self.authorize(authority)?;
        Ok(self.entries.get(&id).copied())
    }
    /// Add or update the endpoint for this ID without changing any other identity's entry.
    fn put(&mut self, authority: &ContactAccess, contact: Contact) -> Result<(), IdentityError> {
        tracing::trace!("Updating an authorized configured contact");
        self.authorize(authority)?;
        self.validate(contact)?;
        if !self.entries.contains_key(&contact.expected_id)
            && self.entries.len() >= self.limits.contacts()
        {
            return Err(IdentityError::ResourceLimit);
        }
        self.entries.insert(contact.expected_id, contact);
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    // Snapshots are copied, duplicate replacements are atomic, and denied/capacity failures preserve existing hints.
    #[test]
    fn bounded_authorized_snapshots_preserve_expected_ids() {
        let limits = Limits::new(256, 1, 1024, 2048, 4096).unwrap();
        let (mut book, access) = ContactBook::new(limits);
        let (_, foreign) = ContactBook::new(limits);
        let hint = Contact {
            expected_id: IdentityId([1; 32]),
            endpoint: None,
        };
        assert_eq!(
            book.get(&foreign, hint.expected_id),
            Err(IdentityError::AccessDenied)
        );
        assert_eq!(book.put(&foreign, hint), Err(IdentityError::AccessDenied));
        assert_eq!(book.snapshot(&foreign), Err(IdentityError::AccessDenied));
        assert_eq!(
            book.replace(&foreign, &[]),
            Err(IdentityError::AccessDenied)
        );
        assert_eq!(book.get(&access, hint.expected_id), Ok(None));
        book.put(&access, hint).unwrap();
        let old = book.snapshot(&access).unwrap();
        let updated = Contact {
            endpoint: Some("127.0.0.1:1".parse().unwrap()),
            ..hint
        };
        book.put(&access, updated).unwrap();
        assert_eq!(old, vec![hint]);
        assert_eq!(book.get(&access, hint.expected_id), Ok(Some(updated)));
        assert_eq!(
            book.put(
                &access,
                Contact {
                    expected_id: IdentityId([2; 32]),
                    ..hint
                }
            ),
            Err(IdentityError::ResourceLimit)
        );
        assert_eq!(
            book.replace(&access, &[hint, hint]),
            Err(IdentityError::ResourceLimit)
        );
        assert_eq!(
            book.put(
                &access,
                Contact {
                    endpoint: Some("127.0.0.1:0".parse().unwrap()),
                    ..hint
                }
            ),
            Err(IdentityError::InvalidInput)
        );
        assert_eq!(book.get(&access, hint.expected_id), Ok(Some(updated)));
        book.replace(&access, &[]).unwrap();
        assert!(book.snapshot(&access).unwrap().is_empty());
        let (mut book, access) = ContactBook::new(Limits::new(58, 2, 1, 148, 1).unwrap());
        assert_eq!(
            book.replace(&access, &[hint, hint]),
            Err(IdentityError::Conflict)
        );
        assert_eq!(
            book.put(&access, updated),
            Err(IdentityError::ResourceLimit)
        );
        book.replace(&access, &[hint]).unwrap();
        assert_eq!(book.snapshot(&access).unwrap(), vec![hint]);
    }
}
