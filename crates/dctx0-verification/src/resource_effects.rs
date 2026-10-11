//! Resource permissions are assessed only from actual adapter access events.
use crate::OperationPlan;
use dctx0_core::*;
use dctx0_interpreter::ObservationContext;
fn outcome(node: &SemanticNode, code: &str, message: String, failed: bool) -> VerificationResult {
    let issue = VerificationIssue {
        code: code.into(),
        message,
        span: Some(node.span.clone()),
    };
    if failed {
        VerificationResult::Fail { issue }
    } else {
        VerificationResult::Unknown { issue }
    }
}
fn kind(access: ResourceAccess) -> ReferenceKind {
    match access {
        ResourceAccess::Read => ReferenceKind::Reads,
        ResourceAccess::Write => ReferenceKind::Writes,
        ResourceAccess::Observe => ReferenceKind::Observes,
    }
}
pub(crate) fn assess_resource(
    ir: &WorkspaceIr,
    plan: &OperationPlan,
    node: &SemanticNode,
    context: &ObservationContext,
    access: ResourceAccess,
) -> VerificationResult {
    let Some(resources) = &context.resources else {
        return outcome(
            node,
            "RESOURCE_OBSERVER_REQUIRED",
            "Actual resource access observation is unavailable".into(),
            false,
        );
    };
    let reference_kind = kind(access);
    let Some(reference) = ir.references.iter().find(|r| {
        r.from == plan.operation.id
            && r.kind == reference_kind
            && r.span.file == node.span.file
            && r.span.start.offset >= node.span.start.offset
            && r.span.end.offset <= node.span.end.offset
    }) else {
        return outcome(
            node,
            "RESOURCE_REFERENCE_REQUIRED",
            "Resource effect has no resolved reference".into(),
            false,
        );
    };
    if !ir.symbols.contains_key(&reference.target) {
        return outcome(
            node,
            "RESOURCE_CONTRACT_REQUIRED",
            "Resource target is not modeled".into(),
            false,
        );
    }
    assess_boundary(ir, plan, node, context, resources)
}
/// Check all delivered accesses, including operations with no resource clauses.
pub(crate) fn assess_boundary(
    ir: &WorkspaceIr,
    plan: &OperationPlan,
    node: &SemanticNode,
    context: &ObservationContext,
    resources: &[ResourceObservation],
) -> VerificationResult {
    for observed in resources {
        if !ir.symbols.contains_key(&observed.resource) {
            return outcome(
                node,
                "RESOURCE_LABEL_UNMODELED",
                format!("Actual label {} is not modeled", observed.resource),
                false,
            );
        }
        let owner = observed.operation.as_deref().unwrap_or(&plan.operation.id);
        if owner != plan.operation.id
            && !context
                .calls
                .as_ref()
                .is_some_and(|calls| calls.iter().any(|call| call.invocation.subject == owner))
        {
            return outcome(
                node,
                "RESOURCE_OPERATION_UNOBSERVED",
                format!(
                    "Resource access names operation {owner} without an actual call observation"
                ),
                false,
            );
        }
        if !ir.symbols.contains_key(owner) {
            return outcome(
                node,
                "RESOURCE_OPERATION_UNMODELED",
                format!("Resource access names unmodeled operation {owner}"),
                false,
            );
        }
        if owner != plan.operation.id
            && !ir.references.iter().any(|reference| {
                reference.from == plan.operation.id
                    && reference.target == owner
                    && reference.kind == ReferenceKind::Calls
            })
        {
            return outcome(
                node,
                "RESOURCE_CALL_UNDECLARED",
                format!("Resource access was delegated to undeclared collaborator {owner}"),
                true,
            );
        }
        if !ir.references.iter().any(|r| {
            r.from == owner && r.target == observed.resource && r.kind == kind(observed.access)
        }) {
            return outcome(
                node,
                "UNDECLARED_RESOURCE_ACCESS",
                format!(
                    "Actual {:?} access to {} is outside the declared effects",
                    observed.access, observed.resource
                ),
                true,
            );
        }
    }
    // Permission is not a minimum/exact access count. Such counts require an
    // explicit trace assertion. Some([]) represents actual observed zero.
    VerificationResult::Pass
}
