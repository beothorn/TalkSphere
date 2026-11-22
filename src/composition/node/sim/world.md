# Simulation world

`run(&scenario)` advances a controlled clock in steps. Each step: clients go online and offline and may lose everything they
store; online clients offer to fulfil the wanted terms and each gets the neediest item it does not hold (the real `Book::take_for`),
bounded by the finite issuance budget; the real maintenance planner decides which copies to expire, renew or check, and the model
carries those out (an offline, silent or unpaid holder is inconclusive, a wiped one fails); verified observations feed the real
trust policy.

Measurements cover the horizon after a one-day warm-up (or a quarter of a shorter horizon): the share of item-time in each CR2SE
state, in which the target was met, at risk, and **retrievable** (a confirmed copy at a connected holder that still has it: the
chance the data could be fetched back right now if the server lost its own copy); the mean time to repair after falling below the
target; placements, checks passed, failed and incomplete, renewals; credits issued for placements and spent on maintenance; the
times maintenance could not be paid because a holder's float ran out; and the mean trust. `Outcome::to_json` is the
machine-readable form.

Models, each testable alone: **churn** (connected or not, exponential stretches), **storage loss** (wiped clients fail checks), **slow peers** (inconclusive checks, never failed), **dishonest peers** (paid, keep nothing, fail checks, never a way to fetch the data back; the credits they receive are reported separately) and, in [Abuse](abuse.md), **flooding** and probing.
