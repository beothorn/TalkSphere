# Peer policy simulations

First results of the [policy simulation](../../src/composition/node/sim/mod.md), recorded on 2026-11-22. Regenerate them with:

```sh
cargo run --locked --release --example policy_sim -- sweep
```

Each figure is the mean of seeds 1 to 5 over seven simulated days of 20 hosted items on a fleet of 40 clients, with the first
day ignored. Runs are reproducible: the same options print the same numbers.

## What these numbers are, and are not

They come from the **real policy, replica book, maintenance planner and redundancy report** running against a **modelled**
network, Storage and prices (the wire protocol is not exercised here; the process tests cover it). They compare parameters
under stated assumptions. They do not predict real phones: every assumption below is a guess to be replaced by measurement.

Model assumptions: a client is connected for exponentially distributed stretches (the default fleet averages 4 hours on and
8 hours off); it loses everything it stores with probability 1 % a day; 5 % of its answers to a check are late. A copy is
**retrievable** while some holder with an unexpired copy that it still has is connected, which is the chance the data could be
fetched back right now if the server lost its own copy. Placements pay 5 credits each; a check costs 3 and a renewal 40 in the
holder's credits (the renewal price is an arbitrary assumption and dominates the cost columns); each holder grants the server a
float of 10000 credits at its first placement. Credit columns are per item per simulated day.

### Replica target on a flaky fleet

| target | retrievable | target met | not redundant | mean repair | issued/item-day | spent/item-day |
|---|---|---|---|---|---|---|
| 1 | 39.0% | 99.9% | 100.0% | 0.1 h | 2.6 | 28.7 |
| 2 | 59.6% | 99.8% | 0.2% | 0.1 h | 5.0 | 57.7 |
| 3 | 75.7% | 99.7% | 0.0% | 0.1 h | 7.3 | 90.4 |
| 5 | 90.9% | 99.5% | 0.0% | 0.1 h | 12.2 | 145.3 |
| 8 | 97.2% | 98.8% | 0.0% | 0.1 h | 19.2 | 225.1 |
| 10 | 98.9% | 98.3% | 0.0% | 0.1 h | 24.8 | 277.4 |
| 15 | 100.0% | 95.7% | 0.0% | 0.2 h | 37.7 | 399.3 |

### Smallest target that keeps data retrievable 99 % of the time

| fleet | target | retrievable | credits/item-day |
|---|---|---|---|
| reliable (12 h on, 2 h off, 0.2 %/day wipes) | 3 | 99.8% | 152.3 |
| flaky (4 h on, 8 h off, 1 %/day wipes) | 15 | 100.0% | 437.0 |
| very flaky (1 h on, 12 h off, 3 %/day wipes) | > 20 (best 20 reached 65.0%) | 65.0% | - |

### Check interval at target 5 with 5 % wipes per day

| check every | retrievable | target met | failed checks | incomplete | spent/item-day |
|---|---|---|---|---|---|
| 1 h | 88.9% | 99.2% | 20 | 59017 | 240.2 |
| 6 h | 90.7% | 99.5% | 27 | 36507 | 145.9 |
| 24 h | 89.6% | 99.4% | 16 | 14688 | 118.1 |
| 72 h | 89.1% | 99.4% | 7 | 9217 | 111.1 |

### Dishonest clients at target 5

| dishonest share | retrievable | target met | failed checks | credits to dishonest |
|---|---|---|---|---|
| 0.0% | 90.9% | 99.5% | 3 | 0 % |
| 10.0% | 90.4% | 99.3% | 57 | 13 % |
| 30.0% | 85.3% | 99.2% | 162 | 36 % |
| 50.0% | 83.3% | 99.1% | 324 | 57 % |

### Request limit per identity per minute (10 abusers at 200/s, honest peers about 12/min in bursts of 3)

| requestsPerMinute | abuser gets through /min | abuser refused | honest refused |
|---|---|---|---|
| none | 12000 | 0.0% | 0.0% |
| 600 | 600 | 95.0% | 0.0% |
| 120 | 120 | 99.0% | 0.0% |
| 60 | 60 | 99.5% | 0.0% |
| 30 | 30 | 99.8% | 0.2% |
| 12 | 12 | 99.9% | 29.4% |

## What the first results say

