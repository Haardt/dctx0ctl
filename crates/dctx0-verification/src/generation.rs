//! Versioned, target-neutral bounded input selection. No randomness from the target.
use dctx0_core::{
    StatementKind as K, TypeRef, VerificationBudgets, VerificationIssue, VerificationValue as V,
    WorkspaceIr,
};
use dctx0_interpreter::{ValueContract, check_value};
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

pub const GENERATOR_VERSION: &str = "dctx0-values-1";
pub const REJECTION_GENERATOR_VERSION: &str = "dctx0-rejections-1";
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
        V::String(String::new()),
        V::String("not-a-uuid".into()),
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
    };
    let mut current = ty;
    let mut issues = Vec::new();
    for _ in 0..=budgets.recursion_depth.min(ir.data.len()) {
        let Some(id) = &current.symbol_id else {
            break;
        };
        if let Some((min, max)) = generator.bounds(id) {
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
            } else {
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
}
impl Generator<'_> {
    fn domain(&mut self, ty: &TypeRef, depth: usize) -> ValueCorpus {
        self.steps += 1;
        if depth > self.budgets.recursion_depth || self.steps > self.budgets.evaluation_steps {
            return ValueCorpus::empty(
                "GENERATION_BUDGET",
                "Type generation exceeded its recursion or step budget",
            );
        }
        let mut corpus = if let Some(id) = &ty.symbol_id {
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
        if corpus.values.len() > self.budgets.generated_cases {
            corpus.exhaustive = false;
            let last = corpus.values.pop().unwrap();
            corpus.values.truncate(self.budgets.generated_cases);
            if self.budgets.generated_cases > 1 {
                *corpus.values.last_mut().unwrap() = last;
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
        if self.exhausted {
            corpus.issues.push(VerificationIssue {
                code: "GENERATION_BUDGET".into(),
                message: "Composite input selection exceeded its work budget".into(),
                span: None,
            });
        }
        if corpus.values.is_empty() {
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
        self.steps += cost;
        let tuples = (0..count)
            .map(|position| {
                let mut remainder = position;
                domains
                    .iter()
                    .map(|domain| {
                        let index = if complete {
                            let index = remainder % domain.len();
                            remainder /= domain.len();
                            index
                        } else if position == 0 {
                            0
                        } else if position == 1 {
                            domain.len() - 1
                        } else {
                            self.rng.index(domain.len())
                        };
                        domain[index].clone()
                    })
                    .collect()
            })
            .collect();
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
            "String" => ["", "a", "Cinema", " whitespace ", "😀", "é"]
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
            "Float" => [
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
                .map(|n| V::String(format!("00000000-0000-4000-8000-{n:012x}")))
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
            "List" | "Option" => {
                let Some(element) = ty.arguments.first().filter(|_| ty.arguments.len() == 1) else {
                    return ValueCorpus::empty(
                        "TYPE_ARGUMENTS",
                        "Collection needs exactly one element type",
                    );
                };
                let child = self.domain(element, depth + 1);
                if ty.name == "Option" {
                    let mut values = vec![V::Option(None)];
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
