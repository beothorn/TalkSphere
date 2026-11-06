# Board module contract

Board exports authority, resource limits, lossless documents, offering/definition
validation, trusted semantic ports, prepared publications, atomic local owners,
remote caches, selection and persistence. Linux supplies protected durable storage.
Services schema formats are an independent dependency; its runtime is not supplied.

Errors are Invalid, Unsupported, Denied, Limit, Missing, Stale, Cancelled,
Storage (known precommit failure) and Unknown (possible commitment requiring
recovery). Independent schema Invalid/Limit errors map to these boundary errors.
No error carries untrusted document text or secrets. Module construction does not
start networking or register capabilities; composition must supply real support.
