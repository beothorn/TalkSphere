use std::{error::Error, fmt};

/// Local scaffold failure; this is not a CR2SE wire error code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportError {
    /// The setup milestone does not implement networking.
    NotImplemented,
}

impl fmt::Display for TransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotImplemented => formatter.write_str(
                "Transport is not implemented yet; no server was started. See plans/PLAN.md step 2.",
            ),
        }
    }
}

impl Error for TransportError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffold_error_explains_the_failure_and_next_step() {
        let error: &dyn Error = &TransportError::NotImplemented;
        let message = error.to_string();
        assert!(message.contains("not implemented"));
        assert!(message.contains("no server was started"));
        assert!(message.contains("plans/PLAN.md step 2"));
        assert!(error.source().is_none());
    }
}
