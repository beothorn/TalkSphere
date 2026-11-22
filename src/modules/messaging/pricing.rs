//! Exact rational Messaging prices: one upward rounding after adding both exact terms, overflow-safe.
//!
//! Callers: offering validation, quoting and the provider. Floating point is never used for credits.
use super::error::Error;

/// Exact nonnegative credit rate per applicable unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rate {
    /// Nonnegative credit numerator, including zero.
    pub numerator: u64,
    /// Positive unit denominator; zero is rejected by every calculation.
    pub denominator: u64,
}

/// Compute store price with one ceiling after adding both exact rational terms.
/// # Errors
/// Invalid rejects zero size/duration/minimum/denominators; Limit rejects a final uint64 overflow.
pub fn store(
    size: u64,
    seconds: u64,
    upload: Rate,
    retention: Rate,
    minimum: u64,
) -> Result<u64, Error> {
    tracing::trace!("Calculating exact Messaging upload and retention charge");
    if size == 0 || seconds == 0 || minimum == 0 {
        return Err(Error::Invalid);
    }
    let (whole_a, rem_a) = term(size, 1, upload)?;
    let (whole_b, rem_b) = term(size, seconds, retention)?;
    let denominator = u128::from(upload.denominator) * u128::from(retention.denominator);
    let left = rem_a * u128::from(retention.denominator);
    let right = rem_b * u128::from(upload.denominator);
    // Each product fits uint128, but their sum need not. Compare by subtraction instead.
    let fractional_ceiling = if left == 0 && right == 0 {
        0
    } else if left > denominator - right {
        2
    } else {
        1
    };
    let price = whole_a
        .checked_add(whole_b)
        .and_then(|p| p.checked_add(fractional_ceiling))
        .ok_or(Error::Limit)?;
    finish(price, minimum)
}

/// Compute a recovery or renewal price from a positive byte count and positive duration units.
/// Recovery uses units=1; renewal uses the exact additional seconds. See [`recover`] and [`renew`].
/// # Errors
/// Invalid rejects zero inputs/minimum/denominator; Limit rejects unrepresentable final prices.
pub fn usage(size: u64, units: u64, rate: Rate, minimum: u64) -> Result<u64, Error> {
    tracing::trace!("Calculating exact Messaging usage charge");
    if size == 0 || units == 0 || minimum == 0 {
        return Err(Error::Invalid);
    }
    let (whole, remainder) = term(size, units, rate)?;
    finish(whole + u128::from(remainder != 0), minimum)
}

// A product above uint128 necessarily exceeds uint64 after division by a uint64 denominator.
// Thus rejecting multiplication overflow cannot reject any representable valid final price.
fn term(size: u64, units: u64, rate: Rate) -> Result<(u128, u128), Error> {
    tracing::trace!("Splitting exact Messaging rate into integer and fractional parts");
    if rate.denominator == 0 {
        return Err(Error::Invalid);
    }
    let numerator = (u128::from(size) * u128::from(units))
        .checked_mul(u128::from(rate.numerator))
        .ok_or(Error::Limit)?;
    let denominator = u128::from(rate.denominator);
    Ok((numerator / denominator, numerator % denominator))
}

// Convert only the exact final integer, preserving minimum as a floor rather than an added fee.
fn finish(price: u128, minimum: u64) -> Result<u64, Error> {
    tracing::trace!("Checking exact Messaging price fits uint64 credits");
    Ok(minimum.max(u64::try_from(price).map_err(|_| Error::Limit)?))
}

/// Price of recovering a payload of `size` bytes: `max(minimum, ceil(size * R))`.
/// # Errors
/// `Invalid` for zero size, minimum or denominator; `Limit` when the result exceeds uint64.
pub fn recover(size: u64, rate: Rate, minimum: u64) -> Result<u64, Error> {
    tracing::trace!("Calculating the exact Messaging recovery price");
    usage(size, 1, rate, minimum)
}

/// Price of adding `additional_seconds` of retention to `size` bytes: `max(minimum, ceil(size * seconds * N))`.
/// # Errors
/// `Invalid` for zero size, seconds, minimum or denominator; `Limit` when the result exceeds uint64.
pub fn renew(size: u64, additional_seconds: u64, rate: Rate, minimum: u64) -> Result<u64, Error> {
    tracing::trace!("Calculating the exact Messaging renewal price");
    usage(size, additional_seconds, rate, minimum)
}

