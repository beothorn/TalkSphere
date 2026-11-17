# Storage module boundary

Services supplies authenticated accepted invocation context and owns coordination;
Storage owns immutable byte and lease effects. Exporting these primitives does
not register a runnable paid provider. See [current scope](README.md) and the
[requirements audit](REQUIREMENTS.md).

[Contracts](contract.md) define exact lease descriptions and injected ports.
[Operation validation](operation.md), [lease authorization](lease.md),
[terms](terms.md), [pricing](pricing.md) and [store assessment](assessment.md)
check values before execution. [Clock policy](clock_policy.md) and
[retention](retention.md) separate requester tolerance from provider elapsed time.

[Images](image.md), [effects](effect.md) and the [participant journal](journal.md)
retain exact owner state and local decisions. [Quota](quota.md) bounds original
resource holds. Linux [byte persistence](bytes/mod.md) and the
[recovery gate](recovery/mod.md) protect durable resources before admission.

Actual paid lease creation remains pending upstream acceptance-time clarification
[#11](https://github.com/beothorn/CR2SE/issues/11); no precommit timing rule is
silently advertised as a complete Storage implementation.

[Requester contracts](requester/mod.md) independently validate original store
expectations and retain only acknowledged results in application-owned catalogs.

Linux [retrieval acceptance](retrieve.md) retains an authorized bounded subrange
under immutable accepted prices, independently of later lease/Board state changes.
