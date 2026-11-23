//! Requester-side judgement of a hosted publication byte check from private expectations and trusted timing.
//!
//! Callers: the Services `Rules` binding composed by the Services client, and the sender's own tests. The sender keeps the content it stored, so
//! it can compute the byte the provider must return; that expectation never leaves the requester and is never part of the challenge
//! or any log. Pass, fail and inconclusive follow the Storage approach: only complete, timely, matching evidence against a
//! known expectation can pass or fail; everything else is inconclusive and never a fail. Dependencies: operation values only.
use super::{
    ids::PlacementId,
    operation::{CheckOutput, CheckRequest},
};

/// The requester's conclusion about a check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The provider returned the byte it must hold.
    Pass,
    /// Complete, timely evidence contradicts the stored byte.
    Fail,
    /// Missing, late, ambiguous or mismatched evidence, or no expectation, prevents a conclusion.
    Inconclusive,
}

/// What the requester observed about the response time, never a provider assertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timing {
    /// Complete evidence arrived within the accepted response interval.
    Within,
    /// The original timing cannot be compared reliably (for example across a restart).
    Ambiguous,
    /// No complete evidence arrived, or it arrived after the interval.
    Missing,
}

/// The byte a provider must return for one challenge, known only to the requester.
#[derive(Clone, PartialEq, Eq)]
pub struct Expectation {
    placement_id: PlacementId,
    offset: u64,
    byte: u8,
}

impl Expectation {
    /// An expectation held by the requester for one challenge: the placement, the offset and the byte the provider must return.
    pub fn new(placement_id: PlacementId, offset: u64, byte: u8) -> Self {
        tracing::trace!("Creating a private byte-check expectation");
        Self {
            placement_id,
            offset,
            byte,
        }
    }

    /// Derive the expectation from the content the sender stored, or `None` when `offset` is outside it.
    pub fn from_content(placement_id: PlacementId, content: &[u8], offset: u64) -> Option<Self> {
        tracing::trace!("Deriving a private byte-check expectation");
        let byte = *content.get(usize::try_from(offset).ok()?)?;
        Some(Self {
            placement_id,
            offset,
            byte,
        })
    }
}

impl std::fmt::Debug for Expectation {
    // The expected byte is private requester knowledge and is never printed.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Expectation")
            .field("offset", &self.offset)
            .finish_non_exhaustive()
    }
}

/// Classify the response time: `Within` when it arrived no later than `response_seconds` after the request, `Missing` when it
/// arrived later or not at all, and `Ambiguous` when the clock could not measure it (`None` with `observed` true).
pub fn timing(elapsed_milliseconds: Option<u64>, observed: bool, response_seconds: u64) -> Timing {
    tracing::trace!("Classifying byte-check response timing");
    match elapsed_milliseconds {
        Some(elapsed) if elapsed <= response_seconds.saturating_mul(1000) => Timing::Within,
        Some(_) => Timing::Missing,
        None if observed => Timing::Ambiguous,
        None => Timing::Missing,
    }
}

/// Whether `evidence` is a complete answer to exactly this `challenge`; this is what justifies a charge, independently of whether the
/// byte is correct.
pub fn is_valid_evidence(challenge: &CheckRequest, evidence: &CheckOutput) -> bool {
    tracing::trace!("Validating byte-check evidence against its challenge");
    evidence.placement_id == challenge.placement_id && evidence.offset == challenge.offset
}

