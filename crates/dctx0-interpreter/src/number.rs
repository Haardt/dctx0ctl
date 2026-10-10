//! Exact finite decimal arithmetic. Unsupported nonterminating division is explicit.
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, Zero};
use std::str::FromStr;

pub(crate) fn parse(text: &str, integer: bool) -> Option<BigRational> {
    let unsigned = text.strip_prefix('-').unwrap_or(text);
    if unsigned.is_empty() || text.len() > 65_536 {
        return None;
    }
    let mut parts = unsigned.split('.');
    let whole = parts.next()?;
    let fraction = parts.next();
    if parts.next().is_some()
        || whole.is_empty()
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || fraction
            .is_some_and(|f| integer || f.is_empty() || !f.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    let digits = format!("{whole}{}", fraction.unwrap_or(""));
    let numerator = BigInt::from_str(&digits).ok()?;
    let numerator = if text.starts_with('-') {
        -numerator
    } else {
        numerator
    };
    Some(BigRational::new(
        numerator,
        BigInt::from(10).pow(fraction.map_or(0, |s| s.len() as u32)),
    ))
}

pub(crate) fn decimal(value: &BigRational) -> Option<String> {
    let mut denominator = value.denom().clone();
    let mut twos = 0;
    let mut fives = 0;
    for (factor, count) in [(2u32, &mut twos), (5u32, &mut fives)] {
        while (&denominator % factor).is_zero() {
            denominator /= factor;
            *count += 1;
        }
    }
    if denominator != BigInt::one() {
        return None;
    }
    let scale = twos.max(fives);
    let coefficient =
        value.numer() * BigInt::from(2).pow(scale - twos) * BigInt::from(5).pow(scale - fives);
    let mut digits = coefficient.abs().to_string();
    if scale > 0 {
        let scale = scale as usize;
        if digits.len() <= scale {
            digits = format!("{}{}", "0".repeat(scale + 1 - digits.len()), digits);
        }
        digits.insert(digits.len() - scale, '.');
        // A reduced rational has no removable base-10 factor at this scale.
    }
    if coefficient.is_negative() {
        digits.insert(0, '-');
    }
    Some(digits)
}
