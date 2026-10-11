//! Stable sorting with explicit comparison budgets and language-neutral keys.
use super::{V, number};
use num_rational::BigRational;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Number(BigRational),
    Text(Vec<u16>),
}

pub(super) fn sort_direction(
    values: Vec<(V, V)>,
    steps: &mut usize,
    budget: usize,
    descending: bool,
) -> Result<Vec<V>, &'static str> {
    let mut keyed = Vec::with_capacity(values.len());
    for (key, value) in values {
        let key = match key {
            V::String(text) => Key::Text(text.encode_utf16().collect()),
            V::Integer(text) => {
                Key::Number(number::parse(&text, true).ok_or("UNSUPPORTED_SORT_KEY")?)
            }
            V::Decimal(text) => {
                Key::Number(number::parse(&text, false).ok_or("UNSUPPORTED_SORT_KEY")?)
            }
            V::Float(text) => Key::Number(
                text.parse::<f64>()
                    .ok()
                    .filter(|value| value.is_finite())
                    .and_then(BigRational::from_float)
                    .ok_or("UNSUPPORTED_SORT_KEY")?,
            ),
            _ => return Err("UNSUPPORTED_SORT_KEY"),
        };
        if keyed.first().is_some_and(|(first, _)| {
            matches!(
                (first, &key),
                (Key::Text(_), Key::Number(_)) | (Key::Number(_), Key::Text(_))
            )
        }) {
            return Err("INCOMPATIBLE_SORT_KEYS");
        }
        keyed.push((key, value));
    }
    let mut width = 1usize;
    while width < keyed.len() {
        let mut merged = Vec::with_capacity(keyed.len());
        for start in (0..keyed.len()).step_by(width.saturating_mul(2)) {
            let middle = start.saturating_add(width).min(keyed.len());
            let end = middle.saturating_add(width).min(keyed.len());
            let (mut left, mut right) = (start, middle);
            while left < middle && right < end {
                *steps = steps.saturating_add(1);
                if *steps > budget {
                    return Err("EVALUATION_BUDGET");
                }
                // Equal keys retain the original order.
                if if descending {
                    keyed[left].0 >= keyed[right].0
                } else {
                    keyed[left].0 <= keyed[right].0
                } {
                    merged.push(keyed[left].clone());
                    left += 1;
                } else {
                    merged.push(keyed[right].clone());
                    right += 1;
                }
            }
            merged.extend_from_slice(&keyed[left..middle]);
            merged.extend_from_slice(&keyed[right..end]);
        }
        keyed = merged;
        width = width.saturating_mul(2);
    }
    Ok(keyed.into_iter().map(|(_, value)| value).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sort(values: Vec<(V, V)>, steps: &mut usize, budget: usize) -> Result<Vec<V>, &'static str> {
        sort_direction(values, steps, budget, false)
    }
    fn entry(key: V, id: &str) -> (V, V) {
        (key, V::String(id.into()))
    }
    #[test]
    fn unicode_and_equal_numeric_keys_have_stable_defined_order() {
        let mut steps = 0;
        assert_eq!(
            sort(
                vec![
                    entry(V::String("\u{e000}".into()), "bmp"),
                    entry(V::String("\u{10000}".into()), "supplementary")
                ],
                &mut steps,
                100
            )
            .unwrap(),
            vec![V::String("supplementary".into()), V::String("bmp".into())]
        );
        let values = vec![
            entry(V::Decimal("1.00".into()), "decimal"),
            entry(V::Float("1e0".into()), "float"),
            entry(V::Integer("1".into()), "integer"),
            entry(V::Integer("0".into()), "zero"),
            entry(V::Integer("2".into()), "two"),
        ];
        assert_eq!(
            sort(values, &mut steps, 100).unwrap(),
            ["zero", "decimal", "float", "integer", "two"]
                .map(|id| V::String(id.into()))
                .to_vec()
        );
        assert_eq!(sort(vec![], &mut steps, 100).unwrap(), vec![]);
        assert_eq!(
            sort_direction(
                vec![
                    entry(V::Integer("1".into()), "first"),
                    entry(V::Integer("2".into()), "high"),
                    entry(V::Float("1e0".into()), "second")
                ],
                &mut 0,
                100,
                true
            )
            .unwrap(),
            ["high", "first", "second"]
                .map(|id| V::String(id.into()))
                .to_vec()
        );
    }
    #[test]
    fn malformed_incompatible_and_over_budget_keys_never_return_partial_ordering() {
        for key in [
            V::Integer("bad".into()),
            V::Decimal("bad".into()),
            V::Float("NaN".into()),
            V::Boolean(true),
        ] {
            assert_eq!(
                sort(vec![entry(key, "invalid")], &mut 0, 100),
                Err("UNSUPPORTED_SORT_KEY")
            );
        }
        for values in [
            vec![
                entry(V::String("x".into()), "text"),
                entry(V::Integer("1".into()), "number"),
            ],
            vec![
                entry(V::Integer("1".into()), "number"),
                entry(V::String("x".into()), "text"),
            ],
        ] {
            assert_eq!(sort(values, &mut 0, 100), Err("INCOMPATIBLE_SORT_KEYS"));
        }
        assert_eq!(
            sort(
                vec![
                    entry(V::Integer("2".into()), "two"),
                    entry(V::Integer("1".into()), "one")
                ],
                &mut 0,
                0
            ),
            Err("EVALUATION_BUDGET")
        );
    }
}
