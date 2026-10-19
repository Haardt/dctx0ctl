//! Pure, budgeted evaluation of validated expressions. No filesystem or target I/O.
mod contracts;
mod modeled;
mod observed;
mod refinements;
pub use observed::ObservationContext;
mod number;
mod rounding;
mod sorting;
pub use contracts::{ValueContract, check_value};
use dctx0_core::{
    Expression, ExpressionKind as E, Invocation, InvocationObservation, Observation,
    VerificationBudgets, VerificationIssue, VerificationValue as V, WorkspaceIr,
};
pub use modeled::modeled_value;
use num_rational::BigRational;
use num_traits::{ToPrimitive, Zero};
use std::collections::BTreeMap;

pub const INTERPRETER_VERSION: &str = "dctx0-interpreter-5";

/// Exact boundary offsets shared by generators; never native floating arithmetic.
pub fn numeric_offset(value: &V, offset: i8) -> Option<V> {
    let (text, integer) = match value {
        V::Integer(text) => (text, true),
        V::Decimal(text) => (text, false),
        _ => return None,
    };
    let result = number::parse(text, integer)? + BigRational::from_integer(offset.into());
    if integer {
        Some(V::Integer(result.to_integer().to_string()))
    } else {
        number::decimal(&result).map(V::Decimal)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evaluation {
    Complete(V),
    /// Resume by appending exactly this invocation's observation to the tape.
    Invoke(Invocation),
    Unknown(VerificationIssue),
    ApplicationFailure(VerificationIssue),
}

/// Re-evaluating from a deterministic observation tape is a pure continuation.
/// Repeated equal invocations occupy separate positions; they are never memoized.
pub fn evaluate(
    ir: &WorkspaceIr,
    expression: &Expression,
    variables: &BTreeMap<String, V>,
    observations: &[InvocationObservation],
    budgets: &VerificationBudgets,
) -> Evaluation {
    evaluate_observed(
        ir,
        expression,
        variables,
        observations,
        &ObservationContext::default(),
        budgets,
    )
}

pub fn evaluate_observed(
    ir: &WorkspaceIr,
    expression: &Expression,
    variables: &BTreeMap<String, V>,
    observations: &[InvocationObservation],
    context: &ObservationContext,
    budgets: &VerificationBudgets,
) -> Evaluation {
    let mut evaluator = Evaluator {
        ir,
        observations,
        budgets,
        cursor: 0,
        steps: 0,
        context,
    };
    // Bound external wire values before recursive cloning or serialization.
    for value in variables.values() {
        if let Err(error) = evaluator.validate(expression, value, 0) {
            return error;
        }
        if serde_size(value) > budgets.value_bytes {
            return evaluator.issue(
                expression,
                "VALUE_SIZE",
                "Input exceeds canonical value byte budget",
            );
        }
    }
    match evaluator.eval(expression, variables, 0) {
        Ok(value) if evaluator.cursor == observations.len() => Evaluation::Complete(value),
        Ok(_) => evaluator.issue(
            expression,
            "OBSERVATION_MISMATCH",
            "Unused observations in continuation",
        ),
        Err(outcome) => outcome,
    }
}

type EvalResult = Result<V, Evaluation>;
struct Evaluator<'a> {
    ir: &'a WorkspaceIr,
    observations: &'a [InvocationObservation],
    budgets: &'a VerificationBudgets,
    cursor: usize,
    steps: usize,
    context: &'a ObservationContext,
}
impl Evaluator<'_> {
    fn issue(&self, e: &Expression, code: &str, message: &str) -> Evaluation {
        Evaluation::Unknown(VerificationIssue {
            code: code.into(),
            message: message.into(),
            span: Some(e.span.clone()),
        })
    }
    fn bad(&self, e: &Expression, message: &str) -> Evaluation {
        self.issue(e, "INVALID_VALUE", message)
    }
    fn boolean(&self, e: &Expression, value: V) -> Result<bool, Evaluation> {
        if let V::Boolean(value) = value {
            Ok(value)
        } else {
            Err(self.bad(e, "Expected Boolean"))
        }
    }
    fn numeric(&self, e: &Expression, value: &V) -> Result<BigRational, Evaluation> {
        let parsed = match value {
            V::Integer(text) => number::parse(text, true),
            V::Decimal(text) => number::parse(text, false),
            _ => None,
        };
        parsed.ok_or_else(|| self.bad(e, "Expected exact finite number"))
    }
    fn numeric_value(&self, e: &Expression, value: BigRational, integer: bool) -> EvalResult {
        if integer {
            Ok(V::Integer(value.to_integer().to_string()))
        } else {
            number::decimal(&value).map(V::Decimal).ok_or_else(|| {
                self.issue(
                    e,
                    "NONTERMINATING_DECIMAL",
                    "Use an explicit rounding contract for nonterminating division",
                )
            })
        }
    }
    fn comparison_number(&self, e: &Expression, value: &V) -> Result<BigRational, Evaluation> {
        if let V::Float(text) = value {
            text.parse::<f64>()
                .ok()
                .filter(|value| value.is_finite())
                .and_then(BigRational::from_float)
                .ok_or_else(|| self.bad(e, "Expected finite Float"))
        } else {
            self.numeric(e, value)
        }
    }
    fn validate(&self, e: &Expression, value: &V, depth: usize) -> Result<(), Evaluation> {
        if depth > self.budgets.recursion_depth {
            return Err(self.issue(e, "VALUE_DEPTH", "Value recursion budget exhausted"));
        }
        match value {
            V::Float(text) if !text.parse::<f64>().is_ok_and(f64::is_finite) => {
                return Err(self.bad(e, "Expected a finite Float"));
            }
            V::Integer(_) | V::Decimal(_) => {
                self.numeric(e, value)?;
            }
            V::List(values) => {
                for value in values {
                    self.validate(e, value, depth + 1)?;
                }
            }
            V::Record(fields) => {
                for value in fields.values() {
                    self.validate(e, value, depth + 1)?;
                }
            }
            V::Option(Some(value))
            | V::Variant {
                payload: Some(value),
                ..
            }
            | V::Failure {
                payload: Some(value),
                ..
            } => self.validate(e, value, depth + 1)?,
            _ => {}
        }
        Ok(())
    }
    fn eval(&mut self, e: &Expression, vars: &BTreeMap<String, V>, depth: usize) -> EvalResult {
        self.steps += 1;
        if self.steps > self.budgets.evaluation_steps || depth > self.budgets.recursion_depth {
            return Err(self.issue(
                e,
                "EVALUATION_BUDGET",
                "Expression evaluation budget exhausted",
            ));
        }
        let result = self.eval_kind(e, vars, depth)?;
        self.validate(e, &result, 0)?;
        let bytes = serde_size(&result);
        if bytes > self.budgets.value_bytes {
            return Err(self.issue(e, "VALUE_SIZE", "Canonical value byte budget exhausted"));
        }
        Ok(result)
    }
    fn eval_kind(
        &mut self,
        e: &Expression,
        vars: &BTreeMap<String, V>,
        depth: usize,
    ) -> EvalResult {
        let next = depth + 1;
        match &e.value {
            E::Literal { text, literal_kind } => match literal_kind.as_str() {
                "Boolean" => match text.as_str() {
                    "true" => Ok(V::Boolean(true)),
                    "false" => Ok(V::Boolean(false)),
                    _ => Err(self.bad(e, "Invalid Boolean literal")),
                },
                "String" => Ok(V::String(text.clone().into())),
                "Float" => Ok(V::Float(text.clone())),
                "Int" | "Decimal" => {
                    let integer = literal_kind == "Int";
                    let value = number::parse(text, integer)
                        .ok_or_else(|| self.bad(e, "Invalid numeric literal"))?;
                    self.numeric_value(e, value, integer)
                }
                _ => Err(self.issue(e, "UNSUPPORTED_LITERAL", "Literal type is not executable")),
            },
            E::Variable { name } => vars.get(name).cloned().ok_or_else(|| {
                self.issue(e, "UNBOUND_VALUE", &format!("No concrete value for {name}"))
            }),
            E::Variant { name, declaration } => Ok(V::Variant {
                declaration: declaration.clone(),
                name: name.clone(),
                payload: None,
            }),
            E::VariantRecord {
                declaration,
                variant,
                payload,
            } => Ok(V::Variant {
                declaration: declaration.clone(),
                name: variant.clone(),
                payload: Some(Box::new(self.eval(payload, vars, next)?)),
            }),
            E::Field { target, name } => match self.eval(target, vars, next)? {
                V::Record(fields) => fields
                    .get(name)
                    .cloned()
                    .ok_or_else(|| self.bad(e, "Missing record field")),
                V::Variant {
                    payload: Some(value),
                    ..
                } if name == "value" => Ok(*value),
                _ => Err(self.bad(e, "Field access requires a record or variant payload")),
            },
            E::Record { fields, .. } => {
                let mut values = BTreeMap::new();
                for (name, expr) in fields {
                    if values
                        .insert(name.clone(), self.eval(expr, vars, next)?)
                        .is_some()
                    {
                        return Err(self.bad(e, "Duplicate record field"));
                    }
                }
                Ok(V::Record(values))
            }
            E::List { values } => Ok(V::List(
                values
                    .iter()
                    .map(|expr| self.eval(expr, vars, next))
                    .collect::<Result<_, _>>()?,
            )),
            E::Unary { operator, operand } => {
                let value = self.eval(operand, vars, next)?;
                match operator.as_str() {
                    "not" => Ok(V::Boolean(!self.boolean(e, value)?)),
                    "-" => self.numeric_value(
                        e,
                        -self.numeric(e, &value)?,
                        matches!(value, V::Integer(_)),
                    ),
                    _ => Err(self.issue(
                        e,
                        "UNSUPPORTED_OPERATOR",
                        "Unary operator is not executable",
                    )),
                }
            }
            E::Binary {
                operator,
                left,
                right,
            } => self.binary(e, operator, left, right, vars, next),
            E::Call {
                name,
                target,
                arguments,
            } => {
                if name == "ifElse" && target.is_none() {
                    let [condition, yes, no] = arguments.as_slice() else {
                        return Err(self.bad(e, "ifElse requires three arguments"));
                    };
                    let condition = self.eval(condition, vars, next)?;
                    return self.eval(
                        if self.boolean(e, condition)? { yes } else { no },
                        vars,
                        next,
                    );
                }
                let values = arguments
                    .iter()
                    .map(|expr| self.eval(expr, vars, next))
                    .collect::<Result<Vec<_>, _>>()?;
                if let Some(subject) = target {
                    if let Some((predicate, entity)) = name.split_once(':') {
                        if self.ir.data.contains_key(subject) && entity == subject {
                            return self.state_predicate(e, predicate, subject, &values);
                        }
                    }
                    if !self.ir.operations.get(subject).is_some_and(|o| o.pure) {
                        return Err(self.issue(
                            e,
                            "IMPURE_OPERATION",
                            "Operation needs effects or unavailable declaration",
                        ));
                    }
                    let invocation = Invocation {
                        subject: subject.clone(),
                        arguments: values,
                    };
                    let Some(record) = self.observations.get(self.cursor) else {
                        return Err(Evaluation::Invoke(invocation));
                    };
                    if record.invocation != invocation {
                        return Err(self.issue(
                            e,
                            "OBSERVATION_MISMATCH",
                            "Observation does not match the requested invocation",
                        ));
                    }
                    self.cursor += 1;
                    match &record.observation {
                        Observation::Returned { value } => {
                            let contract = check_value(
                                self.ir,
                                &self.ir.operations[subject].result,
                                value,
                                self.budgets,
                            );
                            match contract {
                                ValueContract::Accepted => Ok(value.clone()),
                                ValueContract::Rejected { reason } => {
                                    Err(self.issue(e, "INVALID_RETURN_TYPE", &reason))
                                }
                                ValueContract::Unknown { mut issue } => {
                                    if issue.span.is_none() {
                                        issue.span = Some(e.span.clone());
                                    }
                                    Err(Evaluation::Unknown(issue))
                                }
                            }
                        }
                        Observation::Unavailable { reason } => {
                            Err(self.issue(e, "OPERATION_UNAVAILABLE", reason))
                        }
                        Observation::Threw { message } => {
                            Err(Evaluation::ApplicationFailure(VerificationIssue {
                                code: "APPLICATION_EXCEPTION".into(),
                                message: message.clone(),
                                span: Some(e.span.clone()),
                            }))
                        }
                    }
                } else {
                    self.builtin(e, name, values)
                }
            }
            E::Collection {
                operator,
                binder,
                collection,
                predicate,
                projection,
            } => {
                let V::List(values) = self.eval(collection, vars, next)? else {
                    return Err(self.bad(e, "Collection requires a List"));
                };
                self.collection(
                    e,
                    operator,
                    binder.as_deref(),
                    values,
                    (predicate.as_deref(), projection.as_deref()),
                    vars,
                    next,
                )
            }
            E::State {
                phase,
                entity,
                keys,
            } => {
                let values = keys
                    .iter()
                    .map(|key| self.eval(key, vars, next))
                    .collect::<Result<Vec<_>, _>>()?;
                let selected = self.selected_state(e, *phase, entity, &values)?;
                if keys.is_empty() {
                    Ok(V::List(selected))
                } else {
                    selected.into_iter().next().ok_or_else(|| {
                        self.issue(
                            e,
                            "STATE_ENTITY_MISSING",
                            "Keyed state access has no observed entity",
                        )
                    })
                }
            }
            E::Trace {
                trace_kind,
                modeled_target,
                arguments,
                count,
                ..
            } => self.trace(
                e,
                observed::TraceInput {
                    kind: trace_kind,
                    target: modeled_target.as_deref(),
                    arguments,
                    count,
                },
                vars,
                next,
            ),
        }
    }
    fn equal(&self, e: &Expression, left: &V, right: &V) -> Result<bool, Evaluation> {
        match (left, right) {
            (V::Float(_), V::Integer(_) | V::Decimal(_))
            | (V::Integer(_) | V::Decimal(_), V::Float(_)) => {
                Ok(self.comparison_number(e, left)? == self.comparison_number(e, right)?)
            }
            (V::Float(a), V::Float(b)) => {
                let a = a
                    .parse::<f64>()
                    .ok()
                    .filter(|value| value.is_finite())
                    .ok_or_else(|| self.bad(e, "Expected finite Float"))?;
                let b = b
                    .parse::<f64>()
                    .ok()
                    .filter(|value| value.is_finite())
                    .ok_or_else(|| self.bad(e, "Expected finite Float"))?;
                Ok(a == b)
            }
            (V::Integer(_) | V::Decimal(_), V::Integer(_) | V::Decimal(_)) => {
                Ok(self.numeric(e, left)? == self.numeric(e, right)?)
            }
            (V::List(a), V::List(b)) => {
                if a.len() != b.len() {
                    return Ok(false);
                }
                for (a, b) in a.iter().zip(b) {
                    if !self.equal(e, a, b)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            (V::Record(a), V::Record(b)) => {
                if a.keys().ne(b.keys()) {
                    return Ok(false);
                }
                for (name, a) in a {
                    if !self.equal(e, a, &b[name])? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            (V::Option(Some(a)), V::Option(Some(b))) => self.equal(e, a, b),
            (
                V::Variant {
                    declaration: a,
                    name: an,
                    payload: ap,
                },
                V::Variant {
                    declaration: b,
                    name: bn,
                    payload: bp,
                },
            ) => {
                if a != b || an != bn {
                    return Ok(false);
                }
                match (ap, bp) {
                    (Some(a), Some(b)) => self.equal(e, a, b),
                    _ => Ok(ap == bp),
                }
            }
            _ => Ok(left == right),
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn binary(
        &mut self,
        e: &Expression,
        op: &str,
        left: &Expression,
        right: &Expression,
        vars: &BTreeMap<String, V>,
        depth: usize,
    ) -> EvalResult {
        let left = self.eval(left, vars, depth)?;
        if matches!(op, "and" | "or" | "implies") {
            let value = self.boolean(e, left)?;
            if op == "and" && !value {
                return Ok(V::Boolean(false));
            }
            if (op == "or" && value) || (op == "implies" && !value) {
                return Ok(V::Boolean(true));
            }
            let right = self.eval(right, vars, depth)?;
            return Ok(V::Boolean(self.boolean(e, right)?));
        }
        let right = self.eval(right, vars, depth)?;
        if matches!(op, "<" | "<=" | ">" | ">=")
            && (matches!(&left, V::Float(_)) || matches!(&right, V::Float(_)))
        {
            let a = self.comparison_number(e, &left)?;
            let b = self.comparison_number(e, &right)?;
            return Ok(V::Boolean(match op {
                "<" => a < b,
                "<=" => a <= b,
                ">" => a > b,
                _ => a >= b,
            }));
        }
        match op {
            "==" | "!=" => return Ok(V::Boolean(self.equal(e, &left, &right)? == (op == "=="))),
            "is" => {
                let matches = match (&left, &right) {
                    (
                        V::Failure { declaration, .. },
                        V::Variant {
                            declaration: expected,
                            ..
                        },
                    ) => declaration == expected,
                    (
                        V::Variant {
                            declaration: a,
                            name: an,
                            ..
                        },
                        V::Variant {
                            declaration: b,
                            name: bn,
                            ..
                        },
                    ) => a == b && an == bn,
                    _ => self.equal(e, &left, &right)?,
                };
                return Ok(V::Boolean(matches));
            }
            "in" => {
                let V::List(values) = right else {
                    return Err(self.bad(e, "Membership requires List or evaluated range"));
                };
                for value in values {
                    if self.equal(e, &left, &value)? {
                        return Ok(V::Boolean(true));
                    }
                }
                return Ok(V::Boolean(false));
            }
            "+" if matches!(left, V::String(_)) => {
                let (V::String(a), V::String(b)) = (left, right) else {
                    return Err(self.bad(e, "String concatenation requires two Strings"));
                };
                return Ok(V::String(format!("{a}{b}").into()));
            }
            _ => {}
        }
        let a = self.numeric(e, &left)?;
        let b = self.numeric(e, &right)?;
        let integer = matches!(left, V::Integer(_)) && matches!(right, V::Integer(_));
        let result = match op {
            "<" => return Ok(V::Boolean(a < b)),
            "<=" => return Ok(V::Boolean(a <= b)),
            ">" => return Ok(V::Boolean(a > b)),
            ">=" => return Ok(V::Boolean(a >= b)),
            "+" => a + b,
            "-" => a - b,
            "*" => a * b,
            "/" | "%" => {
                if op == "%" && !integer {
                    return Err(self.issue(
                        e,
                        "UNSUPPORTED_OPERATOR",
                        "Decimal remainder is not supported",
                    ));
                }
                if b.is_zero() {
                    return Err(self.issue(e, "DIVISION_BY_ZERO", "Divisor must be nonzero"));
                }
                if op == "/" { a / b } else { a % b }
            }
            ".." => {
                if !a.is_integer() || !b.is_integer() {
                    return Err(self.bad(e, "Range endpoints must be integral"));
                }
                let count = (&b - &a)
                    .to_integer()
                    .to_usize()
                    .and_then(|n| n.checked_add(1))
                    .ok_or_else(|| self.bad(e, "Descending or oversized range"))?;
                if count > self.budgets.evaluation_steps.saturating_sub(self.steps) {
                    return Err(self.issue(
                        e,
                        "EVALUATION_BUDGET",
                        "Range exceeds evaluation budget",
                    ));
                }
                self.steps += count;
                return Ok(V::List(
                    (0..count)
                        .map(|n| V::Integer((a.to_integer() + n).to_string()))
                        .collect(),
                ));
            }
            _ => {
                return Err(self.issue(
                    e,
                    "UNSUPPORTED_OPERATOR",
                    "Binary operator is not executable",
                ));
            }
        };
        self.numeric_value(e, result, integer)
    }
    fn builtin(&mut self, e: &Expression, name: &str, args: Vec<V>) -> EvalResult {
        match (name, args.as_slice()) {
            ("None", []) => Ok(V::Option(None)),
            ("Some", [value]) => Ok(V::Option(Some(Box::new(value.clone())))),
            ("first", [V::List(values)]) => Ok(V::Option(values.first().cloned().map(Box::new))),
            ("single", [V::List(values)]) => match values.as_slice() {
                [value] => Ok(value.clone()),
                _ => Err(self.issue(
                    e,
                    "RESULT_CARDINALITY_REQUIRED",
                    "single requires exactly one observed value",
                )),
            },
            ("getOrElse", [V::Option(value), fallback]) => {
                Ok(value.as_deref().unwrap_or(fallback).clone())
            }
            ("indexOf", [V::List(values), needle]) => {
                for (index, value) in values.iter().enumerate() {
                    self.steps += 1;
                    if self.steps > self.budgets.evaluation_steps {
                        return Err(self.issue(
                            e,
                            "EVALUATION_BUDGET",
                            "List search exceeds evaluation budget",
                        ));
                    }
                    if self.equal(e, value, needle)? {
                        return Ok(V::Integer(index.to_string()));
                    }
                }
                Ok(V::Integer("-1".into()))
            }
            ("mean", [V::List(values)]) => {
                if values.is_empty() {
                    return Ok(V::Option(None));
                }
                let mut sum = BigRational::zero();
                for value in values {
                    self.steps += 1;
                    if self.steps > self.budgets.evaluation_steps {
                        return Err(self.issue(
                            e,
                            "EVALUATION_BUDGET",
                            "Mean exceeds evaluation budget",
                        ));
                    }
                    sum += self.numeric(e, value)?;
                }
                let mean = sum / BigRational::from_integer(values.len().into());
                let value = mean
                    .to_f64()
                    .filter(|value| value.is_finite())
                    .ok_or_else(|| {
                        self.issue(
                            e,
                            "FLOAT_REPRESENTATION_REQUIRED",
                            "Mean cannot be represented as a finite Float",
                        )
                    })?;
                Ok(V::Option(Some(Box::new(V::Float(value.to_string())))))
            }
            ("lowercase", [V::String(s)]) => Ok(V::String(s.to_lowercase().into())),
            ("uppercase", [V::String(s)]) => Ok(V::String(s.to_uppercase().into())),
            // Match the reference's ECMAScript WhiteSpace/LineTerminator set;
            // Rust's Unicode trim includes NEL and excludes BOM.
            ("trim", [V::String(s)]) => Ok(V::String(
                s.trim_matches(|character| {
                    matches!(character,
                        '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{00a0}' | '\u{1680}' |
                        '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' |
                        '\u{205f}' | '\u{3000}' | '\u{feff}'
                    )
                })
                .into(),
            )),
            ("length", [V::String(s)]) => Ok(V::Integer(s.encode_utf16().count().to_string())),
            ("length", [V::List(v)]) => Ok(V::Integer(v.len().to_string())),
            ("startsWith", [V::String(s), V::String(prefix)]) => {
                Ok(V::Boolean(s.starts_with(prefix.as_ref())))
            }
            ("endsWith", [V::String(s), V::String(suffix)]) => {
                Ok(V::Boolean(s.ends_with(suffix.as_ref())))
            }
            ("includesText", [V::String(text), V::String(query)]) => {
                Ok(V::Boolean(text.contains(query.as_ref())))
            }
            ("slice", [V::List(values), V::Integer(offset), V::Integer(limit)]) => {
                let offset = number::parse(offset, true)
                    .ok_or_else(|| self.bad(e, "Invalid slice offset"))?;
                let limit =
                    number::parse(limit, true).ok_or_else(|| self.bad(e, "Invalid slice limit"))?;
                if offset < BigRational::zero() || limit < BigRational::zero() {
                    return Err(self.issue(
                        e,
                        "SLICE_INDEX_REQUIRED",
                        "Slice offset and limit must be nonnegative",
                    ));
                }
                let length = BigRational::from_integer(values.len().into());
                if offset >= length {
                    return Ok(V::List(vec![]));
                }
                let offset = offset
                    .to_integer()
                    .to_usize()
                    .ok_or_else(|| self.bad(e, "Invalid slice offset"))?;
                let available = values.len() - offset;
                let count = if limit >= BigRational::from_integer(available.into()) {
                    available
                } else {
                    limit
                        .to_integer()
                        .to_usize()
                        .ok_or_else(|| self.bad(e, "Invalid slice limit"))?
                };
                Ok(V::List(
                    values.iter().skip(offset).take(count).cloned().collect(),
                ))
            }
            ("round", [value, scale, mode]) => {
                let number = self.numeric(e, value)?;
                self.round_decimal(e, number, scale, mode)
            }
            ("divide", [left, right, scale, mode]) => {
                let numerator = self.numeric(e, left)?;
                let denominator = self.numeric(e, right)?;
                if denominator.is_zero() {
                    return Err(self.issue(e, "DIVISION_BY_ZERO", "Divisor must be nonzero"));
                }
                let quotient = numerator / denominator;
                // Preserve the reference evaluator's exact finite quotient.
                if let Some(text) = number::decimal(&quotient) {
                    self.round_decimal(e, quotient.clone(), scale, mode)?;
                    return Ok(V::Decimal(text));
                }
                self.round_decimal(e, quotient, scale, mode)
            }
            ("reverse", [V::List(v)]) => Ok(V::List(v.iter().rev().cloned().collect())),
            _ => Err(self.issue(
                e,
                "UNSUPPORTED_BUILTIN",
                "Builtin or argument representation is not supported",
            )),
        }
    }
    fn round_decimal(&self, e: &Expression, value: BigRational, scale: &V, mode: &V) -> EvalResult {
        let V::Integer(text) = scale else {
            return Err(self.bad(e, "Decimal scale must be a nonnegative integer"));
        };
        let scale = number::parse(text, true)
            .and_then(|value| value.to_integer().to_u32())
            .ok_or_else(|| self.bad(e, "Decimal scale must be a nonnegative integer"))?;
        if scale as usize > self.budgets.value_bytes.saturating_sub(32)
            || scale as usize > self.budgets.evaluation_steps.saturating_sub(self.steps)
        {
            return Err(self.issue(
                e,
                "EVALUATION_BUDGET",
                "Decimal scale exceeds the declared budget",
            ));
        }
        let V::Variant {
            declaration,
            name,
            payload: None,
        } = mode
        else {
            return Err(self.bad(e, "Expected a rounding mode"));
        };
        if declaration != "RoundingMode" {
            return Err(self.bad(e, "Expected a rounding mode"));
        }
        let rounded = rounding::round(value, scale, name)
            .ok_or_else(|| self.bad(e, "Unknown rounding mode"))?;
        number::decimal(&rounded)
            .map(V::Decimal)
            .ok_or_else(|| self.bad(e, "Rounded decimal is not finite"))
    }
    #[allow(clippy::too_many_arguments)]
    fn collection(
        &mut self,
        e: &Expression,
        op: &str,
        binder: Option<&str>,
        values: Vec<V>,
        clauses: (Option<&Expression>, Option<&Expression>),
        vars: &BTreeMap<String, V>,
        depth: usize,
    ) -> EvalResult {
        let (predicate, projection) = clauses;
        if op == "contains" {
            let expected =
                predicate.ok_or_else(|| self.bad(e, "contains needs a comparison value"))?;
            let expected = self.eval(expected, vars, depth)?;
            for value in &values {
                if self.equal(e, value, &expected)? {
                    return Ok(V::Boolean(true));
                }
            }
            return Ok(V::Boolean(false));
        }
        let mut selected = Vec::new();
        let mut keyed = Vec::new();
        let mut count = 0usize;
        for value in values {
            let original = matches!(op, "sortBy" | "sortDescBy").then(|| value.clone());
            self.steps += 1;
            if self.steps > self.budgets.evaluation_steps {
                return Err(self.issue(
                    e,
                    "EVALUATION_BUDGET",
                    "Collection exceeds evaluation budget",
                ));
            }
            let mut context = vars.clone();
            if let Some(name) = binder {
                context.insert(name.into(), value.clone());
            }
            let matches = if let Some(predicate) = predicate {
                let value = self.eval(predicate, &context, depth)?;
                self.boolean(e, value)?
            } else if matches!(op, "all" | "any") {
                self.boolean(e, value.clone())?
            } else {
                true
            };
            if op == "all" && !matches {
                return Ok(V::Boolean(false));
            }
            if op == "any" && matches {
                return Ok(V::Boolean(true));
            }
            if matches {
                count += 1;
            }
            let value = if let Some(projection) = projection {
                self.eval(projection, &context, depth)?
            } else {
                value
            };
            if let Some(original) = original {
                keyed.push((value, original));
            } else if op != "filter" || matches {
                selected.push(value);
            }
        }
        match op {
            "sortBy" | "sortDescBy" => sorting::sort_direction(
                keyed,
                &mut self.steps,
                self.budgets.evaluation_steps,
                op == "sortDescBy",
            )
            .map(V::List)
            .map_err(|code| {
                self.issue(
                    e,
                    code,
                    "Sorting requires supported keys and sufficient comparison budget",
                )
            }),
            "filter" | "map" => Ok(V::List(selected)),
            "count" => Ok(V::Integer(count.to_string())),
            "all" => Ok(V::Boolean(true)),
            "any" => Ok(V::Boolean(false)),
            "distinct" => {
                let mut unique = Vec::new();
                for value in selected {
                    let mut duplicate = false;
                    for prior in &unique {
                        if self.equal(e, &value, prior)? {
                            duplicate = true;
                            break;
                        }
                    }
                    if !duplicate {
                        unique.push(value);
                    }
                }
                Ok(V::List(unique))
            }
            "sum" => {
                let mut sum = BigRational::zero();
                let mut integer = true;
                for value in selected {
                    integer &= matches!(value, V::Integer(_));
                    sum += self.numeric(e, &value)?;
                }
                self.numeric_value(e, sum, integer)
            }
            "min" | "max" => {
                let mut iter = selected.into_iter();
                let mut best = iter.next().ok_or_else(|| {
                    self.issue(
                        e,
                        "EMPTY_AGGREGATE",
                        "min/max on an empty collection has no value",
                    )
                })?;
                for value in iter {
                    let ordering = self.numeric(e, &value)?.cmp(&self.numeric(e, &best)?);
                    if (op == "min" && ordering.is_lt()) || (op == "max" && ordering.is_gt()) {
                        best = value;
                    }
                }
                Ok(best)
            }
            _ => Err(self.issue(
                e,
                "UNSUPPORTED_COLLECTION",
                "Collection operator is not executable",
            )),
        }
    }
}

fn serde_size(value: &V) -> usize {
    // The value enum has no fallible JSON fields. Saturation prevents overflow.
    serde_json::to_vec(value).map_or(usize::MAX, |bytes| bytes.len())
}
