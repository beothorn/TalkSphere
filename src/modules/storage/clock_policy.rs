//! Explicit requester-local acceptance of provider UTC estimates; no peer clock controls this policy.
use crate::modules::services::Error;

/// Reject a returned provider acceptance time whose distance from independently observed local UTC is excessive.
/// All parameters use whole Unix seconds, and `maximum_difference_seconds` is authorized local policy.
/// # Errors
/// Invalid denotes a clock estimate beyond the permitted skew, including future and past estimates.
pub fn validate(
    accepted_at: u64,
    observed_local_unix_seconds: u64,
    maximum_difference_seconds: u64,
) -> Result<(), Error> {
    tracing::trace!("Checking Storage provider acceptance time against requester clock policy");
    if accepted_at.abs_diff(observed_local_unix_seconds) > maximum_difference_seconds {
        Err(Error::Invalid)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Both skew directions include the exact limit and reject just outside it without signed arithmetic overflow.
    #[test]
    fn requester_controls_clock_skew() {
        assert_eq!(validate(1000, 1000, 0), Ok(()));
        assert_eq!(validate(995, 1000, 5), Ok(()));
        assert_eq!(validate(1005, 1000, 5), Ok(()));
        assert_eq!(validate(994, 1000, 5), Err(Error::Invalid));
        assert_eq!(validate(1006, 1000, 5), Err(Error::Invalid));
        assert_eq!(validate(u64::MAX, 0, u64::MAX), Ok(()));
        assert_eq!(validate(u64::MAX, 0, u64::MAX - 1), Err(Error::Invalid));
    }
}
