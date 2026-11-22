//! A simulated population and the policy under test.
//!
//! Callers: [`super::world::run`] and the sweeps. Every number here is a model assumption, not a
//! measurement: the whole point of the harness is to compare policies under assumptions that real
//! usage will later replace. Defaults describe a deliberately unreliable phone fleet.
use crate::composition::policy::Params;

/// How the honest clients behave.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Honest {
    /// Number of clients.
    pub count: usize,
    /// Mean seconds a client stays connected.
    pub mean_online_seconds: f64,
    /// Mean seconds a client stays disconnected.
    pub mean_offline_seconds: f64,
    /// Mean seconds between a connected client's offers to fulfil the wanted terms.
    pub offer_every_seconds: f64,
    /// Probability per day that a client loses everything it stores (a wiped or replaced phone).
    pub wipe_per_day: f64,
    /// Probability that a connected client fails to answer a check in time.
    pub unresponsive: f64,
    /// Share of the clients that are slow: they miss checks with `slow_unresponsive` instead.
    pub slow_fraction: f64,
    /// Probability that a slow client fails to answer a check in time.
    pub slow_unresponsive: f64,
    /// Share of the clients that accept placements and are paid for them but keep nothing, so every check
    /// of their copies fails.
    pub dishonest_fraction: f64,
}

/// The whole scenario.
#[derive(Debug, Clone, PartialEq)]
pub struct Scenario {
    /// Seed that determines the run.
    pub seed: u64,
    /// The policy parameters under test.
    pub params: Params,
    /// Hosted items to protect.
    pub items: usize,
    /// Sealed size of each item in bytes.
    pub item_bytes: u64,
    /// Simulated seconds.
    pub horizon_seconds: u64,
    /// Seconds per simulation step.
    pub step_seconds: u64,
    /// The honest clients.
    pub honest: Honest,
    /// Credits the server may issue for placements over the whole run.
    pub issuance_budget: u64,
    /// Credits each holder issued to the server up front to pay for its maintenance.
    pub float_per_holder: u64,
    /// Price of one check at a holder.
    pub check_price: u64,
    /// Price of one renewal at a holder.
    pub renew_price: u64,
}

impl Default for Scenario {
    // A small fleet of flaky phones over one simulated week.
    fn default() -> Self {
        let mut params = Params::default();
        params.credit.wanted = Some(crate::composition::policy::Wanted {
            credits_per_object: 5,
            maximum_object_bytes: 16384,
            retention_seconds: 86_400,
        });
        params.redundancy.target = 3;
        params.redundancy.check_seconds = 6 * 3600;
        params.redundancy.renew_before_seconds = 4 * 3600;
        params.redundancy.max_spend = 1000;
        params.redundancy.retry_seconds = 600;
        Self {
            seed: 1,
            params,
            items: 20,
            item_bytes: 4096,
            horizon_seconds: 7 * 86_400,
            step_seconds: 60,
            honest: Honest {
                count: 40,
                mean_online_seconds: 4.0 * 3600.0,
                mean_offline_seconds: 8.0 * 3600.0,
                offer_every_seconds: 1800.0,
                wipe_per_day: 0.01,
                unresponsive: 0.05,
                slow_fraction: 0.0,
                slow_unresponsive: 0.8,
                dishonest_fraction: 0.0,
            },
            issuance_budget: 1_000_000,
            float_per_holder: 10_000,
            check_price: 3,
            renew_price: 40,
        }
    }
}

impl Scenario {
    /// Check the scenario is runnable.
    /// # Errors
    /// Zero sizes, a step longer than the horizon, probabilities outside `[0, 1]` or invalid policy.
    pub fn validate(&self) -> Result<(), String> {
        tracing::trace!("Validating a simulation scenario");
        let h = &self.honest;
        let probability = |p: f64| (0.0..=1.0).contains(&p);
        if self.items == 0 || self.item_bytes == 0 || h.count == 0 {
            return Err("items, item size and clients must be positive".into());
        }
        if self.step_seconds == 0 || self.step_seconds > self.horizon_seconds {
            return Err("the step must be positive and within the horizon".into());
        }
        let all = [
            h.wipe_per_day,
            h.unresponsive,
            h.slow_fraction,
            h.slow_unresponsive,
            h.dishonest_fraction,
        ];
        if !all.into_iter().all(probability) || h.slow_fraction + h.dishonest_fraction > 1.0 {
            return Err(
                "probabilities must be within 0 and 1 and the client kinds must fit".into(),
            );
        }
        let positive = |x: f64| x.is_finite() && x > 0.0;
        if !(positive(h.mean_online_seconds)
            && positive(h.mean_offline_seconds)
            && positive(h.offer_every_seconds))
        {
            return Err("durations must be positive".into());
        }
        if self.params.credit.wanted.is_none() {
            return Err("the policy must publish wanted terms to place replicas".into());
        }
        self.params.validate().map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The default scenario is valid, and each kind of bad value is refused.
    #[test]
    fn validation_refuses_unrunnable_scenarios() {
        assert!(Scenario::default().validate().is_ok());
        let bad = |edit: fn(&mut Scenario)| {
            let mut s = Scenario::default();
            edit(&mut s);
            s.validate().is_err()
        };
        assert!(bad(|s| s.items = 0));
        assert!(bad(|s| s.honest.count = 0));
        assert!(bad(|s| s.step_seconds = 0));
        assert!(bad(|s| s.step_seconds = s.horizon_seconds + 1));
        assert!(bad(|s| s.honest.wipe_per_day = 1.5));
        assert!(bad(|s| s.honest.unresponsive = -0.1));
        assert!(bad(|s| {
            s.honest.slow_fraction = 0.7;
            s.honest.dishonest_fraction = 0.6;
        }));
        assert!(bad(|s| s.honest.dishonest_fraction = 1.2));
        assert!(bad(|s| s.honest.mean_online_seconds = 0.0));
        assert!(bad(|s| s.honest.offer_every_seconds = f64::NAN));
        assert!(bad(|s| s.params.credit.wanted = None));
        assert!(bad(
            |s| s.params.admission.per_peer_connections = s.params.admission.connections + 1
        ));
    }
}
