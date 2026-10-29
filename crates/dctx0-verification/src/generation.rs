//! Versioned, target-neutral bounded input selection. No randomness from the target.
use dctx0_core::{
    ExpressionKind as E, StatementKind as K, TypeRef, VerificationBudgets, VerificationIssue,
    VerificationValue as V, WorkspaceIr,
};
use dctx0_interpreter::{ValueContract, check_value};
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

mod strings;

pub const GENERATOR_VERSION: &str = "dctx0-values-6";
pub const REJECTION_GENERATOR_VERSION: &str = "dctx0-rejections-3";

// Adjacent IEEE-754 values, without the Rust 1.86 next_up/next_down APIs.
fn adjacent_float(value: f64, up: bool) -> f64 {
    if value.is_nan() || value == if up { f64::INFINITY } else { f64::NEG_INFINITY } {
        return value;
    }
    if value == 0.0 {
        return f64::from_bits(if up { 1 } else { (1 << 63) | 1 });
    }
    let bits = value.to_bits();
    f64::from_bits(if value.is_sign_positive() == up {
        bits + 1
    } else {
        bits - 1
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValueCorpus {
    pub values: Vec<V>,
    /// True only when the entire modeled type domain is finite and enumerated.
    pub exhaustive: bool,
    pub issues: Vec<VerificationIssue>,
}
impl ValueCorpus {
    fn empty(code: &str, message: &str) -> Self {
        Self {
            values: Vec::new(),
            exhaustive: false,
            issues: vec![VerificationIssue {
                code: code.into(),
                message: message.into(),
                span: None,
            }],
        }
    }
}

/// Canonical, centrally rejected values for native boundary checks. This is a
/// bounded counterexample corpus, not an exhaustive invalid-input domain.
pub fn generate_rejections(
    ir: &WorkspaceIr,
    ty: &TypeRef,
    budgets: &VerificationBudgets,
) -> ValueCorpus {
    let mut candidates = vec![
        V::Unit,
        V::Boolean(true),
        V::Integer("0".into()),
        V::Integer("-1".into()),
        V::String(String::new().into()),
        V::String("not-a-uuid".into()),
        V::String("\u{feff}".into()),
        V::String("\u{0085}".into()),
        V::Float("1.5".into()),
        V::Record(BTreeMap::new()),
        V::List(vec![]),
        V::Option(None),
        V::List(vec![V::Unit]),
        V::List(vec![V::Boolean(true)]),
        V::Option(Some(Box::new(V::Unit))),
        V::Option(Some(Box::new(V::Boolean(true)))),
    ];
    let generator = Generator {
        ir,
        budgets,
        rng: Random(budgets.seed),
        steps: 0,
        exhausted: false,
        active: BTreeMap::new(),
        recursive_frontiers: 0,
    };
    let mut current = ty;
    let mut issues = Vec::new();
    for _ in 0..=budgets.recursion_depth.min(ir.data.len()) {
        let Some(id) = &current.symbol_id else {
            break;
        };
        if let Some((min, max)) = generator.bounds(id) {
            if matches!(generator.base(current, 0), Some("Float" | "Duration")) {
                for outside in [
                    min.parse::<f64>()
                        .ok()
                        .map(|value| adjacent_float(value, false)),
                    max.parse::<f64>()
                        .ok()
                        .map(|value| adjacent_float(value, true)),
                ]
                .into_iter()
                .flatten()
                .filter(|value| value.is_finite())
                {
                    candidates.push(V::Float(outside.to_string()));
                }
            }
            let bounds = match generator.base(current, 0) {
                Some("Int") => Some((V::Integer(min), V::Integer(max))),
                Some("Decimal") => Some((V::Decimal(min), V::Decimal(max))),
                _ => None,
            };
            if let Some((Some(min), Some(max))) = bounds.map(|(min, max)| {
                (
                    dctx0_interpreter::numeric_offset(&min, -1),
                    dctx0_interpreter::numeric_offset(&max, 1),
                )
            }) {
                candidates.extend([min, max]);
            } else if !matches!(generator.base(current, 0), Some("Float" | "Duration")) {
                issues.push(VerificationIssue {
                    code: "REJECTION_RANGE_UNSUPPORTED".into(),
                    message: format!("Cannot construct exact outside-range values for {id}"),
                    span: ir.symbols.get(id).map(|s| s.span.clone()),
                });
            }
        }
        let Some(data) = ir.data.get(id) else {
            break;
        };
        for node in ir
            .nodes
            .iter()
            .filter(|node| node.owner == *id && matches!(node.kind, K::Format | K::Length))
        {
            if node.kind == K::Format {
                let invalid: &[&str] = match node.terms.first().map(|token| token.text.as_str()) {
                    Some("Email") => &["a@@b.co", "a@b", "a@b.co\u{feff}"],
                    Some("UUID") => &["00000000-0000-0000-0000-000000000001"],
                    Some("Uri") => &["/relative", "https://bad host.test"],
                    Some("Date") => &["1900-02-29", "2026-02-30", "2026-1-01"],
                    Some("DateTime") => &[
                        "2026-02-30T00:00:00Z",
                        "2026-01-01T24:00:00Z",
                        "October 11, 2026",
                    ],
                    _ => &[],
                };
                candidates.extend(invalid.iter().map(|text| V::String((*text).into())));
            } else {
                let text = node
                    .terms
                    .iter()
                    .map(|token| token.text.as_str())
                    .collect::<String>();
                if let Some((minimum, maximum)) = text.split_once("..") {
                    if let (Ok(minimum), Ok(maximum)) =
                        (minimum.parse::<usize>(), maximum.parse::<usize>())
                    {
                        for size in [minimum.checked_sub(1), maximum.checked_add(1)]
                            .into_iter()
                            .flatten()
                        {
                            if size > budgets.value_bytes.saturating_sub(32) {
                                continue;
                            }
                            match generator.base(current, 0) {
                                Some("String") => candidates.push(V::String("x".repeat(size).into())),
                                Some("List") if size <= budgets.collection_size => {
                                    if let Some(V::List(sample)) = data.alias.as_ref().and_then(|alias| generate_values(ir, alias, budgets).values.into_iter().find(|value| matches!(value, V::List(items) if !items.is_empty()))) {
                                        candidates.push(V::List(vec![sample[0].clone(); size]));
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
        if !data.variants.is_empty() {
            candidates.push(V::Variant {
                declaration: id.clone(),
                name: "__invalid_alternative__".into(),
                payload: None,
            });
        }
        let Some(alias) = &data.alias else {
            break;
        };
        current = alias;
    }
    for value in generate_values(ir, ty, budgets).values.into_iter().take(1) {
        if let V::Record(mut fields) = value {
            let mut extra = fields.clone();
            extra.insert("__unexpected_field__".into(), V::Unit);
            candidates.push(V::Record(extra));
            if let Some(name) = fields.keys().next().cloned() {
                fields.remove(&name);
                candidates.push(V::Record(fields));
            }
        }
    }
    let mut seen = Vec::new();
    candidates.retain(|v| {
        matches!(
            check_value(ir, ty, v, budgets),
            ValueContract::Rejected { .. }
        ) && if seen.contains(v) {
            false
        } else {
            seen.push(v.clone());
            true
        }
    });
    if candidates.is_empty() {
        return ValueCorpus::empty(
            "REJECTION_CORPUS_UNAVAILABLE",
            "No centrally rejected canonical values fit this contract and budget",
        );
    }
    ValueCorpus {
        values: candidates,
        exhaustive: false,
        issues,
    }
}

pub fn missing_rejections() -> ValueCorpus {
    ValueCorpus::empty(
        "LEGACY_REJECTION_CORPUS",
        "This saved plan predates required native rejection cases; reproject its pinned requirements",
    )
}
pub fn generate_values(
    ir: &WorkspaceIr,
    ty: &TypeRef,
    budgets: &VerificationBudgets,
) -> ValueCorpus {
    if budgets.generated_cases == 0
        || budgets.generated_cases > budgets.evaluation_steps
        || budgets.collection_size > budgets.evaluation_steps
    {
        return ValueCorpus::empty(
            "GENERATION_BUDGET",
            "Generation limits must be nonzero and fit the work budget",
        );
    }
    Generator {
        ir,
        budgets,
        rng: Random(budgets.seed),
        steps: 0,
        exhausted: false,
        active: BTreeMap::new(),
        recursive_frontiers: 0,
    }
    .domain(ty, 0)
}
struct Random(u64);
impl Random {
    fn next(&mut self) -> u64 {
        // SplitMix64; wraparound is part of the versioned generator contract.
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
        value ^ (value >> 31)
    }
    fn index(&mut self, length: usize) -> usize {
        (self.next() as u128 % length as u128) as usize
    }
}
struct Generator<'a> {
    ir: &'a WorkspaceIr,
    budgets: &'a VerificationBudgets,
    rng: Random,
    steps: usize,
    exhausted: bool,
    active: BTreeMap<String, usize>,
    recursive_frontiers: usize,
}
impl Generator<'_> {
    fn domain(&mut self, ty: &TypeRef, depth: usize) -> ValueCorpus {
        // Recursive alternatives describe an infinite domain. Select a finite
        // productive frontier rather than expanding both JSON branches until
        // the work budget is exhausted. Two visits retain nonempty recursive
        // collections and their terminal alternatives; never claim exhaustive.
        if let Some(id) = &ty.symbol_id {
            if self.active.get(id).copied().unwrap_or(0) >= 2
                && self
                    .ir
                    .data
                    .get(id)
                    .is_some_and(|data| data.alias.is_none())
            {
                self.recursive_frontiers += 1;
                return ValueCorpus {
                    values: Vec::new(),
                    exhaustive: false,
                    issues: Vec::new(),
                };
            }
            *self.active.entry(id.clone()).or_default() += 1;
        }
        let frontier_before = self.recursive_frontiers;
        let corpus = self.expand(ty, depth, frontier_before);
        if let Some(id) = &ty.symbol_id {
            *self.active.get_mut(id).expect("active type was entered") -= 1;
        }
        corpus
    }
    fn expand(&mut self, ty: &TypeRef, depth: usize, frontier_before: usize) -> ValueCorpus {
        self.steps += 1;
        if depth > self.budgets.recursion_depth || self.steps > self.budgets.evaluation_steps {
            return ValueCorpus::empty(
                "GENERATION_BUDGET",
                "Type generation exceeded its recursion or step budget",
            );
        }
        let witnesses = self.string_equality_witnesses(ty, depth);
        let has_witnesses = !witnesses.is_empty();
        let mut corpus = if has_witnesses {
            ValueCorpus {
                values: witnesses,
                exhaustive: false,
                issues: Vec::new(),
            }
        } else if let Some(id) = &ty.symbol_id {
            let Some(data) = self.ir.data.get(id) else {
                return ValueCorpus::empty("MISSING_TYPE", "Input declaration is unavailable");
            };
            if let Some(alias) = &data.alias {
                if let Some(bounds) = self.bounds(id) {
                    match self.base(alias, depth + 1) {
                        Some("Int") => self.integer_range(&bounds.0, &bounds.1),
                        Some("Decimal") => {
                            let mut corpus = self.domain(alias, depth + 1);
                            corpus
                                .values
                                .splice(0..0, [V::Decimal(bounds.0), V::Decimal(bounds.1)]);
                            corpus.exhaustive = false;
                            corpus
                        }
                        Some("Float" | "Duration") => {
                            let mut corpus = self.domain(alias, depth + 1);
                            corpus
                                .values
                                .splice(0..0, [V::Float(bounds.0), V::Float(bounds.1)]);
                            corpus.exhaustive = false;
                            corpus
                        }
                        _ => self.domain(alias, depth + 1),
                    }
                } else {
                    self.domain(alias, depth + 1)
                }
            } else if !data.variants.is_empty() {
                let mut corpus = ValueCorpus {
                    values: Vec::new(),
                    exhaustive: true,
                    issues: Vec::new(),
                };
                for variant in &data.variants {
                    if let Some(payload) = &variant.payload {
                        let child = self.domain(payload, depth + 1);
                        corpus.exhaustive &= child.exhaustive;
                        corpus.issues.extend(child.issues);
                        corpus
                            .values
                            .extend(child.values.into_iter().map(|value| V::Variant {
                                declaration: id.clone(),
                                name: variant.name.clone(),
                                payload: Some(Box::new(value)),
                            }));
                    } else {
                        corpus.values.push(V::Variant {
                            declaration: id.clone(),
                            name: variant.name.clone(),
                            payload: None,
                        });
                    }
                }
                corpus
            } else {
                let fields = data.fields.clone();
                let mut domains = Vec::new();
                let mut issues = Vec::new();
                let mut exhaustive = true;
                for field in &fields {
                    let child = self.domain(&field.ty, depth + 1);
                    exhaustive &= child.exhaustive;
                    issues.extend(child.issues);
                    domains.push(child.values);
                }
                let (tuples, complete) = self.product(&domains);
                ValueCorpus {
                    values: tuples
                        .into_iter()
                        .map(|tuple| {
                            V::Record(
                                fields
                                    .iter()
                                    .zip(tuple)
                                    .map(|(f, value)| (f.name.clone(), value))
                                    .collect(),
                            )
                        })
                        .collect(),
                    exhaustive: exhaustive && complete,
                    issues,
                }
            }
        } else {
            self.builtin(ty, depth)
        };
        if let Some(id) = &ty.symbol_id
            && !has_witnesses
        {
            for node in self
                .ir
                .nodes
                .iter()
                .filter(|node| node.owner == *id && matches!(node.kind, K::Format | K::Length))
            {
                if node.kind == K::Format {
                    let samples: &[&str] = match node.terms.first().map(|token| token.text.as_str())
                    {
                        Some("Email") => &["a@b.co", "sample@example.test"],
                        Some("UUID") => &[
                            "00000000-0000-4000-8000-000000000001",
                            "ffffffff-ffff-5fff-bfff-ffffffffffff",
                        ],
                        Some("Uri") => &[
                            "https://example.test/tasks",
                            "mailto:sample@example.test",
                            "urn:task:1",
                        ],
                        Some("Date") => &["1970-01-01", "2000-02-29", "2026-10-11"],
                        Some("DateTime") => &["1970-01-01T00:00:00Z", "2000-02-29T12:00:00+01:00"],
                        _ => &[],
                    };
                    corpus.values.splice(
                        0..0,
                        samples.iter().map(|sample| V::String((*sample).into())),
                    );
                    corpus.exhaustive = false;
                } else if self.base(ty, depth) == Some("String") {
                    let text = node
                        .terms
                        .iter()
                        .map(|token| token.text.as_str())
                        .collect::<String>();
                    if let Some((minimum, maximum)) = text.split_once("..") {
                        for bound in [minimum, maximum] {
                            if let Ok(size) = bound.parse::<usize>() {
                                if size <= self.budgets.value_bytes.saturating_sub(32) {
                                    corpus.values.insert(0, V::String("x".repeat(size).into()));
                                } else {
                                    corpus.issues.push(VerificationIssue { code: "GENERATION_BUDGET".into(), message: "Required length boundary exceeds the canonical value budget".into(), span: Some(node.span.clone()) });
                                }
                            }
                        }
                    }
                    corpus.exhaustive = false;
                }
            }
        }
        let mut seen = BTreeSet::new();
        let mut valid = Vec::new();
        for value in corpus.values {
            match check_value(self.ir, ty, &value, self.budgets) {
                ValueContract::Accepted => {
                    let key = serde_json::to_string(&value)
                        .expect("wire values contain no fallible JSON fields");
                    if seen.insert(key) {
                        valid.push(value);
                    }
                }
                ValueContract::Rejected { .. } => {}
                ValueContract::Unknown { issue } => corpus.issues.push(issue),
            }
        }
        corpus.values = valid;
        if corpus.values.len() > self.budgets.generated_cases {
            corpus.exhaustive = false;
            let last = corpus.values.pop().unwrap();
            corpus.values.truncate(self.budgets.generated_cases);
            if self.budgets.generated_cases > 1 {
                *corpus.values.last_mut().unwrap() = last;
            }
        }
        if self.exhausted {
            corpus.issues.push(VerificationIssue {
                code: "GENERATION_BUDGET".into(),
                message: "Composite input selection exceeded its work budget".into(),
                span: None,
            });
        }
        if corpus.values.is_empty() && (depth == 0 || frontier_before == self.recursive_frontiers) {
            corpus.issues.push(VerificationIssue {
                code: "EMPTY_CORPUS".into(),
                message: "No valid input was generated; this is not verification evidence".into(),
                span: ty
                    .symbol_id
                    .as_ref()
                    .and_then(|id| self.ir.symbols.get(id))
                    .map(|s| s.span.clone()),
            });
        }
        corpus
            .issues
            .sort_by(|a, b| (&a.code, &a.message).cmp(&(&b.code, &b.message)));
        corpus.issues.dedup();
        if !corpus.issues.is_empty() {
            corpus.exhaustive = false;
        }
        corpus
    }
    /// Literal equalities and prefixes seed candidates; the full contract decides validity.
    fn string_equality_witnesses(&mut self, ty: &TypeRef, depth: usize) -> Vec<V> {
        if self.base(ty, depth) != Some("String") {
            return Vec::new();
        }
        let mut values = Vec::new();
        let mut current = ty;
        for _ in depth..=self.budgets.recursion_depth {
            let Some(id) = &current.symbol_id else { break };
            for expression in self
                .ir
                .nodes
                .iter()
                .filter(|node| node.owner == *id && node.kind == K::Where)
                .flat_map(|node| &node.expressions)
            {
                self.steps += 1;
                if self.steps > self.budgets.evaluation_steps {
                    self.exhausted = true;
                    return values;
                }
                if let E::Binary {
                    operator,
                    left,
                    right,
                } = &expression.value
                    && operator == "=="
                {
                    for (variable, literal) in [(left, right), (right, left)] {
                        if matches!(&variable.value, E::Variable { name } if name == "value")
                            && let E::Literal { text, literal_kind } = &literal.value
                            && literal_kind == "String"
                        {
                            let value = V::String(text.clone().into());
                            if !values.contains(&value) {
                                values.push(value);
                            }
                        }
                    }
                }
            }
            let Some(alias) = self.ir.data.get(id).and_then(|data| data.alias.as_ref()) else {
                break;
            };
            current = alias;
        }
        values.extend(strings::prefix_witnesses(self.ir, ty, self.budgets));
        values
    }
    fn base<'a>(&'a self, ty: &'a TypeRef, depth: usize) -> Option<&'a str> {
        if depth > self.budgets.recursion_depth {
            return None;
        }
        if let Some(id) = &ty.symbol_id {
            self.base(self.ir.data.get(id)?.alias.as_ref()?, depth + 1)
        } else {
            Some(&ty.name)
        }
    }
    fn bounds(&self, id: &str) -> Option<(String, String)> {
        let node = self
            .ir
            .nodes
            .iter()
            .find(|n| n.owner == id && n.kind == K::Range)?;
        let text = node
            .terms
            .iter()
            .map(|t| t.text.as_str())
            .collect::<String>();
        let (min, max) = text.split_once("..")?;
        Some((min.into(), max.into()))
    }
    fn integer_range(&mut self, min: &str, max: &str) -> ValueCorpus {
        let (Ok(min), Ok(max)) = (BigInt::from_str(min), BigInt::from_str(max)) else {
            return ValueCorpus::empty(
                "UNSUPPORTED_RANGE",
                "Integer range needs integral finite bounds",
            );
        };
        if min > max {
            return ValueCorpus::empty("INVALID_RANGE", "Integer range is descending");
        }
        let width: BigInt = &max - &min + 1;
        let exhaustive = width
            .to_usize()
            .is_some_and(|n| n <= self.budgets.generated_cases);
        let values = if exhaustive {
            (0..width.to_usize().unwrap())
                .map(|n| V::Integer((&min + n).to_string()))
                .collect()
        } else {
            let mut values = vec![
                V::Integer(min.to_string()),
                V::Integer(max.to_string()),
                V::Integer((&min + 1u32).to_string()),
                V::Integer((&max - 1u32).to_string()),
            ];
            for _ in 0..self.budgets.generated_cases {
                let sample: BigInt =
                    (BigInt::from(self.rng.next()) << 64usize) + BigInt::from(self.rng.next());
                values.push(V::Integer((&min + sample % &width).to_string()));
            }
            values
        };
        ValueCorpus {
            values,
            exhaustive,
            issues: Vec::new(),
        }
    }
    fn product(&mut self, domains: &[Vec<V>]) -> (Vec<Vec<V>>, bool) {
        if domains.iter().any(Vec::is_empty) {
            return (Vec::new(), true);
        }
        let total = domains
            .iter()
            .try_fold(1usize, |total, domain| total.checked_mul(domain.len()));
        let complete = total.is_some_and(|n| n <= self.budgets.generated_cases);
        let count = total
            .unwrap_or(self.budgets.generated_cases)
            .min(self.budgets.generated_cases);
        let cost = count.saturating_mul(domains.len());
        if cost > self.budgets.evaluation_steps.saturating_sub(self.steps) {
            self.exhausted = true;
            return (Vec::new(), false);
        }
        let mut tuples = Vec::new();
        let mut seen = BTreeSet::new();
        let mut position = 0;
        while tuples.len() < count {
            if domains.len() > self.budgets.evaluation_steps.saturating_sub(self.steps) {
                self.exhausted = true;
                break;
            }
            self.steps += domains.len();
            let mut remainder = position;
            let indices = domains
                .iter()
                .map(|domain| {
                    if complete {
                        let index = remainder % domain.len();
                        remainder /= domain.len();
                        index
                    } else if position == 0 {
                        0
                    } else if position == 1 {
                        domain.len() - 1
                    } else {
                        self.rng.index(domain.len())
                    }
                })
                .collect::<Vec<_>>();
            position += 1;
            if seen.insert(indices.clone()) {
                tuples.push(
                    domains
                        .iter()
                        .zip(indices)
                        .map(|(domain, index)| domain[index].clone())
                        .collect(),
                );
            }
        }
        (tuples, complete)
    }
    fn builtin(&mut self, ty: &TypeRef, depth: usize) -> ValueCorpus {
        let mut exhaustive = false;
        let values = match ty.name.as_str() {
            "Unit" => {
                exhaustive = true;
                vec![V::Unit]
            }
            "Boolean" => {
                exhaustive = true;
                vec![V::Boolean(false), V::Boolean(true)]
            }
            "String" => [
                "",
                "a",
                "Cinema",
                " whitespace ",
                "😀",
                "é",
                "\u{feff}",
                "\u{0085}",
            ]
            .into_iter()
            .map(|s| V::String(s.into()))
            .collect(),
            "Int" => {
                let mut values = [
                    "0",
                    "1",
                    "-1",
                    "2",
                    "-2",
                    "9007199254740991",
                    "9007199254740992",
                    "9007199254740993",
                    "-9007199254740993",
                    "1000000000000000000000000000000",
                ]
                .into_iter()
                .map(|s| V::Integer(s.into()))
                .collect::<Vec<_>>();
                for _ in 0..self.budgets.generated_cases {
                    values.push(V::Integer((self.rng.next() as i64).to_string()));
                }
                values
            }
            "Decimal" => [
                "0",
                "1",
                "-1",
                "0.1",
                "0.5",
                "1.5",
                "-0.1",
                "9007199254740993.1",
            ]
            .into_iter()
            .map(|s| V::Decimal(s.into()))
            .collect(),
            "Float" | "Duration" => [
                "0",
                "1",
                "-1",
                "4.5",
                "0.1",
                "1.7976931348623157e308",
                "5e-324",
            ]
            .into_iter()
            .map(|s| V::Float(s.into()))
            .collect(),
            "UUID" => (1..=self.budgets.generated_cases.min(16))
                .map(|n| V::String(format!("00000000-0000-4000-8000-{n:012x}").into()))
                .collect(),
            "Timestamp" => [
                "2026-10-10T00:00:00Z",
                "1970-01-01T00:00:00Z",
                "2000-02-29T12:00:00+01:00",
                "2026-10-10T00:00:00.123456789Z",
            ]
            .into_iter()
            .map(|s| V::String(s.into()))
            .collect(),
            "None" => {
                exhaustive = true;
                vec![V::Option(None)]
            }
            "Bytes" => [vec![], vec![0], vec![255], vec![0, 1, 128, 255]]
                .into_iter()
                .filter(|bytes| bytes.len() <= self.budgets.collection_size)
                .map(|bytes| {
                    V::List(
                        bytes
                            .into_iter()
                            .map(|byte| V::Integer(byte.to_string()))
                            .collect(),
                    )
                })
                .collect(),
            "Dctx0Result" => {
                let [success, failure] = ty.arguments.as_slice() else {
                    return ValueCorpus::empty(
                        "TYPE_ARGUMENTS",
                        "Dctx0Result needs two element types",
                    );
                };
                let success = self.domain(success, depth + 1);
                let failure = self.domain(failure, depth + 1);
                return ValueCorpus {
                    exhaustive: success.exhaustive && failure.exhaustive,
                    issues: success.issues.into_iter().chain(failure.issues).collect(),
                    values: success
                        .values
                        .into_iter()
                        .map(|value| ("Success", value))
                        .chain(failure.values.into_iter().map(|value| ("Failure", value)))
                        .map(|(name, value)| V::Variant {
                            declaration: "builtin:Dctx0Result".into(),
                            name: name.into(),
                            payload: Some(Box::new(value)),
                        })
                        .collect(),
                };
            }
            "List" | "Option" | "Dctx0Option" | "Some" => {
                let Some(element) = ty.arguments.first().filter(|_| ty.arguments.len() == 1) else {
                    return ValueCorpus::empty(
                        "TYPE_ARGUMENTS",
                        "Collection needs exactly one element type",
                    );
                };
                let child = self.domain(element, depth + 1);
                if ty.name != "List" {
                    let mut values = if ty.name == "Some" {
                        vec![]
                    } else {
                        vec![V::Option(None)]
                    };
                    values.extend(
                        child
                            .values
                            .into_iter()
                            .map(|v| V::Option(Some(Box::new(v)))),
                    );
                    return ValueCorpus {
                        values,
                        exhaustive: child.exhaustive,
                        issues: child.issues,
                    };
                }
                let mut values = vec![V::List(Vec::new())];
                let mut pools = Vec::new();
                for length in 1..=self.budgets.collection_size {
                    if length.saturating_mul(child.values.len())
                        > self.budgets.evaluation_steps.saturating_sub(self.steps)
                    {
                        self.exhausted = true;
                        break;
                    }
                    let domains = vec![child.values.clone(); length];
                    let (tuples, _) = self.product(&domains);
                    if let Some(first) = tuples.first() {
                        values.push(V::List(first.clone()));
                    }
                    if let Some(last) = tuples.last().filter(|last| tuples.first() != Some(last)) {
                        values.push(V::List(last.clone()));
                    }
                    pools.extend(tuples.into_iter().map(V::List));
                    if self.exhausted {
                        break;
                    }
                }
                values.extend(pools);
                // Lists remain an unbounded modeled type even if <=4 is enumerated.
                return ValueCorpus {
                    values,
                    exhaustive: false,
                    issues: child.issues,
                };
            }
            _ => {
                return ValueCorpus::empty(
                    "UNSUPPORTED_TYPE",
                    &format!("Cannot generate {}", ty.name),
                );
            }
        };
        ValueCorpus {
            values,
            exhaustive,
            issues: Vec::new(),
        }
    }
}

/// Correlated parameter tuples; names retain the validated declaration order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputCorpus {
    pub bindings: Vec<BTreeMap<String, V>>,
    pub exhaustive: bool,
    pub issues: Vec<VerificationIssue>,
}
pub fn generate_inputs(
    ir: &WorkspaceIr,
    parameters: &[dctx0_core::FieldIr],
    budgets: &VerificationBudgets,
) -> InputCorpus {
    let mut names = BTreeSet::new();
    if parameters
        .iter()
        .any(|p| p.name.is_empty() || !names.insert(&p.name))
    {
        return InputCorpus {
            bindings: Vec::new(),
            exhaustive: false,
            issues: ValueCorpus::empty(
                "INVALID_PARAMETERS",
                "Parameter names must be nonempty and unique",
            )
            .issues,
        };
    }
    if budgets.generated_cases == 0
        || budgets.generated_cases > budgets.evaluation_steps
        || budgets.collection_size > budgets.evaluation_steps
    {
        return InputCorpus {
            bindings: Vec::new(),
            exhaustive: false,
            issues: ValueCorpus::empty(
                "GENERATION_BUDGET",
                "Input selection limits exceed the work budget",
            )
            .issues,
        };
    }
    let mut generator = Generator {
        ir,
        budgets,
        rng: Random(budgets.seed),
        steps: 0,
        exhausted: false,
        active: BTreeMap::new(),
        recursive_frontiers: 0,
    };
    let mut domains = Vec::new();
    let mut issues = Vec::new();
    let mut exhaustive = true;
    for parameter in parameters {
        let corpus = generator.domain(&parameter.ty, 0);
        exhaustive &= corpus.exhaustive;
        issues.extend(corpus.issues);
        domains.push(corpus.values);
    }
    let (tuples, complete) = generator.product(&domains);
    if generator.exhausted {
        issues.extend(
            ValueCorpus::empty(
                "GENERATION_BUDGET",
                "Parameter combinations exceeded the work budget",
            )
            .issues,
        );
    }
    let mut seen = BTreeSet::new();
    let bindings = tuples
        .into_iter()
        .map(|tuple| {
            parameters
                .iter()
                .zip(tuple)
                .map(|(p, v)| (p.name.clone(), v))
                .collect::<BTreeMap<_, _>>()
        })
        .filter(|tuple| {
            seen.insert(serde_json::to_string(tuple).expect("wire bindings are JSON-compatible"))
        })
        .collect::<Vec<_>>();
    if bindings.is_empty() {
        issues.extend(
            ValueCorpus::empty(
                "EMPTY_CORPUS",
                "No complete parameter assignment was generated",
            )
            .issues,
        );
    }
    issues.sort_by(|a, b| (&a.code, &a.message).cmp(&(&b.code, &b.message)));
    issues.dedup();
    InputCorpus {
        bindings,
        exhaustive: exhaustive && complete && issues.is_empty(),
        issues,
    }
}

#[cfg(test)]
mod adjacent_tests {
    use super::adjacent_float;

    #[test]
    fn ieee_neighbors_preserve_sign_zero_subnormals_and_infinite_boundaries() {
        for (bits, up, down) in [
            (0, 1, 0x8000_0000_0000_0001),
            (0x8000_0000_0000_0000, 1, 0x8000_0000_0000_0001),
            (1, 2, 0),
            (
                0x8000_0000_0000_0001,
                0x8000_0000_0000_0000,
                0x8000_0000_0000_0002,
            ),
            (
                0x3ff0_0000_0000_0000,
                0x3ff0_0000_0000_0001,
                0x3fef_ffff_ffff_ffff,
            ),
            (
                0xbff0_0000_0000_0000,
                0xbfef_ffff_ffff_ffff,
                0xbff0_0000_0000_0001,
            ),
            (
                0x7fef_ffff_ffff_ffff,
                0x7ff0_0000_0000_0000,
                0x7fef_ffff_ffff_fffe,
            ),
            (
                0xffef_ffff_ffff_ffff,
                0xffef_ffff_ffff_fffe,
                0xfff0_0000_0000_0000,
            ),
            (
                0x7ff0_0000_0000_0000,
                0x7ff0_0000_0000_0000,
                0x7fef_ffff_ffff_ffff,
            ),
            (
                0xfff0_0000_0000_0000,
                0xffef_ffff_ffff_ffff,
                0xfff0_0000_0000_0000,
            ),
        ] {
            let value = f64::from_bits(bits);
            assert_eq!(adjacent_float(value, true).to_bits(), up);
            assert_eq!(adjacent_float(value, false).to_bits(), down);
        }
        assert!(adjacent_float(f64::NAN, true).is_nan());
        assert!(adjacent_float(f64::NAN, false).is_nan());
    }
}
