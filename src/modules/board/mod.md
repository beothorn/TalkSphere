# Board module contract

Board exposes authority, resource configuration and direct boundary contracts.
The error categories are Invalid, Unsupported, Denied, Limit, Missing, Stale,
Cancelled, Storage (known precommit failure) and Unknown (possible commitment
requiring recovery). Errors carry no document contents or secrets.

Current boundary reads fail explicitly with Unsupported after validating caller
control. Other operations are not yet exposed or advertised as working.
