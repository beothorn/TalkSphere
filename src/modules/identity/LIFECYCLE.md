# Private authority lifecycle audit

The reviewed baseline is [CR2SE d40b6278](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f),
including Cryptography ownership rules and CR-09 cleanup requirements.

| Boundary | Executed evidence |
| --- | --- |
| Denied administration before entropy | creation_checks_authority_and_commits_complete_association |
| First/second entropy and failed installation | creation_failures_erase_staging_without_automatic_replacement |
| Failed cached association loading | loading_validates_without_replacement |
| Abandoned handle and failed/successful restore | lifecycle_releases_private_authority_on_abandonment_and_restore_failures |
| Signing borrows live authority without exporting keys | signing_preserves_exact_protocol_bytes_and_local_authority |
| Authenticated decryption and no provisional output | decryption_checks_recipient_authority_format_and_complete_authentication |
| Backup source remains usable after failures | backup_is_complete_authorized_and_preserves_source |
| Pre/post-install durable cuts and reconciliation | backups_obey_protected_destination_durability; restore_failure_cuts_never_replace_or_regenerate |
| Acknowledged state survives process restart | identity_persistence and identity_recovery integration tests |

Release probes observe dropping acquired Secret owners after their guarded bytes
are cleared. They do not prove erasure of compiler/register copies or native backend
stack temporaries. Complete-record buffers and returned plaintext use scoped
Zeroizing ownership. Persistent files remain protected credentials until explicitly
removed; memory cleanup does not imply physical disk erasure.

Public derivation, canonical key construction and binding use fixed-size local
native operations with no fallible dependency callback. Their API errors propagate,
but there is no meaningful backend-unavailable fault to inject after valid keys
are acquired. Entropy, malformed import, authority, bounds and storage failures
are tested at their actual fallible boundaries. Atomic records never store one
pair independently, so the separate signing/encryption write failures in ID-05
are represented by complete-record staging failures.

Operations are synchronous. Cancellation before invocation has no effects; dropping
a returned handle releases volatile authority without undoing its durable record.
A caller abandoning a result after commit must reload existing state, not generate
new keys. Filesystem tests cover abandoned staging on restart and distinguish
pre-install failure from uncertain post-install durability. Mid-call asynchronous
cancellation and process-separated key-store deployment are not supported claims.

Run `cargo test --locked modules::identity` and the two Identity integration suites
from the repository root. The final acceptance matrix tracks conformance separately.

`cargo test --locked --test identity_privacy` captures all structured fields at
TRACE across real creation, signing, encryption/decryption, backup, restore,
failures and drops. Recognizable fixture keys, private record magic, plaintext and
protocol message markers must be absent. Public QR output is exactly a textual ID.
Authority failures disclose only typed secret-free categories, before store reads.