/// Judge a check: pass or fail only for timely, matching evidence against an expectation for this very challenge.
pub fn evaluate(
    challenge: &CheckRequest,
    evidence: Option<&CheckOutput>,
    expected: Option<&Expectation>,
    timing: Timing,
) -> Verdict {
    tracing::trace!("Judging a byte check from private expectations");
    if timing != Timing::Within {
        return Verdict::Inconclusive;
    }
    let (Some(evidence), Some(expected)) = (evidence, expected) else {
        return Verdict::Inconclusive;
    };
    if expected.placement_id != challenge.placement_id
        || expected.offset != challenge.offset
        || !is_valid_evidence(challenge, evidence)
    {
        return Verdict::Inconclusive;
    }
    if evidence.value == expected.byte {
        Verdict::Pass
    } else {
        Verdict::Fail
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLACEMENT: PlacementId = PlacementId([3; 32]);

    // A challenge for the placement at the offset.
    fn challenge(offset: u64) -> CheckRequest {
        CheckRequest {
            placement_id: PLACEMENT,
            offset,
        }
    }
    // Evidence for the placement at the offset with the byte.
    fn evidence(offset: u64, value: u8) -> CheckOutput {
        CheckOutput {
            placement_id: PLACEMENT,
            offset,
            value,
        }
    }

    // HP-13 requester side: matching timely evidence passes, a different byte fails, and every other situation (late, ambiguous or
    // missing timing, absent evidence or expectation, evidence for another offset or placement, an expectation for another
    // challenge) is inconclusive rather than a failure.
    #[test]
    fn only_timely_matching_evidence_passes_or_fails() {
        let expected = Expectation::from_content(PLACEMENT, b"abc", 1).unwrap();
        let c = challenge(1);
        assert_eq!(
            evaluate(
                &c,
                Some(&evidence(1, b'b')),
                Some(&expected),
                Timing::Within
            ),
            Verdict::Pass
        );
        assert_eq!(
            evaluate(
                &c,
                Some(&evidence(1, b'x')),
                Some(&expected),
                Timing::Within
            ),
            Verdict::Fail
        );
        for timing in [Timing::Ambiguous, Timing::Missing] {
            assert_eq!(
                evaluate(&c, Some(&evidence(1, b'x')), Some(&expected), timing),
                Verdict::Inconclusive
            );
        }
        assert_eq!(
            evaluate(&c, None, Some(&expected), Timing::Within),
            Verdict::Inconclusive
        );
        assert_eq!(
            evaluate(&c, Some(&evidence(1, b'b')), None, Timing::Within),
            Verdict::Inconclusive
        );
        assert_eq!(
            evaluate(
                &c,
                Some(&evidence(2, b'b')),
                Some(&expected),
                Timing::Within
            ),
            Verdict::Inconclusive
        );
        let other = CheckOutput {
            placement_id: PlacementId([4; 32]),
            offset: 1,
            value: b'b',
        };
        assert_eq!(
            evaluate(&c, Some(&other), Some(&expected), Timing::Within),
            Verdict::Inconclusive
        );
        assert_eq!(
            evaluate(
                &challenge(2),
                Some(&evidence(2, b'c')),
                Some(&expected),
                Timing::Within
            ),
            Verdict::Inconclusive
        );
    }

    // The expectation exists only for offsets inside the content, and never prints the expected byte.
    #[test]
    fn expectations_are_bounded_and_private() {
        assert!(Expectation::from_content(PLACEMENT, b"abc", 3).is_none());
        assert!(Expectation::from_content(PLACEMENT, b"", 0).is_none());
        assert!(Expectation::from_content(PLACEMENT, b"abc", u64::MAX).is_none());
        let printed = format!(
            "{:?}",
            Expectation::from_content(PLACEMENT, b"abc", 1).unwrap()
        );
        assert!(!printed.contains("byte") && !printed.contains("98"));
    }

    // Timing is within at exactly the interval, missing after it or when nothing arrived, and ambiguous only when a response was
    // observed but could not be timed.
    #[test]
    fn timing_is_classified_exactly() {
        assert_eq!(timing(Some(3000), true, 3), Timing::Within);
        assert_eq!(timing(Some(3001), true, 3), Timing::Missing);
        assert_eq!(timing(Some(0), true, 0), Timing::Within);
        assert_eq!(timing(None, true, 3), Timing::Ambiguous);
        assert_eq!(timing(None, false, 3), Timing::Missing);
        assert_eq!(timing(Some(u64::MAX), true, u64::MAX), Timing::Within);
    }

    // Evidence is chargeable when it answers exactly the challenge, whether or not the byte is right.
    #[test]
    fn evidence_validity_does_not_depend_on_correctness() {
        let c = challenge(1);
        assert!(is_valid_evidence(&c, &evidence(1, 0)));
        assert!(!is_valid_evidence(&c, &evidence(2, 0)));
    }
}
