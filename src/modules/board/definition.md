# Offering definitions

Definition.parse accepts bounded bytes, one validated offering and Limits.
It requires exact id, service, serviceVersion and description matches and complete
input/output/check schemas. Economic terms and capacity info belong on the Board
and are rejected in definitions. Each mismatch or malformed schema returns Invalid;
unknown nested types produce a discoverable definition with Unknown support.
The immutable Document preserves exact original bytes and unknown metadata.

same_contract compares service, exact version, description and full input/output/
check structures while excluding offering ID. Equality is conservative; matching
names alone never establish compatibility or redefine a standard service version.
