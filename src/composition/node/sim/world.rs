//! The simulation loop: a server hosting items, a fleet of unreliable clients and the real policy.
//!
//! What is real: the policy contracts (`Configured`), the observation table, the replica book
//! (volatile), the maintenance planner and the redundancy report. What is modelled: time (a controlled
//! clock), the network (a client is online or not), Storage (a copy exists while the holder has not been
//! wiped) and prices. The wire protocol is not exercised here; the process tests cover it. Everything is
//! a function of the scenario's seed, so a run is exactly reproducible.
use super::{rng::Rng, scenario::Scenario};
use crate::{
    composition::{
        node::replicas::{
            book::{Book, Item, Replica},
            maintain::{Action, MAXIMUM_PAID_ACTIONS, plan},
            report::{State, report},
        },
        policy::{Configured, Event, Kind, Observations, PeerView, Trust as _},
    },
    modules::identity::api::IdentityId,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// Simulated Unix time at which a run starts.
const START: u64 = 1_000_000_000;
/// Seconds of a run ignored before measuring, while the first copies are placed.
pub const WARMUP_SECONDS: u64 = 86_400;

/// Everything measured in one run, over the measured part of the horizon.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    /// Seed of the run.
    pub seed: u64,
    /// Fraction of item-time in each state.
    pub not_redundant: f64,
    /// Fraction of item-time under-replicated.
    pub under_replicated: f64,
    /// Fraction of item-time with the target satisfied.
    pub target_satisfied: f64,
    /// Fraction of item-time with the target met by the confirmed count.
    pub target_met: f64,
    /// Fraction of item-time with at least one confirmed copy at a connected holder that still has it:
    /// the chance the data could be fetched back right now if the server lost its own copy.
    pub retrievable: f64,
    /// Fraction of item-time the report called at risk.
    pub at_risk: f64,
    /// Mean seconds from falling below the target to meeting it again, and how many such repairs.
    pub mean_repair_seconds: f64,
    /// Number of completed repairs.
    pub repairs: u64,
    /// Copies placed.
    pub placements: u64,
    /// Checks that passed, failed and were inconclusive (unreachable, silent or unpaid).
    pub checks_passed: u64,
    /// Checks that returned conclusive negative evidence.
    pub checks_failed: u64,
    /// Checks and renewals that could not be completed.
    pub incomplete: u64,
    /// Renewals completed.
    pub renewals: u64,
    /// Credits the server issued for placements.
    pub credits_issued: u64,
    /// Credits the server spent, in holders' credits, on checks and renewals.
    pub credits_spent: u64,
    /// Credits the server issued to clients that kept nothing.
    pub credits_to_dishonest: u64,
    /// Times a maintenance operation could not be paid because the holder's float ran out.
    pub unpaid: u64,
    /// Mean trust the server ended with across clients that contributed, in millionths.
    pub mean_trust: f64,
}

impl Outcome {
    /// The machine-readable form of the run.
    pub fn to_json(&self) -> Value {
        json!({
            "seed": self.seed,
            "notRedundant": self.not_redundant, "underReplicated": self.under_replicated,
            "targetSatisfied": self.target_satisfied, "targetMet": self.target_met,
            "retrievable": self.retrievable, "atRisk": self.at_risk,
            "meanRepairSeconds": self.mean_repair_seconds, "repairs": self.repairs,
            "placements": self.placements, "checksPassed": self.checks_passed,
            "checksFailed": self.checks_failed, "incomplete": self.incomplete,
            "renewals": self.renewals, "creditsIssued": self.credits_issued,
            "creditsSpent": self.credits_spent, "creditsToDishonest": self.credits_to_dishonest,
            "unpaid": self.unpaid, "meanTrust": self.mean_trust,
        })
    }
}

// How a client behaves beyond going offline.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Behavior {
    Normal,
    /// Misses most checks.
    Slow,
    /// Takes the payment and keeps nothing.
    Dishonest,
}

