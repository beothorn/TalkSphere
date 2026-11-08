# Local status inspection

Summary requires local Read authority and returns one consistent copied aggregate
of Reserved, Prepared, Committed and Aborted operations plus Pending, Unknown and
Unsettled payments for committed work. It reveals no operation terms, result bytes,
claim secrets or private keys. Unknown local storage gates inspection until recovery.

CLI composition uses the protected store's exclusive writer and requires explicit
local identity and external account selection. Balances remain issuer-separated;
uint64 amounts are printed as exact decimal text. Inspection never initializes,
grants, spends, creates claims, repairs state or opens a peer connection.
