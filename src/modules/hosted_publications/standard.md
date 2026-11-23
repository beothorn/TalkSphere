# Hosted Publications standard definition

`Catalogue::new(limits)` loads the pinned `cr2se.hosted-publication` version-one service definition (copied from the proposal's service-definition
section into `fixtures/definition.json`) under local Board document budgets, without registering or publishing any capability. `definition(offering)`
returns that definition bound to the offering's own ID and description (`Unsupported` for another service or version, `Limit` for an oversized ID or
description). As the Services `Standards` hook, `validate(definition)` accepts a definition only when its input, output, check input and check output
schemas equal the pinned ones in field names, types and optionality; inert prose does not matter, an added optional field is a compatible extension and
an added required field is refused. Economics (`pricing`, `checkPrice`, `info`, `creditIssuer`) belong to the offering and never to the definition.
