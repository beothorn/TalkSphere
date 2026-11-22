//! Parameter sweeps over seeded runs.
//!
//! Callers: the `policy_sim` example and tests. Each sweep varies one family of policy parameters over
//! several seeds, averages the measurements and returns plain tables that render as Markdown or JSON.
//! The results answer "what does this parameter cost and buy under these assumptions", not "what will
//! happen": the assumptions are the model's, and real usage should replace them.
use super::{
    abuse::{self, Abuse},
    scenario::{Honest, Scenario},
    world::{self, Outcome},
};
use crate::composition::policy::Params;
use serde_json::{Value, json};

/// A titled table of strings.
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    /// What the table shows.
    pub title: String,
    /// Column headings.
    pub columns: Vec<String>,
    /// Rows of cells, one cell per column.
    pub rows: Vec<Vec<String>>,
}

impl Table {
    /// The table as Markdown.
    pub fn markdown(&self) -> String {
        tracing::trace!("Rendering a sweep table as Markdown");
        let mut out = format!(
            "### {}\n\n| {} |\n|{}\n",
            self.title,
            self.columns.join(" | "),
            "---|".repeat(self.columns.len())
        );
        for row in &self.rows {
            out.push_str(&format!("| {} |\n", row.join(" | ")));
        }
        out
    }

    /// The table as JSON.
    pub fn to_json(&self) -> Value {
        json!({"title": self.title, "columns": self.columns, "rows": self.rows})
    }
}

/// Size of a sweep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    /// Seeds averaged for every configuration.
    pub seeds: u64,
    /// Simulated days per run.
    pub days: u64,
    /// Hosted items per run.
    pub items: usize,
}

impl Default for Size {
    fn default() -> Self {
        Self {
            seeds: 5,
            days: 7,
            items: 20,
        }
    }
}

// The base scenario for a sweep size.
fn base(size: Size) -> Scenario {
    Scenario {
        horizon_seconds: size.days * 86_400,
        items: size.items,
        ..Scenario::default()
    }
}

// Mean of every measurement over seeds `1..=size.seeds` of one scenario.
fn mean(scenario: &Scenario, size: Size) -> Result<Outcome, String> {
    let runs: Vec<Outcome> = (1..=size.seeds)
        .map(|seed| {
            world::run(&Scenario {
                seed,
                ..scenario.clone()
            })
        })
        .collect::<Result<_, _>>()?;
    let n = runs.len() as f64;
    let avg = |f: fn(&Outcome) -> f64| runs.iter().map(f).sum::<f64>() / n;
    let count =
        |f: fn(&Outcome) -> u64| (runs.iter().map(f).sum::<u64>() as f64 / n).round() as u64;
    Ok(Outcome {
        seed: 0,
        not_redundant: avg(|o| o.not_redundant),
        under_replicated: avg(|o| o.under_replicated),
        target_satisfied: avg(|o| o.target_satisfied),
        target_met: avg(|o| o.target_met),
        retrievable: avg(|o| o.retrievable),
        at_risk: avg(|o| o.at_risk),
        mean_repair_seconds: avg(|o| o.mean_repair_seconds),
        repairs: count(|o| o.repairs),
        placements: count(|o| o.placements),
        checks_passed: count(|o| o.checks_passed),
        checks_failed: count(|o| o.checks_failed),
        incomplete: count(|o| o.incomplete),
        renewals: count(|o| o.renewals),
        credits_issued: count(|o| o.credits_issued),
        credits_spent: count(|o| o.credits_spent),
        credits_to_dishonest: count(|o| o.credits_to_dishonest),
        unpaid: count(|o| o.unpaid),
        mean_trust: avg(|o| o.mean_trust),
    })
}

// A percentage with one decimal.
fn pct(x: f64) -> String {
    format!("{:.1}%", x * 100.0)
}

// Credits per item per simulated day.
fn per_item_day(credits: u64, scenario: &Scenario) -> String {
    let item_days = scenario.items as f64 * scenario.horizon_seconds as f64 / 86_400.0;
    format!("{:.1}", credits as f64 / item_days)
}

