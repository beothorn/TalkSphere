//! Deterministic policy simulation: one run, a flooding experiment and parameter sweeps.
//!
//! Exit status 0 is help or success, 1 a runtime failure and 2 invalid arguments. The simulation
//! uses the real policy and replica code against a modelled fleet and writes only to stdout.

const OVERVIEW: &str = "Usage: cargo run --locked --release --example policy_sim -- [run | abuse | sweep | help [COMMAND]] [OPTION VALUE]...\n\nSimulate the peer policy against an unreliable client fleet.\nrun    one seeded run printed as JSON\nabuse  a request-flooding experiment printed as JSON\nsweep  parameter sweeps printed as Markdown tables\nDetailed help: cargo run --locked --example policy_sim -- help COMMAND\n";
const RUN_HELP: &str = "Usage: policy_sim run [--seed N] [--days N] [--items N] [--clients N] [--target N] [--check-seconds N]\n\nRuns one deterministic simulation and prints its measurements as JSON: the share of item-time per CR2SE redundancy state, whether the target was met, how often the data could be fetched back from a connected holder (retrievable), repair times, placements, checks, renewals and credits. Defaults: seed 1, 7 days, 20 items, 40 clients, target 3, check every 21600 seconds. Every option takes a positive integer. The same options always give the same output.\n";
const ABUSE_HELP: &str = "Usage: policy_sim abuse [--limit N] [--seconds N] [--seed N]\n\nTen abusive peers attempt 200 requests a second and fifty honest peers make about 12 requests a minute in bursts of three, counted by the real rolling-minute limiter. --limit is quota.requestsPerMinute (default: no limit). Prints, as JSON, the requests per minute an abuser gets through and the shares of abuser and honest requests refused. Default 600 seconds.\n";
const SWEEP_HELP: &str = "Usage: policy_sim sweep [--seeds N] [--days N] [--items N] [--json yes]\n\nRuns every sweep: replica target, fleet reliability, check interval, dishonest share and request limit, each averaged over seeds 1 to N (default 5, 7 days, 20 items). Prints Markdown tables, or JSON with --json yes. The results are figures under the model's assumptions, not predictions; see docs/policy/SIMULATIONS.md. Use --release for speed.\n";

#[cfg(target_os = "linux")]
mod linux {
    use std::collections::BTreeMap;
    use talksphere::composition::node::sim::{
        abuse::{self, Abuse},
        scenario::Scenario,
        sweep::{self, Size},
        world,
    };
    use talksphere::composition::policy::Params;

    // Parse `--option value` pairs, each given at most once.
    fn options(args: &[String], allowed: &[&str]) -> Result<BTreeMap<String, u64>, String> {
        let mut found = BTreeMap::new();
        let mut words = args.iter();
        while let Some(name) = words.next() {
            if !allowed.contains(&name.as_str()) {
                return Err(format!("option {name:?} is not valid here"));
            }
            let value = words
                .next()
                .and_then(|v| v.parse::<u64>().ok())
                .filter(|v| *v > 0 || name == "--json")
                .ok_or_else(|| format!("{name} needs a positive integer"))?;
            if found.insert(name.clone(), value).is_some() {
                return Err(format!("{name} was given twice"));
            }
        }
        Ok(found)
    }

    // Run one command; the error carries the status it should exit with.
    pub fn execute(command: &str, args: &[String]) -> Result<String, (i32, String)> {
        let invalid = |m: String| (2, m);
        match command {
            "run" => {
                let o = options(
                    args,
                    &[
                        "--seed",
                        "--days",
                        "--items",
                        "--clients",
                        "--target",
                        "--check-seconds",
                    ],
                )
                .map_err(invalid)?;
                let mut s = Scenario {
                    seed: o.get("--seed").copied().unwrap_or(1),
                    horizon_seconds: o.get("--days").copied().unwrap_or(7) * 86_400,
                    items: o.get("--items").copied().unwrap_or(20) as usize,
                    ..Scenario::default()
                };
                s.honest.count = o.get("--clients").copied().unwrap_or(40) as usize;
                if let Some(t) = o.get("--target") {
                    s.params.redundancy.target = *t as usize;
                }
                if let Some(c) = o.get("--check-seconds") {
                    s.params.redundancy.check_seconds = *c;
                }
                let outcome = world::run(&s).map_err(|m| (1, m))?;
                Ok(serde_json::to_string_pretty(&outcome.to_json()).unwrap_or_default() + "\n")
            }
            "abuse" => {
                let o = options(args, &["--limit", "--seconds", "--seed"]).map_err(invalid)?;
                let params = match o.get("--limit") {
                    None => Params::default(),
                    Some(n) => Params::parse(
                        format!(r#"{{"version":1,"quota":{{"requestsPerMinute":{n}}}}}"#)
                            .as_bytes(),
                    )
                    .map_err(|e| invalid(e.to_string()))?,
                };
                let experiment = Abuse {
                    seed: o.get("--seed").copied().unwrap_or(1),
                    params,
                    seconds: o.get("--seconds").copied().unwrap_or(600),
                    ..Abuse::default()
                };
                let outcome = abuse::run(&experiment).map_err(|m| (1, m))?;
                Ok(serde_json::to_string_pretty(&outcome.to_json()).unwrap_or_default() + "\n")
            }
            "sweep" => {
                let o =
                    options(args, &["--seeds", "--days", "--items", "--json"]).map_err(invalid)?;
                let size = Size {
                    seeds: o.get("--seeds").copied().unwrap_or(5),
                    days: o.get("--days").copied().unwrap_or(7),
                    items: o.get("--items").copied().unwrap_or(20) as usize,
                };
                let tables = sweep::all(size).map_err(|m| (1, m))?;
                if o.contains_key("--json") {
                    let json: Vec<_> = tables.iter().map(|t| t.to_json()).collect();
                    return Ok(serde_json::to_string_pretty(&json).unwrap_or_default() + "\n");
                }
                Ok(tables
                    .iter()
                    .map(|t| t.markdown())
                    .collect::<Vec<_>>()
                    .join("\n"))
            }
            other => Err((2, format!("unknown command {other:?}"))),
        }
    }
}

// Choose the text for help requests, or None when a command should run.
fn help(args: &[String]) -> Option<&'static str> {
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] | ["--help"] | ["-h"] | ["help"] => Some(OVERVIEW),
        ["help", "run"] => Some(RUN_HELP),
        ["help", "abuse"] => Some(ABUSE_HELP),
        ["help", "sweep"] => Some(SWEEP_HELP),
        _ => None,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(text) = help(&args) {
        print!("{text}");
        return;
    }
    #[cfg(target_os = "linux")]
    {
        let (command, rest) = args
            .split_first()
            .map_or(("", &[][..]), |(c, r)| (c.as_str(), r));
        match linux::execute(command, rest) {
            Ok(output) => print!("{output}"),
            Err((status, message)) => {
                eprintln!("{message}");
                std::process::exit(status);
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        eprintln!("the policy simulation is unsupported on this platform");
        std::process::exit(1);
    }
}
