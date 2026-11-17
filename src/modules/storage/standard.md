# Independent Storage standard catalogue

`Catalogue` implements the public Services Standards contract for Storage version
1. Construction loads the complete definition embedded from pinned
[CR2SE section 13](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/README.md#13-required-service-definition),
under explicit Board document, depth, node and string budgets. It has no network,
filesystem, registration, publication or payment side effects. Remote descriptions
cannot replace the embedded standard or install executable code.

`definition(offering)` returns a complete Board Definition with the offering's
own ID and description and the pinned standard schemas. Both provided and wanted
initial-store directions use the same schema. A different service/version is
Unsupported. Economic terms, authenticated selection and actual handler support
remain separate checks. Serialization stops before the document byte bound,
including expansion caused by JSON escaping; oversized strings and documents
return Limit. Offering identity mismatches remain Invalid through Board parsing.

`validate(definition)` reparses complete original bytes under independent local
budgets, checks family/version, nonempty identity/prose fields, all input/output
and check schemas, and forbids economic offering fields inside a definition.
A public Known marker cannot bypass actual descriptor validation. It compares
all pinned known field types and optionality: missing or changed fields are
Invalid, including changes inside either check schema. Mathematical version 1
spellings are equivalent. Nonempty descriptions may vary; prose and annotations
are inert data rather than code. Known optional extension fields can be ignored;
ununderstood mandatory extensions and unknown descriptor types are Unsupported.
Unknown metadata remains in the caller's immutable document.

This catalogue neither validates runtime operation values nor replaces the
family's operation-specific requester/provider rules. Registry selection still
compares the complete installed definition; catalogue recognition does not relax
that existing contract-matching policy. Services still requires
explicit local registration authority, independently installed requester code
and an actual provider before provided work can be selected or advertised.
The tests use a description-only substitute solely to exercise real Registry
admission; it implements no Storage execution or economics.

This completes the independent catalogue portion of plan step 28. Full handler
installation and usable publication remain pending durable store creation and
[CR2SE issue #11](https://github.com/beothorn/CR2SE/issues/11). No default paid
Storage provider is registered. The main module build/run instructions remain in
[README](README.md). From the repository root exercise recognition with
`cargo test --locked modules::storage::standard`.
