# Page standard definition

`Catalogue::new(limits)` loads the pinned `cr2se.page` version-one definition (copied from the specification's required service definition into `fixtures/definition.json`) and, as the Services `Standards` hook, accepts a
definition only when its input, output and check schemas equal the pinned ones in field names, types and optionality. Economics (`price`, `info`, `creditIssuer`) belong to the offering.