struct Client {
    behavior: Behavior,
    id: IdentityId,
    online: bool,
    next_toggle: u64,
    /// Incremented when the client loses everything it stores.
    generation: u64,
    /// Credits of this holder the server owns, to pay for maintenance; granted at its first placement.
    float: Option<u64>,
}

/// A client identity: the index in the first bytes.
fn identity(index: usize) -> IdentityId {
    let mut bytes = [0u8; 32];
    bytes[..8].copy_from_slice(&(index as u64 + 1).to_be_bytes());
    IdentityId(bytes)
}

/// Run one scenario to its horizon.
/// # Errors
/// An invalid scenario.
pub fn run(scenario: &Scenario) -> Result<Outcome, String> {
    tracing::trace!("Running one policy simulation");
    scenario.validate()?;
    let mut world = World::new(scenario)?;
    let mut time = 0;
    while time < scenario.horizon_seconds {
        world.step(time);
        time += scenario.step_seconds;
    }
    Ok(world.outcome())
}

struct World<'a> {
    s: &'a Scenario,
    rng: Rng,
    policy: Configured,
    book: Book,
    observations: Observations,
    /// Simulated second of the last incomplete attempt on each copy.
    attempted: BTreeMap<([u8; 16], IdentityId), u64>,
    clients: Vec<Client>,
    trust: BTreeMap<IdentityId, u32>,
    /// The generation of the holder when it confirmed each copy.
    held: BTreeMap<([u8; 16], IdentityId), u64>,
    below_since: BTreeMap<[u8; 16], u64>,
    // Counters.
    measured_item_seconds: f64,
    states: [f64; 3],
    target_met: f64,
    retrievable: f64,
    at_risk: f64,
    repair_total: u64,
    repairs: u64,
    placements: u64,
    passed: u64,
    failed: u64,
    incomplete: u64,
    renewals: u64,
    issued: u64,
    to_dishonest: u64,
    spent: u64,
    unpaid: u64,
}

impl<'a> World<'a> {
    // Build the world: items in the book, clients with their first online or offline stretch.
    fn new(s: &'a Scenario) -> Result<Self, String> {
        let mut rng = Rng::new(s.seed);
        let book = Book::volatile();
        for n in 0..s.items {
            let mut id = [0u8; 16];
            id[..8].copy_from_slice(&(n as u64 + 1).to_be_bytes());
            book.add(
                Item {
                    id,
                    size: s.item_bytes,
                    sealed_size: s.item_bytes,
                    plaintext_hash: [0; 32],
                    created_at: START,
                    replicas: Vec::new(),
                },
                &vec![0u8; s.item_bytes as usize],
            )
            .map_err(|e| format!("adding a simulated item failed: {e:?}"))?;
        }
        let h = &s.honest;
        let clients = (0..h.count)
            .map(|n| {
                let online = rng.chance(
                    h.mean_online_seconds / (h.mean_online_seconds + h.mean_offline_seconds),
                );
                let mean = if online {
                    h.mean_online_seconds
                } else {
                    h.mean_offline_seconds
                };
                // The first clients are dishonest, the next slow, the rest normal: deterministic and nested.
                let dishonest = (h.dishonest_fraction * h.count as f64).round() as usize;
                let slow = (h.slow_fraction * h.count as f64).round() as usize;
                let behavior = if n < dishonest {
                    Behavior::Dishonest
                } else if n < dishonest + slow {
                    Behavior::Slow
                } else {
                    Behavior::Normal
                };
                Client {
                    behavior,
                    id: identity(n),
                    online,
                    next_toggle: START + rng.exponential(mean),
                    generation: 0,
                    float: None,
                }
            })
            .collect();
        Ok(Self {
            s,
            rng,
            policy: Configured::new(s.params),
            book,
            observations: Observations::new(),
            attempted: BTreeMap::new(),
            clients,
            trust: BTreeMap::new(),
            held: BTreeMap::new(),
            below_since: BTreeMap::new(),
            measured_item_seconds: 0.0,
            states: [0.0; 3],
            target_met: 0.0,
            retrievable: 0.0,
            at_risk: 0.0,
            repair_total: 0,
            repairs: 0,
            placements: 0,
            passed: 0,
            failed: 0,
            incomplete: 0,
            renewals: 0,
            issued: 0,
            to_dishonest: 0,
            spent: 0,
            unpaid: 0,
        })
    }