1. **The replica target is the main lever.** On the default flaky fleet, 2 copies keep data retrievable about 60 % of the time,
   5 copies about 91 %, 10 copies about 99 % and 15 copies essentially always. The user's initial guess of roughly ten phones is
   where this model gets close to 99 %; fifteen copies cost half again as much and lower the share of time the target is met
   (95.7 %), because copies take longer to place. Returns diminish quickly past about eight.
2. **Fleet reliability decides the target.** A reliable fleet needs 3 copies for 99 %; the default flaky fleet needs 10 to 15
   (10 copies reach 98.9 %); a very flaky fleet (connected about 8 % of the time) cannot reach 99 % even with 20 copies. When phones
   are rarely connected, more copies are not the fix: the server would need holders that are connected more of the time, or a
   different mechanism.
3. **Mean repair is short** (about 0.1 hours) once a client offers, because repair is client-initiated: an under-replicated item
   is offered to the next client that fulfils the wanted offering. How fast that happens depends on how often clients offer
   (here every 30 minutes while connected).
4. **Check frequency barely moves availability.** Between 1 hour and 72 hours, retrievability stays within noise (89 % to 91 %)
   while checks cost more than twice as much at 1 hour. Frequent checks detect lost copies sooner, but with copies replaced on
   offer the target is met about 99 % of the time either way. A daily check is a sensible default; it needs a float large enough
   that maintenance is never unpaid.
5. **Credits are paid for work the data owner receives no benefit from.** About 57 % of the credits issued go to dishonest clients
   when half the fleet is dishonest, in proportion to their share: they take the payment and keep nothing. They are caught by checks
   (324 failed checks at 50 %), and the trust policy lowers their trust, **but nothing yet stops the server offering them more
   items**. A follow-up policy should exclude, or deprioritize, clients with failed checks from placement.
6. **Maintenance costs more than placement.** At target 5 the server spends about 145 credits of holders' currency per item-day
   on checks and renewals against 12 issued of its own, driven by the assumed renewal price. Real costs depend on holders'
   prices; the point is that **the server must hold ongoing credit from every holder it asks to keep data**, which is a mutual
   economy and not a one-time purchase. When a holder's float runs out its copies go unchecked (counted as `unpaid`).
7. **A per-identity request limit works as intended.** With honest peers making about 12 requests a minute in bursts of 3,
   limits from 600 down to 60 a minute refuse abusers (95 % to 99.5 %) and no honest request; 30 a minute starts to refuse honest
   bursts (0.2 %) and 12 a minute refuses 29 % of them. The limit must sit comfortably above the honest burst rate.

Noise: with five seeds, differences of a few percent between neighbouring rows (for example the 1 hour and 6 hour check intervals)
are not meaningful, and a result sitting on a threshold (10 copies at 98.9 % against 99 %) may flip between runs of different size.

## Chosen profile

The built-in defaults stay conservative: they reproduce the minimum node, whose target of 2 is CR2SE's minimum for calling data
redundant. For a server that keeps data at phones, the results above support the profile in
[phone-fleet.policy.json](phone-fleet.policy.json):

| Parameter | Value | Why |
|---|---|---|
| `redundancy.target` | 10 | about 99 % retrievable on the default flaky fleet |
| `redundancy.checkSeconds` | 86400 | daily checks; hourly ones cost several times as much for no measurable availability |
| `redundancy.renewBeforeSeconds` | 14400 | renew with four hours of retention left, so a holder that is briefly offline still has time |
| `quota.requestsPerMinute` | 120 | refuses about 99 % of abuser requests, none of the honest ones in the model |
| `trust` steps | small rise per passed check, large drop per failed check | trust builds slowly and is lost quickly |
| `credit.bootstrapCredits` | 0 | no hidden grants; earn through the wanted offering |

Treat the profile as a starting point to be tuned from real measurements: connection durations and offer rates of real phones,
real prices, real wipe rates. Replace the model's assumptions in `Scenario` and rerun the sweeps.

## Limits

- The model has no correlated failures (one carrier outage, one app update, one OS change taking many phones offline together),
  which real fleets have and which make the same target less safe than shown.
- Clients are identical apart from the three kinds modelled; real fleets have a long tail.
- Offers are uniform in time; real phones connect around charging and Wi-Fi.
- Credits flow only as modelled; the full economy (clients spending the credits they earn) is the client journey's concern.
- Five seeds give a rough mean; differences of a few percent are noise.
