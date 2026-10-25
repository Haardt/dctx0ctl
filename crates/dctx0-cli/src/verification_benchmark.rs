//! Core-only workloads, deliberately without application execution or adoption.
use crate::{args::Phase, benchmark::benchmark_report};
use dctx0_core::{VerificationBudgets, WorkspaceIr};
use dctx0_verification::{Obligation, VerificationCase, VerificationPlan, VerificationScope};
use std::{collections::BTreeMap, io::Write};

pub(crate) fn benchmark(
    ir: &WorkspaceIr,
    phase: Phase,
    iterations: usize,
    files: usize,
    output: &mut dyn Write,
) -> Result<bool, String> {
    if matches!(phase, Phase::VerificationBindingsPlan) {
        let plan = dctx0_verification::project_program(
            ir,
            "benchmark-only",
            1,
            VerificationScope {
                subjects: [ir.workspace.clone()].into(),
                tasks: Default::default(),
            },
            &Default::default(),
            Default::default(),
        );
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            std::hint::black_box(dctx0_verification::binding_requirements(ir, &plan));
        }
        return benchmark_report(output, start, iterations, files);
    }
    let mut subjects = std::collections::BTreeSet::new();
    let obligations = ir
        .nodes
        .iter()
        .filter(|node| !node.expressions.is_empty())
        .map(|node| {
            subjects.insert(node.owner.clone());
            Obligation {
                id: node.id.clone(),
                subject: node.owner.clone(),
                unsupported: None,
                bounded_corpus: false,
                cases: node
                    .expressions
                    .iter()
                    .enumerate()
                    .map(|(index, expression)| VerificationCase {
                        id: format!("{}:{index}", node.id),
                        variables: BTreeMap::new(),
                        assertion: expression.clone(),
                    })
                    .collect(),
            }
        })
        .collect();
    // This synthetic benchmark plan is not a projection of complete requirements.
    // Unbound variables and missing observations are intentionally assessed UNKNOWN.
    let plan = VerificationPlan {
        schema_version: 1,
        snapshot: "benchmark-only".into(),
        check_occurrence: 1,
        scope: VerificationScope {
            subjects,
            tasks: Default::default(),
        },
        budgets: VerificationBudgets::default(),
        obligations,
    };
    let evidence = plan
        .obligations
        .iter()
        .flat_map(|o| &o.cases)
        .map(|case| dctx0_verification::CaseEvidence {
            case_id: case.id.clone(),
            observations: Vec::new(),
        })
        .collect::<Vec<_>>();
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        if matches!(phase, Phase::VerificationProject) {
            std::hint::black_box(dctx0_verification::project_program(
                ir,
                "benchmark-only",
                1,
                dctx0_verification::VerificationScope {
                    subjects: std::collections::BTreeSet::from([ir.workspace.clone()]),
                    tasks: Default::default(),
                },
                &Default::default(),
                Default::default(),
            ));
        } else if matches!(phase, Phase::VerificationGenerate) {
            for data in ir.data.values() {
                let ty = dctx0_core::TypeRef {
                    name: data.id.clone(),
                    symbol_id: Some(data.id.clone()),
                    arguments: Vec::new(),
                };
                std::hint::black_box(dctx0_verification::generate_values(ir, &ty, &plan.budgets));
            }
        } else if matches!(phase, Phase::VerificationEvaluateUnbound) {
            for obligation in &plan.obligations {
                for case in &obligation.cases {
                    std::hint::black_box(dctx0_interpreter::evaluate(
                        ir,
                        &case.assertion,
                        &case.variables,
                        &[],
                        &plan.budgets,
                    ));
                }
            }
        } else {
            std::hint::black_box(dctx0_verification::assess(ir, &plan, &evidence));
        }
    }
    benchmark_report(output, start, iterations, files)
}