/// Vary the replica target on the default flaky fleet.
/// # Errors
/// An invalid scenario.
pub fn targets(size: Size) -> Result<Table, String> {
    tracing::trace!("Sweeping the replica target");
    let mut rows = Vec::new();
    for target in [1usize, 2, 3, 5, 8, 10, 15] {
        let mut s = base(size);
        s.params.redundancy.target = target;
        let o = mean(&s, size)?;
        rows.push(vec![
            target.to_string(),
            pct(o.retrievable),
            pct(o.target_met),
            pct(o.not_redundant),
            format!("{:.1} h", o.mean_repair_seconds / 3600.0),
            per_item_day(o.credits_issued, &s),
            per_item_day(o.credits_spent, &s),
        ]);
    }
    Ok(Table {
        title: "Replica target on a flaky fleet".into(),
        columns: [
            "target",
            "retrievable",
            "target met",
            "not redundant",
            "mean repair",
            "issued/item-day",
            "spent/item-day",
        ]
        .map(String::from)
        .to_vec(),
        rows,
    })
}

/// Vary the check interval at a fixed target, with some storage loss so detection matters.
/// # Errors
/// An invalid scenario.
pub fn check_intervals(size: Size) -> Result<Table, String> {
    tracing::trace!("Sweeping the check interval");
    let mut rows = Vec::new();
    for hours in [1u64, 6, 24, 72] {
        let mut s = base(size);
        s.params.redundancy.target = 5;
        s.params.redundancy.check_seconds = hours * 3600;
        s.honest.wipe_per_day = 0.05;
        let o = mean(&s, size)?;
        rows.push(vec![
            format!("{hours} h"),
            pct(o.retrievable),
            pct(o.target_met),
            o.checks_failed.to_string(),
            o.incomplete.to_string(),
            per_item_day(o.credits_spent, &s),
        ]);
    }
    Ok(Table {
        title: "Check interval at target 5 with 5 % wipes per day".into(),
        columns: [
            "check every",
            "retrievable",
            "target met",
            "failed checks",
            "incomplete",
            "spent/item-day",
        ]
        .map(String::from)
        .to_vec(),
        rows,
    })
}

/// For three fleets, the target each needs to keep data retrievable 99 % of the time (or the best tried).
/// # Errors
/// An invalid scenario.
pub fn fleets(size: Size) -> Result<Table, String> {
    tracing::trace!("Sweeping fleet reliability");
    let fleets: [(&str, Honest); 3] = [
        (
            "reliable (12 h on, 2 h off, 0.2 %/day wipes)",
            Honest {
                mean_online_seconds: 12.0 * 3600.0,
                mean_offline_seconds: 2.0 * 3600.0,
                wipe_per_day: 0.002,
                unresponsive: 0.01,
                ..base(size).honest
            },
        ),
        ("flaky (4 h on, 8 h off, 1 %/day wipes)", base(size).honest),
        (
            "very flaky (1 h on, 12 h off, 3 %/day wipes)",
            Honest {
                mean_online_seconds: 3600.0,
                mean_offline_seconds: 12.0 * 3600.0,
                wipe_per_day: 0.03,
                unresponsive: 0.1,
                ..base(size).honest
            },
        ),
    ];
    let mut rows = Vec::new();
    for (name, honest) in fleets {
        let mut best = (0usize, 0.0f64);
        let mut found = None;
        for target in [2usize, 3, 5, 8, 10, 15, 20] {
            let mut s = base(size);
            s.honest = honest;
            s.params.redundancy.target = target;
            let o = mean(&s, size)?;
            best = (target, o.retrievable);
            if o.retrievable >= 0.99 {
                found = Some((
                    target,
                    o.retrievable,
                    per_item_day(o.credits_issued + o.credits_spent, &s),
                ));
                break;
            }
        }
        rows.push(match found {
            Some((target, retrievable, cost)) => {
                vec![name.into(), target.to_string(), pct(retrievable), cost]
            }
            None => vec![
                name.into(),
                format!("> 20 (best {} reached {})", best.0, pct(best.1)),
                pct(best.1),
                "-".into(),
            ],
        });
    }
    Ok(Table {
        title: "Smallest target that keeps data retrievable 99 % of the time".into(),
        columns: ["fleet", "target", "retrievable", "credits/item-day"]
            .map(String::from)
            .to_vec(),
        rows,
    })
}

