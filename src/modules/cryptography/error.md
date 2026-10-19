# Cryptography failures

`CryptoError` contains no input bytes, keys, payloads or backend error strings.
It supplies these distinct local outcomes:

| Category | Meaning |
| --- | --- |
| InvalidInput | Malformed values, unsupported parameters or invalid configuration. |
| AuthenticationFailed | A signature, binding, agreement result or tag is rejected. |
| AccessDenied | Private capability use is unauthorized. |
| Unavailable | A primitive, key store or secure entropy source is unavailable or unimplemented. |
| ResourceLimit | A local budget or checked size arithmetic is exceeded. |

Formatting returns a fixed explanation of the category without underlying
secret-bearing diagnostics. These are local errors, not prescribed peer wire
codes. A composition adapter is responsible for mapping failures at its boundary.
