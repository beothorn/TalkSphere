# Messaging standard catalogue

`Catalogue::new(limits)` loads the independently pinned complete section 16 definition (`fixtures/definition.json`, embedded at build time)
under local Board document budgets. `definition(offering)` builds the definition bound to an offering's own ID and description; it checks
schema identity only (`cr2se.messaging`, version 1) and fails `Unsupported` for another family or version and `Limit` for oversized fields.
As a Services `Standards` implementation, `validate` reparses a supplied definition under independent budgets and compares the known field
types and optionality recursively with the pinned template: a changed type, a missing field or an added required field is refused, an added
optional field is a compatible extension, and economics (`pricing`, `price`, `checkPrice`, `info`, `creditIssuer`) must stay on the offering. The
catalogue installs no provider or requester code and advertises nothing. Fixture provenance: [fixtures/README.md](fixtures/README.md).
