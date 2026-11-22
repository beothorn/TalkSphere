//! Request-rate abuse against the quota policy.
//!
//! Callers: sweeps and tests. Abusive peers send requests as fast as they can and honest peers send
//! bursts at a modest rate; both are counted by the real rolling-minute limiter with the limit the real
//! quota policy returns, one second at a time. The result shows what a limit costs honest peers and
//! how much it lets abusers through, which is the trade-off the rate parameters tune.
use super::rng::Rng;
use crate::{
    composition::policy::{Configured, Params, PeerView, Quota as _, Rates, Summary},
    modules::identity::api::IdentityId,
};
use serde_json::{Value, json};

/// An abuse experiment.
#[derive(Debug, Clone, PartialEq)]
pub struct Abuse {
    /// Seed of the run.
    pub seed: u64,
    /// The policy under test.
    pub params: Params,
    /// Abusive peers.
    pub abusers: usize,
    /// Requests each abuser attempts every second.
    pub abuser_requests_per_second: u32,
    /// Honest peers.
    pub honest: usize,
    /// Requests an honest peer makes per minute on average.
    pub honest_requests_per_minute: f64,
    /// Requests in one honest burst (one fetch opens several streams).
    pub honest_burst: u32,
    /// Simulated seconds.
    pub seconds: u64,
}

impl Default for Abuse {
    // Ten abusers hammering at 200 requests a second against fifty peers making about 12 requests a minute.
    fn default() -> Self {
        Self {
            seed: 1,
            params: Params::default(),
            abusers: 10,
            abuser_requests_per_second: 200,
            honest: 50,
            honest_requests_per_minute: 12.0,
            honest_burst: 3,
            seconds: 600,
        }
    }
}

/// What the limiter did.
#[derive(Debug, Clone, PartialEq)]
pub struct AbuseOutcome {
    /// Requests per minute each abuser got through, on average.
    pub abuser_allowed_per_minute: f64,
    /// Share of abuser requests refused.
    pub abuser_refused: f64,
    /// Share of honest requests refused.
    pub honest_refused: f64,
}

impl AbuseOutcome {
    /// The machine-readable form.
    pub fn to_json(&self) -> Value {
        json!({
            "abuserAllowedPerMinute": self.abuser_allowed_per_minute,
            "abuserRefused": self.abuser_refused, "honestRefused": self.honest_refused,
        })
    }
}

/// Run the experiment.
/// # Errors
/// Zero peers or seconds, or an invalid burst.
pub fn run(abuse: &Abuse) -> Result<AbuseOutcome, String> {
    tracing::trace!("Running a request-rate abuse experiment");
    if abuse.seconds == 0 || abuse.honest == 0 || abuse.honest_burst == 0 {
        return Err("seconds, honest peers and the burst size must be positive".into());
    }
    abuse.params.validate().map_err(|e| e.to_string())?;
    let policy = Configured::new(abuse.params);
    let mut rng = Rng::new(abuse.seed);
    let mut rates = Rates::new();
    let id = |n: usize| {
        let mut bytes = [0u8; 32];
        bytes[..8].copy_from_slice(&(n as u64 + 1).to_be_bytes());
        IdentityId(bytes)
    };
    let limit = |peer, now| {
        policy
            .limits(&PeerView {
                peer,
                trust_millionths: 0,
                summary: Summary::default(),
                now_seconds: now,
            })
            .requests_per_minute
    };
    let (mut abuse_allowed, mut abuse_total, mut honest_allowed, mut honest_total) =
        (0u64, 0u64, 0u64, 0u64);
    let burst_chance = abuse.honest_requests_per_minute / f64::from(abuse.honest_burst) / 60.0;
    for second in 0..abuse.seconds {
        let now = 1_000_000 + second;
        for n in 0..abuse.abusers {
            let peer = id(n);
            let allowed_limit = limit(peer, now);
            let mut got = 0u32;
            // The limiter refuses once the minute is full, so stop counting hits there.
            while got < abuse.abuser_requests_per_second && rates.hit(peer, now, allowed_limit) {
                got += 1;
            }
            abuse_allowed += u64::from(got);
            abuse_total += u64::from(abuse.abuser_requests_per_second);
        }
        for n in 0..abuse.honest {
            if !rng.chance(burst_chance) {
                continue;
            }
            let peer = id(abuse.abusers + n);
            let allowed_limit = limit(peer, now);
            for _ in 0..abuse.honest_burst {
                honest_total += 1;
                if rates.hit(peer, now, allowed_limit) {
                    honest_allowed += 1;
                }
            }
        }
    }
    let minutes = abuse.seconds as f64 / 60.0;
    let share = |allowed: u64, total: u64| {
        if total == 0 {
            0.0
        } else {
            1.0 - allowed as f64 / total as f64
        }
    };
    Ok(AbuseOutcome {
        abuser_allowed_per_minute: if abuse.abusers == 0 {
            0.0
        } else {
            abuse_allowed as f64 / abuse.abusers as f64 / minutes
        },
        abuser_refused: share(abuse_allowed, abuse_total),
        honest_refused: share(honest_allowed, honest_total),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // A limit parsed from a policy document.
    fn limited(per_minute: u32) -> Params {
        Params::parse(
            format!(r#"{{"version":1,"quota":{{"requestsPerMinute":{per_minute}}}}}"#).as_bytes(),
        )
        .unwrap()
    }

    // With no limit an abuser gets everything through; with a limit it gets about the limit per minute
    // while honest peers below the limit are not refused.
    #[test]
    fn a_limit_caps_abusers_and_spares_honest_peers() {
        let open = run(&Abuse::default()).unwrap();
        assert_eq!(open.abuser_refused, 0.0);
        assert!(
            (open.abuser_allowed_per_minute - 12_000.0).abs() < 1.0,
            "{open:?}"
        );
        let capped = run(&Abuse {
            params: limited(120),
            ..Abuse::default()
        })
        .unwrap();
        assert!(
            (110.0..=125.0).contains(&capped.abuser_allowed_per_minute),
            "{capped:?}"
        );
        assert!(capped.abuser_refused > 0.98, "{capped:?}");
        assert_eq!(capped.honest_refused, 0.0, "{capped:?}");
    }

    // A limit below honest demand refuses honest peers too: the trade-off the parameter tunes.
    #[test]
    fn a_tight_limit_refuses_honest_peers() {
        let tight = run(&Abuse {
            params: limited(3),
            ..Abuse::default()
        })
        .unwrap();
        assert!(tight.honest_refused > 0.1, "{tight:?}");
    }

    // Runs are reproducible, and empty experiments are refused.
    #[test]
    fn runs_are_reproducible_and_validated() {
        let a = Abuse {
            params: limited(30),
            ..Abuse::default()
        };
        assert_eq!(run(&a).unwrap(), run(&a).unwrap());
        assert!(
            run(&Abuse {
                seconds: 0,
                ..Abuse::default()
            })
            .is_err()
        );
        assert!(
            run(&Abuse {
                honest: 0,
                ..Abuse::default()
            })
            .is_err()
        );
        assert!(
            run(&Abuse {
                honest_burst: 0,
                ..Abuse::default()
            })
            .is_err()
        );
        assert!(
            run(&Abuse::default())
                .unwrap()
                .to_json()
                .get("honestRefused")
                .is_some()
        );
    }
}
