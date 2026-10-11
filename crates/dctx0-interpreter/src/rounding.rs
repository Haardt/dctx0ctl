//! Exact rounding shared by all execution targets; no native Number conversions.
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{Signed, Zero};

pub(crate) fn round(value: BigRational, scale: u32, mode: &str) -> Option<BigRational> {
    if !matches!(mode, "Down" | "Up" | "HalfUp" | "HalfEven") {
        return None;
    }
    let factor = BigInt::from(10).pow(scale);
    let scaled = value * BigRational::from_integer(factor.clone());
    let mut quotient = scaled.numer() / scaled.denom();
    let remainder = scaled.numer() % scaled.denom();
    let twice = remainder.abs() * 2;
    let increment = match mode {
        "Down" => false,
        "Up" => !remainder.is_zero(),
        "HalfUp" => twice >= *scaled.denom(),
        _ => twice > *scaled.denom() || twice == *scaled.denom() && !(&quotient % 2u32).is_zero(),
    };
    if increment {
        quotient += if scaled.is_negative() { -1 } else { 1 };
    }
    Some(BigRational::new(quotient, factor))
}
