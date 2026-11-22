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
| 1 | 37.0% | 99.9% | 100.0% | 0.1 h | 2.7 | 27.3 |
| 2 | 59.8% | 99.8% | 0.2% | 0.1 h | 5.2 | 58.3 |
| 3 | 75.5% | 99.5% | 0.0% | 0.1 h | 7.5 | 85.5 |
| 5 | 89.5% | 99.1% | 0.0% | 0.1 h | 12.5 | 140.5 |
| 8 | 97.7% | 98.6% | 0.0% | 0.1 h | 19.7 | 225.0 |
| 10 | 99.4% | 98.4% | 0.0% | 0.1 h | 23.6 | 286.5 |
| 15 | 99.9% | 95.7% | 0.0% | 0.2 h | 38.5 | 392.8 |

### Smallest target that keeps data retrievable 99 % of the time

| fleet | target | retrievable | credits/item-day |
|---|---|---|---|
| reliable (12 h on, 2 h off, 0.2 %/day wipes) | 3 | 99.8% | 151.4 |
| flaky (4 h on, 8 h off, 1 %/day wipes) | 10 | 99.4% | 310.1 |
| very flaky (1 h on, 12 h off, 3 %/day wipes) | > 20 (best 20 reached 62.0%) | 62.0% | - |

### Check interval at target 5 with 5 % wipes per day

| check every | retrievable | target met | failed checks | incomplete | spent/item-day |
|---|---|---|---|---|---|
| 1 h | 90.8% | 99.0% | 35 | 57615 | 245.5 |
| 6 h | 88.2% | 99.1% | 25 | 38059 | 135.4 |
| 24 h | 90.1% | 99.1% | 19 | 15714 | 111.6 |
| 72 h | 87.9% | 99.3% | 8 | 9621 | 109.2 |

### Dishonest clients at target 5

| dishonest share | retrievable | target met | failed checks | credits to dishonest |
|---|---|---|---|---|
| 0.0% | 89.5% | 99.1% | 5 | 0 % |
| 10.0% | 91.1% | 99.3% | 39 | 11 % |
| 30.0% | 88.5% | 99.1% | 149 | 32 % |
| 50.0% | 83.4% | 98.8% | 330 | 56 % |

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
   5 copies about 90 % and 10 copies about 99 %. The user's initial guess of roughly ten phones is where this model reaches 99 %;
   going to 15 adds little availability (99.9 %) but lowers the share of time the target is met (95.7 %), because copies take longer
   to place. Returns diminish quickly past about eight.
2. **Fleet reliability decides the target.** A reliable fleet needs 3 copies for 99 %; the default flaky fleet needs 10; a very flaky
   fleet (connected about 8 % of the time) cannot reach 99 % even with 20 copies. When phones are rarely connected, more copies are
   not the fix: the server would need holders that are connected more of the time, or a different mechanism.
3. **Mean repair is short** (about 0.1 hours) once a client offers, because repair is client-initiated: an under-replicated item
   is offered to the next client that fulfils the wanted offering. How fast that happens depends on how often clients offer
   (here every 30 minutes while connected).
4. **Check frequency barely moves availability.** Between 1 hour and 72 hours, retrievability stays within noise (88 % to 91 %)
   while checks cost far more at 1 hour. Frequent checks detect lost copies sooner (35 failed checks at 1 hour against 8 at 72), but
   with copies replaced on offer the target is met about 99 % either way. A daily check is a sensible default; it needs a
   float large enough that maintenance is never unpaid.
5. **Credits are paid for work the data owner receives no benefit from.** About 56 % of the credits issued go to dishonest clients
   when half the fleet is dishonest, in proportion to their share: they take the payment and keep nothing. They are caught by checks
   (330 failed checks at 50 %), and the trust policy lowers their trust, **but nothing yet stops the server offering them more
   items**. A follow-up policy should exclude, or deprioritize, clients with failed checks from placement.
6. **Maintenance costs more than placement.** At target 5 the server spends about 140 credits of holders' currency per item-day
   on checks and renewals against 12 issued of its own, driven by the assumed renewal price. Real costs depend on holders'
   prices; the point is that **the server must hold ongoing credit from every holder it asks to keep data**, which is a mutual
   economy and not a one-time purchase. When a holder's float runs out its copies go unchecked (counted as `unpaid`).
7. **A per-identity request limit works as intended.** With honest peers making about 12 requests a minute in bursts of 3,
   limits from 600 down to 60 a minute refuse abusers (95 % to 99.5 %) and no honest request; 30 a minute starts to refuse honest
   bursts (0.2 %) and 12 a minute refuses 29 % of them. The limit must sit comfortably above the honest burst rate.

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