    // Advance the world by one step starting `elapsed` seconds into the run.
    fn step(&mut self, elapsed: u64) {
        let now = START + elapsed;
        self.churn(now);
        self.offers(now);
        self.maintain(now);
        if elapsed >= WARMUP_SECONDS.min(self.s.horizon_seconds / 4) {
            self.measure(now);
        }
    }

    // Clients go online and offline and occasionally lose everything they store.
    fn churn(&mut self, now: u64) {
        let h = self.s.honest;
        let wipe = h.wipe_per_day * self.s.step_seconds as f64 / 86_400.0;
        for client in &mut self.clients {
            while now >= client.next_toggle {
                client.online = !client.online;
                let mean = if client.online {
                    h.mean_online_seconds
                } else {
                    h.mean_offline_seconds
                };
                client.next_toggle += self.rng.exponential(mean);
            }
            if self.rng.chance(wipe) {
                client.generation += 1;
            }
        }
    }

    // Connected clients offer to fulfil the wanted terms; each receives the neediest item it does not hold.
    fn offers(&mut self, now: u64) {
        let Some(wanted) = self.s.params.credit.wanted else {
            return;
        };
        let chance = (self.s.step_seconds as f64 / self.s.honest.offer_every_seconds).min(1.0);
        let target = self.policy_target();
        for index in 0..self.clients.len() {
            if !self.clients[index].online || !self.rng.chance(chance) {
                continue;
            }
            let id = self.clients[index].id;
            // The finite issuance budget bounds everything the server promises.
            if self.issued + wanted.credits_per_object > self.s.issuance_budget {
                continue;
            }
            let Some((item, _)) = self
                .book
                .take_for(id, target, wanted.maximum_object_bytes, now)
            else {
                continue;
            };
            let replica = Replica {
                provider: id,
                lease: [0; 32],
                expires_at: now + wanted.retention_seconds,
                confirmed_at: now,
                last_checked: None,
            };
            if self.book.confirm(item, replica).is_err() {
                self.book.release(item, id);
                continue;
            }
            // A dishonest client's generation never matches again: it keeps nothing.
            let kept = match self.clients[index].behavior {
                Behavior::Dishonest => u64::MAX,
                _ => self.clients[index].generation,
            };
            self.held.insert((item, id), kept);
            self.placements += 1;
            self.issued += wanted.credits_per_object;
            if self.clients[index].behavior == Behavior::Dishonest {
                self.to_dishonest += wanted.credits_per_object;
            }
            self.clients[index]
                .float
                .get_or_insert(self.s.float_per_holder);
            self.observe(id, Event::ContributionVerified, now);
        }
    }

    // Run the real planner and carry its actions out against the model.
    fn maintain(&mut self, now: u64) {
        let items = self.book.list();
        let retry = self.s.params.redundancy.retry_seconds;
        let limit = MAXIMUM_PAID_ACTIONS * self.s.step_seconds as usize;
        let attempted = &self.attempted;
        let actions = plan(
            &items,
            now,
            &self.policy,
            &|item, provider| {
                attempted
                    .get(&(item, provider))
                    .is_some_and(|at| now.saturating_sub(*at) < retry)
            },
            limit,
        );
        for action in actions {
            match action {
                Action::Expire { item, provider } => {
                    let _ = self.book.drop_replica(item, provider);
                    self.held.remove(&(item, provider));
                    self.attempted.remove(&(item, provider));
                }
                Action::Renew { item, provider, .. } => self.renew(item, provider, now),
                Action::Check { item, provider, .. } => self.check(item, provider, now),
            }
        }
    }

