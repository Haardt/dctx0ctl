//! Actual before/after state and runtime traces. Absence of observers is not zero.
use super::*;
use dctx0_core::{StatePhase, VerificationEvent};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservationContext {
    pub phase: StatePhase,
    pub before: Option<BTreeMap<String, Vec<V>>>,
    pub after: Option<BTreeMap<String, Vec<V>>>,
    pub calls: Option<Vec<InvocationObservation>>,
    pub events: Option<Vec<VerificationEvent>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<Vec<dctx0_core::ResourceObservation>>,
}
impl Default for ObservationContext {
    fn default() -> Self {
        Self {
            phase: StatePhase::Before,
            before: None,
            after: None,
            calls: None,
            events: None,
            resources: None,
        }
    }
}
pub(super) struct TraceInput<'a> {
    pub(super) kind: &'a str,
    pub(super) target: Option<&'a str>,
    pub(super) arguments: &'a [Expression],
    pub(super) count: &'a Expression,
}
impl Evaluator<'_> {
    pub(super) fn selected_state(
        &mut self,
        e: &Expression,
        phase: StatePhase,
        entity: &str,
        keys: &[V],
    ) -> Result<Vec<V>, Evaluation> {
        let state = match phase {
            StatePhase::Before => &self.context.before,
            StatePhase::After => &self.context.after,
        };
        let rows = state
            .as_ref()
            .and_then(|state| state.get(entity))
            .ok_or_else(|| {
                self.issue(
                    e,
                    "STATE_OBSERVER_REQUIRED",
                    "Required entity/phase has no actual state observer",
                )
            })?;
        let data = self.ir.data.get(entity).ok_or_else(|| {
            self.issue(e, "STATE_DECLARATION", "Observed entity has no declaration")
        })?;
        if !keys.is_empty() && keys.len() != data.keys.len() {
            return Err(self.issue(
                e,
                "STATE_KEY_ARITY",
                "State lookup does not contain every key component",
            ));
        }
        let ty = dctx0_core::TypeRef {
            name: entity.into(),
            symbol_id: Some(entity.into()),
            arguments: vec![],
        };
        let mut selected = Vec::new();
        for row in rows {
            let V::Record(fields) = row else {
                return Err(self.bad(e, "Entity observation must be a record"));
            };
            self.steps += 1;
            if self.steps > self.budgets.evaluation_steps {
                return Err(self.issue(
                    e,
                    "EVALUATION_BUDGET",
                    "State traversal exceeds the evaluation budget",
                ));
            }
            match check_value(self.ir, &ty, row, self.budgets) {
                ValueContract::Accepted => {}
                ValueContract::Rejected { reason } => {
                    return Err(self.bad(e, &format!("Invalid observed state: {reason}")));
                }
                ValueContract::Unknown { issue } => return Err(Evaluation::Unknown(issue)),
            }
            let mut matches = true;
            for (key, value) in data.keys.iter().zip(keys) {
                let field = fields
                    .get(key)
                    .ok_or_else(|| self.bad(e, "Observed entity is missing its key"))?;
                matches &= self.equal(e, field, value)?;
            }
            if matches {
                selected.push(row.clone());
            }
        }
        if !keys.is_empty() && selected.len() > 1 {
            return Err(self.issue(
                e,
                "AMBIGUOUS_STATE_IDENTITY",
                "More than one actual entity matches the complete key",
            ));
        }
        Ok(selected)
    }
    pub(super) fn state_predicate(
        &mut self,
        e: &Expression,
        predicate: &str,
        entity: &str,
        keys: &[V],
    ) -> EvalResult {
        if self
            .ir
            .data
            .get(entity)
            .is_some_and(|data| data.keys.len() != keys.len())
        {
            return Err(self.issue(
                e,
                "STATE_KEY_ARITY",
                "State predicate requires every declared key",
            ));
        }
        if predicate == "exists" {
            return Ok(V::Boolean(
                !self
                    .selected_state(e, self.context.phase, entity, keys)?
                    .is_empty(),
            ));
        }
        let before = self.selected_state(e, StatePhase::Before, entity, keys)?;
        let after = self.selected_state(e, StatePhase::After, entity, keys)?;
        let result = match predicate {
            "preserves" => self.equal(e, &V::List(before), &V::List(after))?,
            "creates" => before.is_empty() && !after.is_empty(),
            "removes" => !before.is_empty() && after.is_empty(),
            "updates" => {
                !before.is_empty()
                    && !after.is_empty()
                    && !self.equal(e, &V::List(before), &V::List(after))?
            }
            _ => return Err(self.issue(e, "STATE_PREDICATE", "State predicate is not supported")),
        };
        Ok(V::Boolean(result))
    }
    pub(super) fn trace(
        &mut self,
        e: &Expression,
        input: TraceInput<'_>,
        vars: &BTreeMap<String, V>,
        depth: usize,
    ) -> EvalResult {
        let TraceInput {
            kind: trace_kind,
            target: modeled_target,
            arguments,
            count,
        } = input;
        let target = modeled_target.ok_or_else(|| {
            self.issue(
                e,
                "TRACE_OBSERVER_REQUIRED",
                "Unmodeled trace labels are not runtime dependencies",
            )
        })?;
        let values = arguments
            .iter()
            .map(|arg| self.eval(arg, vars, depth))
            .collect::<Result<Vec<_>, _>>()?;
        let expected = self.eval(count, vars, depth)?;
        let expected = self.numeric(e, &expected)?;
        if !expected.is_integer() {
            return Err(self.bad(e, "Trace count must be integral"));
        }
        let expected = expected
            .to_integer()
            .to_usize()
            .ok_or_else(|| self.bad(e, "Trace count must be a nonnegative integer"))?;
        let mut actual = 0usize;
        match trace_kind {
            "calls" => {
                let calls = self.context.calls.as_ref().ok_or_else(|| {
                    self.issue(
                        e,
                        "TRACE_OBSERVER_REQUIRED",
                        "Actual collaborator calls were not observed",
                    )
                })?;
                if !self.ir.operations.contains_key(target) {
                    return Err(self.issue(
                        e,
                        "TRACE_TARGET",
                        "Call trace has no declared operation",
                    ));
                }
                for call in calls {
                    self.trace_step(e)?;
                    if call.invocation.subject.as_str() == target
                        && self.equal(
                            e,
                            &V::List(call.invocation.arguments.clone()),
                            &V::List(values.clone()),
                        )?
                    {
                        actual += 1;
                    }
                }
            }
            "emits" => {
                let events = self.context.events.as_ref().ok_or_else(|| {
                    self.issue(
                        e,
                        "TRACE_OBSERVER_REQUIRED",
                        "Actual emitted events were not observed",
                    )
                })?;
                if !self
                    .ir
                    .symbols
                    .get(target)
                    .is_some_and(|symbol| symbol.kind == dctx0_core::StatementKind::Event)
                {
                    return Err(self.issue(e, "TRACE_TARGET", "Event trace has no declared event"));
                }
                if values.len() > 1 {
                    return Err(self.bad(e, "Event trace takes at most one payload"));
                }
                for event in events {
                    self.trace_step(e)?;
                    if event.subject.as_str() == target
                        && (values.is_empty() || self.equal(e, &event.payload, &values[0])?)
                    {
                        actual += 1;
                    }
                }
            }
            _ => return Err(self.issue(e, "TRACE_KIND", "Trace kind is not supported")),
        }
        Ok(V::Boolean(actual == expected))
    }
    fn trace_step(&mut self, e: &Expression) -> Result<(), Evaluation> {
        self.steps += 1;
        if self.steps > self.budgets.evaluation_steps {
            Err(self.issue(
                e,
                "EVALUATION_BUDGET",
                "Trace traversal exceeds the evaluation budget",
            ))
        } else {
            Ok(())
        }
    }
}
