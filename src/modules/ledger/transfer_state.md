# Issuer transfer commitment and replay

TransferResult retains the complete original Order and accepted exact fee;
Received validates fee<gross and returns gross-fee. Commit transfer requires local
Control authority and a fee Accepted capability from the identical trusted local
authority domain/issuer. Under one serialized durable transaction it rechecks
permanent (owner,transferId) replay, spendable issued source and recipient headroom,
debits gross, credits net and retains original order/result before success.

An exact accepted retry returns the original result regardless of later fee policy;
a changed order under the same key conflicts. New admission refuses at replay
capacity instead of deleting lifetime guards. Existing transfer verifies the signed
authenticated owner first and returns the original result before requoting, or
absence; it never submits a new transfer. Unknown commitment requires recovery.
The retained private record is validated on reopen; all amounts remain exact.
