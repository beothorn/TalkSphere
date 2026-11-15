//! Exact rational Storage prices; split quotients/remainders avoid arbitrary-width sums.
use crate::modules::services::Error;

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
    tracing::trace!("Calculating exact Storage upload and retention charge");
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

/// Compute retrieval or renewal price from positive byte count and positive duration units.
/// Retrieval uses units=1; renewal uses the exact additional seconds.
/// # Errors
/// Invalid rejects zero inputs/minimum/denominator; Limit rejects unrepresentable final prices.
pub fn usage(size: u64, units: u64, rate: Rate, minimum: u64) -> Result<u64, Error> {
    tracing::trace!("Calculating exact Storage usage charge");
    if size == 0 || units == 0 || minimum == 0 {
        return Err(Error::Invalid);
    }
    let (whole, remainder) = term(size, units, rate)?;
    finish(whole + u128::from(remainder != 0), minimum)
}

// A product above uint128 necessarily exceeds uint64 after division by a uint64 denominator.
// Thus rejecting multiplication overflow cannot reject any representable valid final price.
fn term(size: u64, units: u64, rate: Rate) -> Result<(u128, u128), Error> {
    tracing::trace!("Splitting exact Storage rate into integer and fractional parts");
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
    tracing::trace!("Checking exact Storage price fits uint64 credits");
    Ok(minimum.max(u64::try_from(price).map_err(|_| Error::Limit)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    const HALF: Rate = Rate {
        numerator: 1,
        denominator: 2,
    };
    // Literal ST-01 arithmetic distinguishes single rounding from per-term rounding.
    #[test]
    fn upstream_literal_prices() {
        assert_eq!(store(1, 1, HALF, HALF, 1), Ok(1));
        assert_eq!(store(3, 10, HALF, HALF, 1), Ok(17));
        assert_eq!(usage(3, 1, HALF, 1), Ok(2));
        assert_eq!(usage(3, 4, HALF, 1), Ok(6));
        assert_eq!(
            store(
                1,
                1,
                Rate {
                    numerator: 1,
                    denominator: 3
                },
                HALF,
                1
            ),
            Ok(1)
        );
        assert_eq!(
            store(
                1,
                1,
                Rate {
                    numerator: 2,
                    denominator: 3
                },
                HALF,
                1
            ),
            Ok(2)
        );
    }
    // Extreme factors exercise >128-bit conceptual products, zero rates, and final-width rejection.
    #[test]
    fn exact_extrema_and_overflow() {
        let max = Rate {
            numerator: u64::MAX,
            denominator: u64::MAX,
        };
        let zero = Rate {
            numerator: 0,
            denominator: u64::MAX,
        };
        assert_eq!(usage(u64::MAX, 1, max, 1), Ok(u64::MAX));
        assert_eq!(usage(u64::MAX, u64::MAX, zero, 7), Ok(7));
        assert_eq!(usage(u64::MAX, u64::MAX, max, 1), Err(Error::Limit));
        assert_eq!(store(u64::MAX, 1, zero, max, 1), Ok(u64::MAX));
        assert_eq!(store(u64::MAX, 1, max, max, 1), Err(Error::Limit));
        assert_eq!(
            usage(
                9_007_199_254_740_993,
                1,
                Rate {
                    numerator: 1,
                    denominator: 1
                },
                1
            ),
            Ok(9_007_199_254_740_993)
        );
        // Fraction cross-products approach uint128 maximum; their sum exceeds it.
        let near = Rate {
            numerator: u64::MAX - 1,
            denominator: u64::MAX,
        };
        assert_eq!(store(1, 1, near, near, 1), Ok(2));
    }
    // A small independent common-denominator oracle exercises both sides of every rounding boundary.
    #[test]
    fn small_rational_grid_matches_independent_oracle() {
        for size in 1u64..5 {
            for seconds in 1u64..5 {
                for bn in 0u64..4 {
                    for bd in 1u64..5 {
                        for xn in 0u64..4 {
                            for xd in 1u64..5 {
                                let numerator = size * bn * xd + size * seconds * xn * bd;
                                let denominator = bd * xd;
                                let expected = numerator.div_ceil(denominator).max(1);
                                assert_eq!(
                                    store(
                                        size,
                                        seconds,
                                        Rate {
                                            numerator: bn,
                                            denominator: bd
                                        },
                                        Rate {
                                            numerator: xn,
                                            denominator: xd
                                        },
                                        1
                                    ),
                                    Ok(expected)
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    // Invalid metadata never reaches division; no wrapping or division-by-zero panic is possible.
    #[test]
    fn invalid_inputs() {
        for (size, time, min) in [(0, 1, 1), (1, 0, 1), (1, 1, 0)] {
            assert_eq!(store(size, time, HALF, HALF, min), Err(Error::Invalid));
            assert_eq!(usage(size, time, HALF, min), Err(Error::Invalid));
        }
        let bad = Rate {
            numerator: 1,
            denominator: 0,
        };
        assert_eq!(store(1, 1, bad, HALF, 1), Err(Error::Invalid));
        assert_eq!(store(1, 1, HALF, bad, 1), Err(Error::Invalid));
        assert_eq!(usage(1, 1, bad, 1), Err(Error::Invalid));
    }
}