    // Renew one copy if its holder is connected and can be paid.
    fn renew(&mut self, item: [u8; 16], provider: IdentityId, now: u64) {
        let price = self.s.renew_price;
        if !self.pay(provider, price, now, item) {
            return;
        }
        let retention = self
            .s
            .params
            .credit
            .wanted
            .map_or(0, |w| w.retention_seconds);
        let _ = self
            .book
            .update_replica(item, provider, |r| r.expires_at += retention);
        self.renewals += 1;
        self.attempted.remove(&(item, provider));
    }

    // Check one copy: unreachable, silent and unpaid holders are inconclusive, a wiped one fails.
    fn check(&mut self, item: [u8; 16], provider: IdentityId, now: u64) {
        let price = self.s.check_price;
        if !self.pay(provider, price, now, item) {
            return;
        }
        let silence = match self
            .clients
            .iter()
            .find(|c| c.id == provider)
            .map(|c| c.behavior)
        {
            Some(Behavior::Slow) => self.s.honest.slow_unresponsive,
            _ => self.s.honest.unresponsive,
        };
        if self.rng.chance(silence) {
            self.incomplete += 1;
            self.attempted.insert((item, provider), now);
            return;
        }
        let intact = self.holder_has(item, provider);
        if intact {
            let _ = self
                .book
                .update_replica(item, provider, |r| r.last_checked = Some(now));
            self.passed += 1;
            self.observe(provider, Event::CheckPassed, now);
        } else {
            let _ = self.book.drop_replica(item, provider);
            self.held.remove(&(item, provider));
            self.failed += 1;
            self.observe(provider, Event::CheckFailed, now);
        }
        self.attempted.remove(&(item, provider));
    }

    // Spend the holder's credits for one operation; false (and a remembered attempt) when the holder is
    // offline or its float cannot cover the price or the policy's spending bound.
    fn pay(&mut self, provider: IdentityId, price: u64, now: u64, item: [u8; 16]) -> bool {
        let max = self.s.params.redundancy.max_spend;
        let Some(client) = self.clients.iter_mut().find(|c| c.id == provider) else {
            return false;
        };
        if !client.online || price > max {
            self.incomplete += 1;
            self.attempted.insert((item, provider), now);
            return false;
        }
        match client.float {
            Some(float) if float >= price => {
                client.float = Some(float - price);
                self.spent += price;
                true
            }
            _ => {
                self.unpaid += 1;
                self.incomplete += 1;
                self.attempted.insert((item, provider), now);
                false
            }
        }
    }

    // Whether the holder still has what it confirmed (it has not been wiped since).
    fn holder_has(&self, item: [u8; 16], provider: IdentityId) -> bool {
        let generation = self
            .clients
            .iter()
            .find(|c| c.id == provider)
            .map(|c| c.generation);
        self.held.get(&(item, provider)).copied() == generation
    }

    // Record a verified observation and apply the trust policy to the server's trust in the client.
    fn observe(&mut self, peer: IdentityId, event: Event, now: u64) {
        let kind = match event {
            Event::CheckPassed => Kind::CheckPassed,
            Event::CheckFailed => Kind::CheckFailed,
            Event::ContributionVerified => Kind::ContributionVerified,
            Event::Failed => Kind::Failed,
        };
        self.observations.record(peer, kind, now);
        let current = self.trust.get(&peer).copied().unwrap_or(0);
        let view = PeerView {
            peer,
            trust_millionths: current,
            summary: self.observations.summary(peer),
            now_seconds: now,
        };
        if let Some(new) = self.policy.adjust(&view, event) {
            self.trust.insert(peer, new);
        }
    }

    // The policy's replica target.
    fn policy_target(&self) -> usize {
        use crate::composition::policy::Redundancy as _;
        self.policy.target()
    }

