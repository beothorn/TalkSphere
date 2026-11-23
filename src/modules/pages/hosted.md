# Hosted pages

**Building** (`build(signer, draft, maximumResources, entropy)`): each resource becomes a signed `resource` publication (sequence zero, a fresh random publication ID) and the manifest a signed `page` publication with the
page's sequence. For a restricted page every stored content, resource or manifest, is the envelope sealed for the audience's keys (`Audience::Restricted` lists reader identities in ascending order with their verified X25519
keys); the manifest's size and hash for a restricted resource describe the stored (encrypted) bytes. Empty content, a manifest that breaks the page rules, an empty restricted audience or missing randomness fail.

**Assembling** (`assemble(manifestPublication, expectedPublisher, reader, fetch)`): the manifest publication must verify against the intended publisher and be a `page`; when restricted the reader's key opens it (otherwise
`NotAvailable`, with no plaintext); the manifest must decode. Each named resource is fetched from any host (`Fetch`) and accepted only when it is signed by the same publisher, has kind `resource`, the same
visibility and audience as the manifest publication, the manifest entry's publication ID, size and hash, verifies, and (when restricted) decrypts. A resource that fails any check is `rejected` and not part of the page; one
no host supplies is `missing`, which makes the page incomplete, never corrupt. `Page::is_complete()` is true only with nothing missing or rejected; the verified resources are available by path.