/// Vary the share of dishonest clients at target 5.
/// # Errors
/// An invalid scenario.
pub fn dishonest(size: Size) -> Result<Table, String> {
    tracing::trace!("Sweeping the dishonest share");
    let mut rows = Vec::new();
    for share in [0.0f64, 0.1, 0.3, 0.5] {
        let mut s = base(size);
        s.params.redundancy.target = 5;
        s.honest.dishonest_fraction = share;
        let o = mean(&s, size)?;
        rows.push(vec![
            pct(share),
            pct(o.retrievable),
            pct(o.target_met),
            o.checks_failed.to_string(),
            format!(
                "{:.0} %",
                100.0 * o.credits_to_dishonest as f64 / o.credits_issued.max(1) as f64
            ),
        ]);
    }
    Ok(Table {
        title: "Dishonest clients at target 5".into(),
        columns: [
            "dishonest share",
            "retrievable",
            "target met",
            "failed checks",
            "credits to dishonest",
        ]
        .map(String::from)
        .to_vec(),
        rows,
    })
}

/// Vary the per-identity request limit against flooding.
/// # Errors
/// An invalid experiment.
pub fn rate_limits(seconds: u64) -> Result<Table, String> {
    tracing::trace!("Sweeping the request limit");
    let mut rows = Vec::new();
    for limit in [None, Some(600u32), Some(120), Some(60), Some(30), Some(12)] {
        let params = match limit {
            None => Params::default(),
            Some(n) => Params::parse(
                format!(r#"{{"version":1,"quota":{{"requestsPerMinute":{n}}}}}"#).as_bytes(),
            )
            .map_err(|e| e.to_string())?,
        };
        let o = abuse::run(&Abuse {
            params,
            seconds,
            ..Abuse::default()
        })?;
        rows.push(vec![
            limit.map_or("none".into(), |n| n.to_string()),
            format!("{:.0}", o.abuser_allowed_per_minute),
            pct(o.abuser_refused),
            pct(o.honest_refused),
        ]);
    }
    Ok(Table {
        title: "Request limit per identity per minute (10 abusers at 200/s, honest peers about 12/min in bursts of 3)".into(),
        columns: ["requestsPerMinute", "abuser gets through /min", "abuser refused", "honest refused"].map(String::from).to_vec(),
        rows,
    })
}

/// Every sweep.
/// # Errors
/// An invalid scenario.
pub fn all(size: Size) -> Result<Vec<Table>, String> {
    tracing::trace!("Running every policy sweep");
    Ok(vec![
        targets(size)?,
        fleets(size)?,
        check_intervals(size)?,
        dishonest(size)?,
        rate_limits(600)?,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    // A tiny size so the tests stay quick.
    const TINY: Size = Size {
        seeds: 2,
        days: 3,
        items: 6,
    };

    // Every sweep returns a rectangular table, in the same order and with the same content each time.
    #[test]
    fn sweeps_return_rectangular_reproducible_tables() {
        let tables = all(TINY).unwrap();
        assert_eq!(tables.len(), 5);
        for table in &tables {
            assert!(!table.rows.is_empty(), "{}", table.title);
            assert!(
                table.rows.iter().all(|r| r.len() == table.columns.len()),
                "{}",
                table.title
            );
            assert!(table.markdown().starts_with("### "));
            // The separator row has exactly one cell per column.
            let separator = table.markdown().lines().nth(3).unwrap().to_string();
            assert_eq!(
                separator,
                format!("|{}", "---|".repeat(table.columns.len()))
            );
            assert!(table.to_json()["rows"].is_array());
        }
        assert_eq!(targets(TINY).unwrap(), tables[0], "a sweep is reproducible");
    }

    // More copies buy more retrievability in the target sweep: the last row beats the first.
    #[test]
    fn the_target_sweep_shows_diminishing_but_positive_returns() {
        let table = targets(TINY).unwrap();
        let first: f64 = table.rows[0][1].trim_end_matches('%').parse().unwrap();
        let last: f64 = table.rows[table.rows.len() - 1][1]
            .trim_end_matches('%')
            .parse()
            .unwrap();
        assert!(last > first + 5.0, "{first} -> {last}");
    }

    // The rate-limit sweep shows the trade-off: unlimited lets everything through, the tightest limit
    // refuses honest peers, and a generous limit refuses nobody honest.
    #[test]
    fn the_rate_limit_sweep_shows_the_tradeoff() {
        let table = rate_limits(120).unwrap();
        assert_eq!(table.rows[0][0], "none");
        assert_eq!(table.rows[0][2], "0.0%");
        assert_eq!(table.rows[2][3], "0.0%");
        assert_ne!(table.rows[table.rows.len() - 1][3], "0.0%");
    }
}
