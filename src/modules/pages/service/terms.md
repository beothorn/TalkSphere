# Page terms

`Terms::parse(offering)`: a provided `cr2se.page` version-one offering with the Board's fixed `price` (positive), `info.maximumPathBytes` (1 to 65535) and `info.maximumResourceBytes` (positive), the `cr2se.identity`
precondition, no `pricing` object and no `checkPrice` (the Page check is local and included). The credit issuer need not be the publisher. `Unsupported` for another service or version; `Invalid` for any missing, zero,
fractional, out-of-range or forbidden value or a wanted offering.
