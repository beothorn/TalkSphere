//! Shared application logging setup; protocol modules emit events without installing subscribers.
use tracing_subscriber::EnvFilter;

/// Install stderr operational logging using RUST_LOG, defaulting to info.
/// # Errors
/// Rejects invalid/non-Unicode filters or an already installed global subscriber with a fixed diagnostic.
pub fn initialize() -> Result<(), &'static str> {
    let input = match std::env::var("RUST_LOG") {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => "info".into(),
        Err(_) => return Err("RUST_LOG must be Unicode"),
    };
    let filter = parse_filter(&input)?;
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .without_time()
        .try_init()
        .map_err(|_| "logging subscriber initialization failed")?;
    tracing::trace!("Configured shared operational logging");
    Ok(())
}
// Disable regex matching of event values; filtering should use level and target rather than peer data.
fn parse_filter(input: &str) -> Result<EnvFilter, &'static str> {
    EnvFilter::builder()
        .with_regex(false)
        .parse(input)
        .map_err(|_| "invalid RUST_LOG filter")
}
#[cfg(test)]
mod tests {
    use super::*;
    // Valid target/level directives are accepted; malformed filters fail instead of silently using defaults.
    #[test]
    fn logging_filters_are_explicit() {
        for value in [
            "info",
            "off",
            "talksphere=trace",
            "warn,talksphere::modules::transport=debug",
        ] {
            assert!(parse_filter(value).is_ok());
        }
        assert!(parse_filter("talksphere=not_a_level").is_err());
    }
}