#[cfg(test)]
mod tests {
    use super::*;
    const HALF: Rate = Rate {
        numerator: 1,
        denominator: 2,
    };

    // The acceptance harness prices: store of `abc` for 10 seconds is 17 (MS-02), a single byte for a
    // single second is 1 and not 2 (MS-03: both terms are added before one rounding), recovery of `abc`
    // is 2 (MS-07) and renewing it for 4 seconds is 6 (MS-10).
    #[test]
    fn acceptance_harness_prices() {
        assert_eq!(store(3, 10, HALF, HALF, 1), Ok(17));
        assert_eq!(store(1, 1, HALF, HALF, 1), Ok(1));
        assert_eq!(recover(3, HALF, 1), Ok(2));
        assert_eq!(renew(3, 4, HALF, 1), Ok(6));
    }

    // Rounding happens once: two thirds of a credit on each term still costs two credits, one third each costs one.
    #[test]
    fn rounding_is_applied_once_after_adding() {
        let third = Rate {
            numerator: 1,
            denominator: 3,
        };
        let two_thirds = Rate {
            numerator: 2,
            denominator: 3,
        };
        assert_eq!(store(1, 1, third, third, 1), Ok(1));
        assert_eq!(store(1, 1, two_thirds, two_thirds, 1), Ok(2));
        // Exact integers add without rounding.
        assert_eq!(
            store(
                2,
                2,
                Rate {
                    numerator: 1,
                    denominator: 1
                },
                Rate {
                    numerator: 1,
                    denominator: 1
                },
                1
            ),
            Ok(6)
        );
    }

    // The minimum price is a floor, not a fee on top of the calculated price.
    #[test]
    fn the_minimum_is_a_floor() {
        assert_eq!(store(1, 1, HALF, HALF, 5), Ok(5));
        assert_eq!(store(3, 10, HALF, HALF, 5), Ok(17));
        assert_eq!(recover(1, HALF, 9), Ok(9));
        assert_eq!(
            renew(
                1,
                1,
                Rate {
                    numerator: 0,
                    denominator: 7
                },
                4
            ),
            Ok(4)
        );
    }

    // Zero sizes, durations, minimums and denominators are invalid, in every function.
    #[test]
    fn zero_inputs_are_invalid() {
        let zero_denominator = Rate {
            numerator: 1,
            denominator: 0,
        };
        assert_eq!(store(0, 1, HALF, HALF, 1), Err(Error::Invalid));
        assert_eq!(store(1, 0, HALF, HALF, 1), Err(Error::Invalid));
        assert_eq!(store(1, 1, HALF, HALF, 0), Err(Error::Invalid));
        assert_eq!(store(1, 1, zero_denominator, HALF, 1), Err(Error::Invalid));
        assert_eq!(store(1, 1, HALF, zero_denominator, 1), Err(Error::Invalid));
        assert_eq!(recover(0, HALF, 1), Err(Error::Invalid));
        assert_eq!(recover(1, zero_denominator, 1), Err(Error::Invalid));
        assert_eq!(renew(1, 0, HALF, 1), Err(Error::Invalid));
        assert_eq!(renew(1, 1, HALF, 0), Err(Error::Invalid));
    }

    // A result above uint64 is a limit failure, while the largest representable price is exact.
    #[test]
    fn overflow_is_a_limit_not_a_wrap() {
        let one = Rate {
            numerator: 1,
            denominator: 1,
        };
        assert_eq!(recover(u64::MAX, one, 1), Ok(u64::MAX));
        assert_eq!(
            recover(
                u64::MAX,
                Rate {
                    numerator: 2,
                    denominator: 1
                },
                1
            ),
            Err(Error::Limit)
        );
        assert_eq!(renew(u64::MAX, u64::MAX, one, 1), Err(Error::Limit));
        assert_eq!(store(u64::MAX, 1, one, one, 1), Err(Error::Limit));
        // The two terms are each representable but their sum is not.
        assert_eq!(
            store(
                u64::MAX,
                1,
                one,
                Rate {
                    numerator: 1,
                    denominator: 1
                },
                1
            ),
            Err(Error::Limit)
        );
        // A huge numerator over a huge denominator that divides back into range stays exact.
        let big = Rate {
            numerator: u64::MAX,
            denominator: u64::MAX,
        };
        assert_eq!(recover(u64::MAX, big, 1), Ok(u64::MAX));
    }
}
