//! Exact positive service prices; only Computation version one admits metering.
use super::{Error, assessment::Assessed};

/// Complete immutable price authorization accepted before execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Price {
    /// Exact positive price required for every successful completion.
    Fixed(u64),
    /// Computation rate in credits per billed millisecond, with accepted duration bound.
    Computation {
        /// Positive minimum charge.
        minimum: u64,
        /// Nonnegative rate numerator; zero charges only the minimum.
        numerator: u64,
        /// Positive rate denominator.
        denominator: u64,
        /// Positive maximum billed milliseconds.
        milliseconds: u64,
    },
}
impl Price {
    /// Validate service eligibility and calculate the accepted maximum using overflow-safe exact arithmetic.
    /// # Errors
    /// Rejects zero/unsupported parameters and unrepresentable uint64 prices.
    pub fn maximum(self, service: &str, version: u32) -> Result<u64, Error> {
        tracing::trace!("Calculating exact accepted service price bound");
        match self {
            Self::Fixed(p) if p > 0 => Ok(p),
            Self::Fixed(_) => Err(Error::Invalid),
            Self::Computation {
                minimum,
                numerator,
                denominator,
                milliseconds,
            } => {
                if service != "cr2se.computation" || version != 1 {
                    return Err(Error::Unsupported);
                }
                calculate(minimum, numerator, denominator, milliseconds)
            }
        }
    }
    /// Validate final billed duration and calculate price without accepting handler-reported arithmetic.
    /// Fixed work must not introduce a duration after acceptance.
    /// # Errors
    /// Invalid duration, unsupported metering and arithmetic overflow fail before settlement.
    pub fn final_price(
        self,
        service: &str,
        version: u32,
        billed: Option<u64>,
    ) -> Result<u64, Error> {
        tracing::trace!("Recalculating final price from accepted exact formula");
        self.maximum(service, version)?;
        match self {
            Self::Fixed(p) if billed.is_none() => Ok(p),
            Self::Fixed(_) => Err(Error::Invalid),
            Self::Computation {
                minimum,
                numerator,
                denominator,
                milliseconds,
            } => {
                let billed = billed
                    .filter(|b| *b >= 1 && *b <= milliseconds)
                    .ok_or(Error::Invalid)?;
                calculate(minimum, numerator, denominator, billed)
            }
        }
    }
}
// u128 holds every product of two uint64 values; quotient/remainder avoids ceiling-add overflow.
fn calculate(
    minimum: u64,
    numerator: u64,
    denominator: u64,
    milliseconds: u64,
) -> Result<u64, Error> {
    tracing::trace!("Calculating checked rational service charge");
    if [minimum, denominator, milliseconds].contains(&0) {
        return Err(Error::Invalid);
    }
    let product = u128::from(numerator) * u128::from(milliseconds);
    let denominator = u128::from(denominator);
    let price = product / denominator + u128::from(product % denominator != 0);
    Ok(minimum.max(u64::try_from(price).map_err(|_| Error::Limit)?))
}
/// Calculate from installed family rules and enforce the explicit requester bound before acceptance.
/// # Errors
/// Unsupported models, zero/overflow prices and insufficient requester bounds fail without execution.
pub fn quote(assessed: &Assessed, requester_maximum: u64) -> Result<Price, Error> {
    tracing::trace!("Authorizing quoted price within explicit requester spending bound");
    let auth = assessed.authorized();
    let o = auth.selection().offering();
    let price = auth
        .implementation()
        .requester()
        .pricing(o, assessed.factors())?;
    if price.maximum(&o.service, o.version)? > requester_maximum {
        return Err(Error::Insufficient);
    }
    Ok(price)
}
#[cfg(test)]
mod tests {
    use super::*;
    // Independent SV-06 arithmetic pins max60/final2 and rejects ordinary post-execution metering.
    #[test]
    fn metering_and_bounds() {
        let p = Price::Computation {
            minimum: 1,
            numerator: 1,
            denominator: 1000,
            milliseconds: 60000,
        };
        assert_eq!(p.maximum("cr2se.computation", 1), Ok(60));
        assert_eq!(p.final_price("cr2se.computation", 1, Some(1201)), Ok(2));
        assert_eq!(p.final_price("cr2se.computation", 1, Some(60000)), Ok(60));
        assert_eq!(p.maximum("org.example.echo", 1), Err(Error::Unsupported));
        for b in [None, Some(0), Some(60001)] {
            assert_eq!(
                p.final_price("cr2se.computation", 1, b),
                Err(Error::Invalid)
            );
        }
        assert_eq!(Price::Fixed(7).final_price("custom", 1, None), Ok(7));
        assert_eq!(
            Price::Fixed(7).final_price("custom", 1, Some(1)),
            Err(Error::Invalid)
        );
        assert_eq!(Price::Fixed(0).maximum("custom", 1), Err(Error::Invalid));
        assert_eq!(calculate(7, 0, 1000, 60000), Ok(7));
        assert_eq!(calculate(1, u64::MAX, 1, 2), Err(Error::Limit));
        assert_eq!(calculate(1, u64::MAX, u64::MAX, u64::MAX), Ok(u64::MAX));
        for tuple in [(0, 1, 1, 1), (1, 1, 0, 1), (1, 1, 1, 0)] {
            assert_eq!(
                calculate(tuple.0, tuple.1, tuple.2, tuple.3),
                Err(Error::Invalid)
            );
        }
    }
}
