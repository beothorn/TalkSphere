# Explicit selection

Request supplies exact offering ID, service/version/description, the identity
opposite the publisher and trusted authentication observation. Provider.select
requires Select authority and an owner-issued pin; Cache.select also requires
a current/fresh directly authenticated receipt with a matching definition.
Forwarded documents cannot authenticate a publisher or authorize selection.
Missing data, stale identity fields and unsupported semantics fail explicitly.

Selection owns immutable offering and definition copies and the bound publisher.
roles returns requester/provider/issuer. For provided work the publisher is
provider; for wanted work the publisher is requester. The issuer always remains
the offering's exact explicit ID, independent of TCP or stream opener.
contract_matches conservatively compares complete logical definitions; economic
compatibility, trust, price and acceptance still require explicit caller policy.
No operation reserves capacity, quotes, invokes, installs software or spends.

Provider.current_selection compares the complete current offering and definition
against retained terms under Select authority. Removal, changed price/issuer,
metadata or contract returns Stale; unresolved durability returns Unknown. Local
select also performs this check before returning new terms. Reordering the Board
arrays alone adds no priority and does not change individual offering terms.
Previously copied selections remain immutable and do not cancel existing work;
actual quote acceptance and durable accepted-invocation ownership belong to later
Services/Peer Protocol integration. Board never accepts a changed price for callers.
