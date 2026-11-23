# Media types

A `MediaType` is exactly `type/subtype`: one slash, no whitespace or parameters, and a type and a subtype of 1 to 63 lowercase ASCII characters from `a-z 0-9 ! # $ & ^ _ . + -`. `parse(text)` is `Invalid` otherwise.
`is_html()` is true for exactly `text/html`, the type the root resource must have. The label describes the content and never transforms it (no negotiation, compression, charset conversion or decompression).
