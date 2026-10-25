//! Implementation-independent requirements from the exact published selection.
use crate::ProgramPlan;
use dctx0_core::*;
use std::collections::{BTreeMap, BTreeSet};

pub fn binding_requirements(ir: &WorkspaceIr, plan: &ProgramPlan) -> BindingRequirements {
    let mut result = BindingRequirements {
        schema_version: BINDINGS_SCHEMA_VERSION,
        snapshot: plan.snapshot.clone(),
        check_occurrence: plan.check_occurrence,
        selected_subjects: plan.scope.subjects.clone(),
        requirements: vec![],
        declarations: BTreeMap::new(),
        capabilities: plan
            .capabilities
            .iter()
            .filter_map(|c| c.adapter_capability().map(str::to_owned))
            .collect(),
        issues: plan.issues.clone(),
    };
    let mut requirements = BTreeSet::new();
    let mut types = BTreeSet::new();
    let mut operations = BTreeSet::new();
    for contract in &plan.contracts {
        requirements.insert((contract.subject.clone(), BindingRole::Validator));
        add_type(&contract.ty, &mut types);
        if !contract.identity_requirements.is_empty() {
            requirements.insert((contract.subject.clone(), BindingRole::EntityObserver));
        }
    }
    operations.extend(plan.operations.iter().map(|o| o.operation.id.clone()));
    // Closure includes pure helpers and reviewed preparations without unrelated
    // sibling operations. Opaque trace spellings are deliberately not bindings.
    operations.extend(
        plan.closure
            .iter()
            .filter(|id| ir.operations.contains_key(*id))
            .cloned(),
    );
    for lifecycle in &plan.lifecycles {
        requirements.insert((lifecycle.entity.clone(), BindingRole::EntityObserver));
        operations.extend(lifecycle.transitions.iter().map(|t| t.operation.clone()));
    }
    loop {
        let before = (operations.len(), types.len(), requirements.len());
        for reference in &ir.references {
            if operations.contains(&reference.from) || types.contains(&reference.from) {
                if reference.kind == ReferenceKind::Calls
                    && ir.operations.contains_key(&reference.target)
                {
                    operations.insert(reference.target.clone());
                }
                if matches!(
                    reference.kind,
                    ReferenceKind::Reads
                        | ReferenceKind::Writes
                        | ReferenceKind::Observes
                        | ReferenceKind::State
                ) && ir
                    .symbols
                    .get(&reference.target)
                    .is_some_and(|s| s.kind == StatementKind::Entity)
                {
                    requirements.insert((reference.target.clone(), BindingRole::EntityObserver));
                    types.insert(reference.target.clone());
                }
            }
        }
        for subject in operations.clone() {
            if let Some(operation) = ir.operations.get(&subject) {
                for ty in operation
                    .parameters
                    .iter()
                    .map(|p| &p.ty)
                    .chain([&operation.result])
                    .chain(&operation.failures)
                {
                    add_type(ty, &mut types);
                }
                requirements.insert((subject, BindingRole::Operation));
            } else {
                let issue = VerificationIssue {
                    code: "BINDING_OPERATION_MISSING".into(),
                    message: subject,
                    span: None,
                };
                if !result.issues.contains(&issue) {
                    result.issues.push(issue);
                }
            }
        }
        for subject in types.clone() {
            if let Some(data) = ir.data.get(&subject) {
                for ty in data
                    .alias
                    .iter()
                    .chain(data.fields.iter().map(|f| &f.ty))
                    .chain(data.variants.iter().filter_map(|v| v.payload.as_ref()))
                {
                    add_type(ty, &mut types);
                }
            }
        }
        if before == (operations.len(), types.len(), requirements.len()) {
            break;
        }
    }
    for subject in types {
        if let Some(data) = ir.data.get(&subject) {
            result.declarations.insert(subject, data.clone());
        } else {
            result.issues.push(VerificationIssue {
                code: "BINDING_TYPE_MISSING".into(),
                message: subject.clone(),
                span: ir.symbols.get(&subject).map(|s| s.span.clone()),
            });
        }
    }
    for (subject, role) in requirements {
        let span = ir.symbols.get(&subject).map(|s| s.span.clone());
        let operation = (role == BindingRole::Operation)
            .then(|| ir.operations.get(&subject).cloned())
            .flatten();
        result.requirements.push(BindingRequirement {
            subject,
            role,
            span,
            operation,
        });
    }
    result
}

fn add_type(ty: &TypeRef, pending: &mut BTreeSet<String>) {
    if let Some(id) = &ty.symbol_id {
        pending.insert(id.clone());
    }
    for argument in &ty.arguments {
        add_type(argument, pending);
    }
}