    // Accumulate the time-weighted state of every item for this step.
    fn measure(&mut self, now: u64) {
        let dt = self.s.step_seconds as f64;
        let check_seconds = self.s.params.redundancy.check_seconds;
        for item in self.book.list() {
            let r = report(&item, now, &self.policy, check_seconds);
            self.measured_item_seconds += dt;
            let slot = match r.state {
                State::NotRedundant => 0,
                State::UnderReplicated => 1,
                State::TargetSatisfied => 2,
            };
            self.states[slot] += dt;
            if r.target_met {
                self.target_met += dt;
            }
            if r.at_risk {
                self.at_risk += dt;
            }
            let reachable = item.replicas.iter().any(|rep| {
                rep.expires_at > now
                    && self
                        .clients
                        .iter()
                        .any(|c| c.id == rep.provider && c.online)
                    && self.holder_has(item.id, rep.provider)
            });
            if reachable {
                self.retrievable += dt;
            }
            match (r.target_met, self.below_since.get(&item.id).copied()) {
                (false, None) => {
                    self.below_since.insert(item.id, now);
                }
                (true, Some(since)) => {
                    self.below_since.remove(&item.id);
                    self.repair_total += now - since;
                    self.repairs += 1;
                }
                _ => {}
            }
        }
    }

    // The measured result.
    fn outcome(&self) -> Outcome {
        let total = self.measured_item_seconds.max(1.0);
        let trusts: Vec<f64> = self.trust.values().map(|t| f64::from(*t)).collect();
        Outcome {
            seed: self.s.seed,
            not_redundant: self.states[0] / total,
            under_replicated: self.states[1] / total,
            target_satisfied: self.states[2] / total,
            target_met: self.target_met / total,
            retrievable: self.retrievable / total,
            at_risk: self.at_risk / total,
            mean_repair_seconds: if self.repairs == 0 {
                0.0
            } else {
                self.repair_total as f64 / self.repairs as f64
            },
            repairs: self.repairs,
            placements: self.placements,
            checks_passed: self.passed,
            checks_failed: self.failed,
            incomplete: self.incomplete,
            renewals: self.renewals,
            credits_issued: self.issued,
            credits_spent: self.spent,
            credits_to_dishonest: self.to_dishonest,
            unpaid: self.unpaid,
            mean_trust: if trusts.is_empty() {
                0.0
            } else {
                trusts.iter().sum::<f64>() / trusts.len() as f64
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A fast scenario: three days of a small fleet.
    fn small() -> Scenario {
        Scenario {
            horizon_seconds: 3 * 86_400,
            items: 8,
            honest: super::super::scenario::Honest {
                count: 20,
                ..Scenario::default().honest
            },
            ..Scenario::default()
        }
    }

    // A fleet that never goes offline, never loses data and always answers.
    fn perfect() -> Scenario {
        let mut s = small();
        s.honest.mean_online_seconds = 1e9;
        s.honest.mean_offline_seconds = 1.0;
        s.honest.wipe_per_day = 0.0;
        s.honest.unresponsive = 0.0;
        s
    }

    // The same seed gives exactly the same measurements, a different seed does not.
    #[test]
    fn a_run_is_reproducible_from_its_seed() {
        let s = small();
        assert_eq!(run(&s).unwrap(), run(&s).unwrap());
        let other = Scenario { seed: 2, ..small() };
        assert_ne!(run(&s).unwrap(), run(&other).unwrap());
    }

    // With a perfect fleet every item reaches and keeps its target and is always retrievable.
    #[test]
    fn a_perfect_fleet_keeps_every_item_at_target() {
        let outcome = run(&perfect()).unwrap();
        assert!(outcome.target_met > 0.99, "{outcome:?}");
        assert!(outcome.retrievable > 0.99, "{outcome:?}");
        assert_eq!(outcome.checks_failed, 0);
        assert!(outcome.checks_passed > 0 && outcome.renewals > 0);
        assert!(
            (outcome.not_redundant + outcome.under_replicated + outcome.target_satisfied - 1.0)
                .abs()
                < 1e-9
        );
    }

    // More copies make data more likely to be retrievable from a flaky fleet, on average over seeds.
    #[test]
    fn more_copies_improve_retrievability() {
        let mean = |target: usize| {
            (1..=3)
                .map(|seed| {
                    let mut s = Scenario { seed, ..small() };
                    s.params.redundancy.target = target;
                    run(&s).unwrap().retrievable
                })
                .sum::<f64>()
                / 3.0
        };
        let (one, five) = (mean(1), mean(5));
        assert!(five > one + 0.05, "one copy {one}, five copies {five}");
    }

    // Wiped holders fail checks and lose their copies; an exhausted float leaves copies unchecked and unpaid.
    #[test]
    fn wipes_fail_checks_and_an_empty_float_leaves_copies_unchecked() {
        let mut wiped = perfect();
        wiped.honest.wipe_per_day = 0.6;
        assert!(run(&wiped).unwrap().checks_failed > 0);
        let mut broke = perfect();
        broke.float_per_holder = 0;
        let outcome = run(&broke).unwrap();
        assert_eq!((outcome.checks_passed, outcome.renewals), (0, 0));
        assert!(outcome.unpaid > 0);
    }

    // The finite issuance budget bounds what the server promises, and a bad scenario is refused.
    #[test]
    fn the_issuance_budget_bounds_placements_and_bad_scenarios_fail() {
        let mut s = perfect();
        s.issuance_budget = 10;
        let outcome = run(&s).unwrap();
        assert!(
            outcome.placements <= 2 && outcome.credits_issued <= 10,
            "{outcome:?}"
        );
        assert!(
            run(&Scenario {
                items: 0,
                ..small()
            })
            .is_err()
        );
    }

    // Churn alone makes data unavailable at times but never fails a check or loses a copy.
    #[test]
    fn churn_alone_reduces_availability_only() {
        let mut s = perfect();
        s.honest.mean_online_seconds = 3600.0;
        s.honest.mean_offline_seconds = 3.0 * 3600.0;
        let outcome = run(&s).unwrap();
        assert_eq!(outcome.checks_failed, 0);
        assert!(outcome.retrievable < 0.999, "{outcome:?}");
        assert!(
            outcome.target_met > 0.5,
            "copies persist through churn: {outcome:?}"
        );
    }

    // Slow peers miss checks (inconclusive) but never fail them and keep their copies.
    #[test]
    fn slow_peers_are_inconclusive_not_failed() {
        let mut s = perfect();
        s.honest.slow_fraction = 1.0;
        s.honest.slow_unresponsive = 0.9;
        let outcome = run(&s).unwrap();
        assert_eq!(outcome.checks_failed, 0);
        assert!(outcome.incomplete > outcome.checks_passed, "{outcome:?}");
        assert!(outcome.target_met > 0.9, "{outcome:?}");
    }

    // Dishonest peers are paid for placements, fail their checks and are never a way to get the data back;
    // honest peers lose nothing to them.
    #[test]
    fn dishonest_peers_fail_checks_and_hold_nothing() {
        let mut s = perfect();
        s.honest.dishonest_fraction = 1.0;
        let all = run(&s).unwrap();
        assert!(
            all.checks_failed > 0 && all.credits_to_dishonest > 0,
            "{all:?}"
        );
        assert_eq!(all.retrievable, 0.0);
        let mut some = perfect();
        some.honest.dishonest_fraction = 0.5;
        let half = run(&some).unwrap();
        assert!(
            half.checks_failed > 0 && half.credits_to_dishonest < half.credits_issued,
            "{half:?}"
        );
        assert!(half.retrievable > 0.5, "{half:?}");
    }

    // The machine-readable form carries every measurement under stable names.
    #[test]
    fn the_report_is_machine_readable() {
        let json = run(&perfect()).unwrap().to_json();
        for key in [
            "seed",
            "retrievable",
            "targetMet",
            "meanRepairSeconds",
            "creditsSpent",
            "unpaid",
        ] {
            assert!(json.get(key).is_some(), "{key}");
        }
    }
}
