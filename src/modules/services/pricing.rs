//! Checked positive fixed pricing before input transfer or execution.
use super::{Error, assessment::Assessed};
/// Complete immutable fixed-price authorization, later extended only for the Computation exception.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Price {
    /// Exact positive charge for a valid successful completion.
    Fixed(u64),
}
impl Price {
    /// Validate positivity and return the exact maximum accepted price.
    /// # Errors
    /// Invalid rejects zero; service/version are reserved for family-specific eligibility checks.
    pub fn maximum(self, _service: &str, _version: u32) -> Result<u64, Error> {
        tracing::trace!("Calculating positive fixed service price");
        match self {
            Self::Fixed(p) if p > 0 => Ok(p),
            _ => Err(Error::Invalid),
        }
    }
    /// Fixed work has no undisclosed post-execution duration or fees.
    /// # Errors
    /// Invalid rejects zero or an unexpected billed duration.
    pub fn final_price(
        self,
        service: &str,
        version: u32,
        billed: Option<u64>,
    ) -> Result<u64, Error> {
        tracing::trace!("Checking exact fixed completion price");
        self.maximum(service, version)?;
        if billed.is_some() {
            return Err(Error::Invalid);
        }
        let Self::Fixed(p) = self;
        Ok(p)
    }
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
    // Fixed success is exact and rejects zero or post-execution metering.
    #[test]
    fn fixed_prices() {
        assert_eq!(Price::Fixed(7).maximum("custom", 1), Ok(7));
        assert_eq!(
            Price::Fixed(u64::MAX).final_price("custom", 1, None),
            Ok(u64::MAX)
        );
        assert_eq!(Price::Fixed(0).maximum("custom", 1), Err(Error::Invalid));
        assert_eq!(
            Price::Fixed(7).final_price("custom", 1, Some(1)),
            Err(Error::Invalid)
        );
    }
}
