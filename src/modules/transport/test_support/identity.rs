use super::script::Script;
use crate::modules::transport::ports::{
    DependencyError,
    identity::{AuthenticationMessage, Identity, IdentityId, PublicIdentity},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityCall {
    PublicIdentity,
    Identify([u8; 32]),
    Sign(Vec<u8>),
}

pub enum IdentityReply {
    PublicIdentity(Result<PublicIdentity, DependencyError>),
    IdentityId(Result<IdentityId, DependencyError>),
    Signature(Result<[u8; 64], DependencyError>),
}

pub struct ScriptedIdentity(pub Script<IdentityCall, IdentityReply>);

impl Identity for ScriptedIdentity {
    fn public_identity(&self) -> Result<PublicIdentity, DependencyError> {
        let IdentityReply::PublicIdentity(result) = self.0.call(IdentityCall::PublicIdentity)
        else {
            panic!("expected public identity reply")
        };
        result
    }

    fn identify(&self, key: &[u8; 32]) -> Result<IdentityId, DependencyError> {
        let IdentityReply::IdentityId(result) = self.0.call(IdentityCall::Identify(*key)) else {
            panic!("expected identity ID reply")
        };
        result
    }

    fn sign_authentication(
        &mut self,
        message: &AuthenticationMessage,
    ) -> Result<[u8; 64], DependencyError> {
        let IdentityReply::Signature(result) =
            self.0.call(IdentityCall::Sign(message.as_bytes().to_vec()))
        else {
            panic!("expected signature reply")
        };
        result
    }
}

#[cfg(test)]
mod tests {
    use super::super::vectors;
    use super::*;

    #[test]
    fn identity_records_exact_public_key_and_signing_bytes() {
        let public = PublicIdentity {
            id: IdentityId(vectors::array("initiator CR2SE ID")),
            signing_public_key: vectors::array("initiator Ed25519 public key"),
        };
        // Deliberately arbitrary test input: canonical AUTH construction is step 7.
        let message = AuthenticationMessage {
            bytes: vec![0, 255, 1],
        };
        let signature = vectors::array("initiator AUTH payload");
        let calls = vec![
            IdentityCall::PublicIdentity,
            IdentityCall::Identify(public.signing_public_key),
            IdentityCall::Sign(message.as_bytes().to_vec()),
        ];
        let script = Script::new(calls.iter().cloned().zip([
            IdentityReply::PublicIdentity(Ok(public)),
            IdentityReply::IdentityId(Ok(public.id)),
            IdentityReply::Signature(Ok(signature)),
        ]));
        let mut identity = ScriptedIdentity(script.clone());
        assert_eq!(identity.public_identity(), Ok(public));
        assert_eq!(identity.identify(&public.signing_public_key), Ok(public.id));
        assert_eq!(identity.sign_authentication(&message), Ok(signature));
        assert_eq!(script.calls(), calls);
        script.assert_finished();
    }

    #[test]
    fn identity_does_not_hide_unavailability_invalid_keys_or_denied_signing() {
        let message = AuthenticationMessage { bytes: vec![42] };
        let script = Script::new([
            (
                IdentityCall::PublicIdentity,
                IdentityReply::PublicIdentity(Err(DependencyError::Unavailable)),
            ),
            (
                IdentityCall::Identify([0; 32]),
                IdentityReply::IdentityId(Err(DependencyError::InvalidInput)),
            ),
            (
                IdentityCall::Sign(vec![42]),
                IdentityReply::Signature(Err(DependencyError::Denied)),
            ),
        ]);
        let mut identity = ScriptedIdentity(script.clone());
        assert_eq!(
            identity.public_identity(),
            Err(DependencyError::Unavailable)
        );
        assert_eq!(
            identity.identify(&[0; 32]),
            Err(DependencyError::InvalidInput)
        );
        assert_eq!(
            identity.sign_authentication(&message),
            Err(DependencyError::Denied)
        );
        script.assert_finished();
    }

    #[test]
    #[should_panic(expected = "script call mismatch")]
    fn signing_different_bytes_is_a_test_failure() {
        let mut identity = ScriptedIdentity(Script::new([(
            IdentityCall::Sign(vec![1]),
            IdentityReply::Signature(Ok([0; 64])),
        )]));
        let _ = identity.sign_authentication(&AuthenticationMessage { bytes: vec![2] });
    }
}
