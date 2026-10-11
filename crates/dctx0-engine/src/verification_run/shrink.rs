//! Shrink actual failing pure invocations; Rust selects and assesses every attempt.
use super::*;
use dctx0_verification::{
    OperationPlan, SHRINK_VERSION, ShrinkAttempt, ShrinkReport, shrink_invocations,
};
fn same_failure(original: &VerificationResult, result: &VerificationResult) -> bool {
    matches!((original,result),(VerificationResult::Fail{issue:a},VerificationResult::Fail{issue:b}) if a.code==b.code&&a.span==b.span)
}
pub(super) async fn pure(
    worker: &mut NativeWorker,
    ir: &WorkspaceIr,
    plan: &OperationPlan,
    original: Invocation,
    failure: &VerificationResult,
    budgets: &VerificationBudgets,
    cancel: &NativeCancellation,
) -> (ShrinkReport, Vec<Vec<NativeSessionEvidence>>) {
    let mut evidence = Vec::new();
    let mut report = ShrinkReport {
        version: SHRINK_VERSION.into(),
        original: original.clone(),
        minimized: original,
        attempts: vec![],
        exhausted: false,
        issues: vec![],
    };
    if !matches!(failure, VerificationResult::Fail { .. }) {
        report.issues.push(issue(
            "SHRINK_FAILURE_REQUIRED",
            "Pure-operation shrinking requires an observed failing assessment",
        ));
        return (report, evidence);
    }
    if budgets.shrink_steps == 0 {
        report.issues.push(issue(
            "SHRINK_BUDGET_EXHAUSTED",
            "Shrinking is disabled by the configured step budget",
        ));
        return (report, evidence);
    }
    let mut visited = BTreeSet::new();
    loop {
        let (candidates, issues) =
            shrink_invocations(ir, plan, &report.minimized, &Default::default(), budgets);
        report.issues.extend(issues);
        let mut improved = false;
        for candidate in candidates {
            if report.attempts.len() >= budgets.shrink_steps {
                report.issues.push(issue(
                    "SHRINK_BUDGET_EXHAUSTED",
                    "The configured shrink step budget was reached",
                ));
                return (report, evidence);
            }
            let key = serde_json::to_string(&candidate).unwrap_or_default();
            if !visited.insert(key) {
                continue;
            }
            let result = match worker
                .execute_pure_operation(ir, &plan.operation, candidate.clone(), budgets, cancel)
                .await
            {
                Ok(result) => result,
                Err(error) => {
                    evidence.push(worker.session_evidence().to_vec());
                    report
                        .issues
                        .push(issue("SHRINK_EXECUTION_INCOMPLETE", error.to_string()));
                    return (report, evidence);
                }
            };
            evidence.push(worker.session_evidence().to_vec());
            let preserves = same_failure(failure, &result);
            report.attempts.push(ShrinkAttempt {
                invocation: candidate.clone(),
                result,
            });
            if preserves {
                report.minimized = candidate;
                improved = true;
                break;
            }
        }
        if !improved {
            report.exhausted = report.issues.is_empty();
            return (report, evidence);
        }
    }
}

pub(super) async fn lifecycle(
    worker: &mut NativeWorker,
    ir: &WorkspaceIr,
    plan: &dctx0_verification::LifecyclePlan,
    path: &dctx0_verification::LifecyclePath,
    original: &LifecycleCaseInput,
    assessment: &dctx0_verification::LifecycleAssessment,
    options: (&VerificationBudgets, &NativeCancellation),
) -> (
    dctx0_verification::LifecycleShrinkReport,
    Vec<Vec<NativeSessionEvidence>>,
) {
    use dctx0_verification::{
        LifecycleShrinkAttempt, LifecycleShrinkReport, shrink_lifecycle_inputs,
    };
    let (budgets, cancel) = options;
    let mut report = LifecycleShrinkReport {
        version: SHRINK_VERSION.into(),
        original: original.clone(),
        original_assessment: assessment.clone(),
        minimized: original.clone(),
        attempts: vec![],
        exhausted: false,
        issues: vec![],
    };
    let mut evidence = Vec::new();
    let mut visited = BTreeSet::new();
    if !matches!(assessment.result, VerificationResult::Fail { .. }) {
        report.issues.push(issue(
            "SHRINK_FAILURE_REQUIRED",
            "Lifecycle shrinking requires an observed failing assessment",
        ));
        return (report, evidence);
    }
    loop {
        if report.attempts.len() >= budgets.shrink_steps {
            report.issues.push(issue(
                "SHRINK_BUDGET_EXHAUSTED",
                "The configured lifecycle shrink step budget was reached",
            ));
            return (report, evidence);
        }
        let (candidates, issues) =
            shrink_lifecycle_inputs(ir, plan, path, &report.minimized, budgets);
        report.issues.extend(issues);
        let mut improved = false;
        for candidate in candidates {
            if report.attempts.len() >= budgets.shrink_steps {
                report.issues.push(issue(
                    "SHRINK_BUDGET_EXHAUSTED",
                    "The configured lifecycle shrink step budget was reached",
                ));
                return (report, evidence);
            }
            if !visited.insert(serde_json::to_string(&candidate).unwrap_or_default()) {
                continue;
            }
            let observed = match worker
                .execute_lifecycle(ir, plan, path, &candidate, budgets, cancel)
                .await
            {
                Ok(observed) => observed,
                Err(error) => {
                    evidence.push(worker.session_evidence().to_vec());
                    report
                        .issues
                        .push(issue("SHRINK_EXECUTION_INCOMPLETE", error.to_string()));
                    return (report, evidence);
                }
            };
            evidence.push(worker.session_evidence().to_vec());
            let preserves = same_failure(&assessment.result, &observed.assessment.result)
                && assessment.completed_transitions == observed.assessment.completed_transitions
                && assessment.reached_states == observed.assessment.reached_states;
            report.attempts.push(LifecycleShrinkAttempt {
                input: candidate.clone(),
                assessment: observed.assessment,
            });
            if preserves {
                report.minimized = candidate;
                improved = true;
                break;
            }
        }
        if !improved {
            report.exhausted = report.issues.is_empty();
            return (report, evidence);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn native_step_limits_preserve_only_verified_pure_and_lifecycle_improvements() {
        let compiled = dctx0_language::compile(&[Source {
            file: "shrink-step-budget.dctx0".into(),
            text: "workspace W { type Status = Open | Closed\nentity Item { id:String\nstatus:Status\nkey id } surface Api { operation calculate(input:Int) -> Int { pure\nensures result == input } operation prepare(id:String,note:String) -> Unit { requires note == \"abcdefgh\" or note == \"abcd\" } operation finish(id:String,note:String) -> Int } lifecycle Flow for Item using status { state Open initial\nstate Closed terminal\ntransition Api.finish from Open to Closed when true } }".into(),
        }]);
        assert!(
            compiled.diagnostics.is_empty(),
            "{:?}",
            compiled.diagnostics
        );
        let ir = compiled.ir.unwrap();
        let plan = dctx0_verification::project_program(
            &ir,
            "pinned",
            1,
            dctx0_verification::VerificationScope {
                subjects: ["W".into()].into(),
                tasks: Default::default(),
            },
            &Default::default(),
            Default::default(),
        );
        let program = r#"import sys,json
present=False
for line in sys.stdin:
 p=json.loads(line);q=p['request'];kind=q['kind']
 if kind=='begin':
  present=False;r={'kind':'begun','case_id':q['case_id']}
 elif kind=='invoke':
  i=q['invocation'];args=i['arguments']
  if i['subject'].endswith('prepare'):present=True;value={'kind':'unit'}
  elif i['subject'].endswith('calculate'):
   number=int(args[0]['value']);value={'kind':'integer','value':str(number+1 if number>0 else number)}
  else:value={'kind':'integer','value':str(len(args[1]['value']))}
  r={'kind':'invoked','observation':{'kind':'returned','value':value},'events':[]}
 elif kind=='observe':r={'kind':'observed','states':{'W.Item':[{'kind':'record','value':{'id':{'kind':'string','value':'chosen'},'status':{'kind':'variant','value':{'declaration':'W.Status','name':'Open','payload':None}}}}] if present else []}}
 elif kind=='end':r={'kind':'ended'}
 elif kind=='shutdown':r={'kind':'bye'}
 else:raise Exception('Unexpected request')
 print(json.dumps({'protocol_version':1,'message':{'kind':'reply','id':p['id'],'reply':r}}),flush=True)
 if kind=='shutdown':break
"#;
        let recipe = ExecutionRecipe {
            program: "python3".into(),
            arguments: vec!["-u".into(), "-c".into(), program.into()],
        };
        let directory = std::env::current_dir().unwrap();
        let cancel = NativeCancellation::default();
        let budgets = VerificationBudgets {
            shrink_steps: 3,
            ..Default::default()
        };
        let operation = plan
            .operations
            .iter()
            .find(|operation| operation.operation.id == "W.Api.calculate")
            .unwrap();
        let original = Invocation {
            subject: operation.operation.id.clone(),
            arguments: vec![VerificationValue::Integer("10".into())],
        };
        let mut worker = NativeWorker::spawn(&recipe, &directory, NativeLimits::default())
            .await
            .unwrap();
        let failure = worker
            .execute_pure_operation(
                &ir,
                &operation.operation,
                original.clone(),
                &budgets,
                &cancel,
            )
            .await
            .unwrap();
        assert!(matches!(failure, VerificationResult::Fail { .. }));
        let (report, evidence) = pure(
            &mut worker,
            &ir,
            operation,
            original,
            &failure,
            &budgets,
            &cancel,
        )
        .await;
        assert_eq!(report.attempts.len(), 3);
        assert_eq!(evidence.len(), 3);
        assert_eq!(
            report.minimized.arguments,
            [VerificationValue::Integer("2".into())]
        );
        assert!(!report.exhausted);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "SHRINK_BUDGET_EXHAUSTED")
        );
        worker.abort().await.unwrap();
        let lifecycle_plan = &plan.lifecycles[0];
        let path = &lifecycle_plan.paths[0];
        let input = LifecycleCaseInput {
            keys: vec![VerificationValue::String("chosen".into())],
            setup: vec![Invocation {
                subject: "W.Api.prepare".into(),
                arguments: vec![
                    VerificationValue::String("chosen".into()),
                    VerificationValue::String("abcdefgh".into()),
                ],
            }],
            arguments: vec![vec![
                VerificationValue::String("chosen".into()),
                VerificationValue::String("long note".into()),
            ]],
        };
        let budgets = VerificationBudgets {
            shrink_steps: 4,
            ..Default::default()
        };
        let mut worker = NativeWorker::spawn(&recipe, &directory, NativeLimits::default())
            .await
            .unwrap();
        let failure = worker
            .execute_lifecycle(&ir, lifecycle_plan, path, &input, &budgets, &cancel)
            .await
            .unwrap();
        assert!(matches!(
            failure.assessment.result,
            VerificationResult::Fail { .. }
        ));
        let (report, evidence) = lifecycle(
            &mut worker,
            &ir,
            lifecycle_plan,
            path,
            &input,
            &failure.assessment,
            (&budgets, &cancel),
        )
        .await;
        assert_eq!(report.attempts.len(), 4);
        assert_eq!(evidence.len(), 4);
        assert_eq!(report.minimized.keys, input.keys);
        assert_eq!(
            report.minimized.setup[0].arguments[1],
            VerificationValue::String("abcd".into())
        );
        assert!(!report.exhausted);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "SHRINK_BUDGET_EXHAUSTED")
        );
        worker.abort().await.unwrap();
    }

    #[tokio::test]
    async fn native_failure_during_shrinking_retains_original_counterexamples_without_exhaustion() {
        let compiled = dctx0_language::compile(&[Source {
            file: "shrink-interruption.dctx0".into(),
            text: "workspace W { type Status = Open | Closed\nentity Item { id:String\nstatus:Status\nkey id } surface Api { operation calculate(input:Int) -> Int { pure\nensures result == input } operation finish(id:String,note:String) -> Int } lifecycle Flow for Item using status { state Open initial\nstate Closed terminal\ntransition Api.finish from Open to Closed when true } }".into(),
        }]);
        assert!(
            compiled.diagnostics.is_empty(),
            "{:?}",
            compiled.diagnostics
        );
        let ir = compiled.ir.unwrap();
        let plan = dctx0_verification::project_program(
            &ir,
            "pinned",
            1,
            dctx0_verification::VerificationScope {
                subjects: ["W".into()].into(),
                tasks: Default::default(),
            },
            &Default::default(),
            Default::default(),
        );
        // This protocol fixture computes actual results and exposes actual local
        // state. Its second session fails technically; Rust assesses the first.
        let program = r#"import sys,json
begins=0
for line in sys.stdin:
 p=json.loads(line); q=p['request']; kind=q['kind']
 if kind=='begin':
  begins+=1
  r={'kind':'begun','case_id':q['case_id']} if begins==1 else {'kind':'error','category':'technical','message':'Second session unavailable'}
 elif kind=='invoke':
  i=q['invocation']; args=i['arguments']; value=int(args[0]['value'])+1 if i['subject'].endswith('calculate') else len(args[1]['value'])
  r={'kind':'invoked','observation':{'kind':'returned','value':{'kind':'integer','value':str(value)}},'events':[]}
 elif kind=='observe':
  r={'kind':'observed','states':{'W.Item':[{'kind':'record','value':{'id':{'kind':'string','value':'chosen'},'status':{'kind':'variant','value':{'declaration':'W.Status','name':'Open','payload':None}}}}]}}
 elif kind=='end':r={'kind':'ended'}
 elif kind=='shutdown':r={'kind':'bye'}
 else:raise Exception('Unexpected request')
 print(json.dumps({'protocol_version':1,'message':{'kind':'reply','id':p['id'],'reply':r}}),flush=True)
 if kind=='shutdown':break
"#;
        let recipe = ExecutionRecipe {
            program: "python3".into(),
            arguments: vec!["-u".into(), "-c".into(), program.into()],
        };
        let cancel = NativeCancellation::default();
        let budgets = VerificationBudgets::default();
        let directory = std::env::current_dir().unwrap();
        let operation = plan
            .operations
            .iter()
            .find(|operation| operation.operation.id == "W.Api.calculate")
            .unwrap();
        let original = Invocation {
            subject: operation.operation.id.clone(),
            arguments: vec![VerificationValue::Integer("10".into())],
        };
        let mut worker = NativeWorker::spawn(&recipe, &directory, NativeLimits::default())
            .await
            .unwrap();
        let failure = worker
            .execute_pure_operation(
                &ir,
                &operation.operation,
                original.clone(),
                &budgets,
                &cancel,
            )
            .await
            .unwrap();
        assert!(matches!(failure, VerificationResult::Fail { .. }));
        let (shrunk, evidence) = pure(
            &mut worker,
            &ir,
            operation,
            original.clone(),
            &failure,
            &budgets,
            &cancel,
        )
        .await;
        assert_eq!(shrunk.minimized, original);
        assert!(shrunk.attempts.is_empty());
        assert!(!shrunk.exhausted);
        assert!(
            shrunk
                .issues
                .iter()
                .any(|issue| issue.code == "SHRINK_EXECUTION_INCOMPLETE")
        );
        assert_eq!(evidence.len(), 1);
        worker.abort().await.unwrap();
        let lifecycle_plan = &plan.lifecycles[0];
        let path = &lifecycle_plan.paths[0];
        let input = LifecycleCaseInput {
            keys: vec![VerificationValue::String("chosen".into())],
            setup: vec![],
            arguments: vec![vec![
                VerificationValue::String("chosen".into()),
                VerificationValue::String("long note".into()),
            ]],
        };
        let mut worker = NativeWorker::spawn(&recipe, &directory, NativeLimits::default())
            .await
            .unwrap();
        let failed = worker
            .execute_lifecycle(&ir, lifecycle_plan, path, &input, &budgets, &cancel)
            .await
            .unwrap();
        assert!(matches!(
            failed.assessment.result,
            VerificationResult::Fail { .. }
        ));
        let (shrunk, evidence) = lifecycle(
            &mut worker,
            &ir,
            lifecycle_plan,
            path,
            &input,
            &failed.assessment,
            (&budgets, &cancel),
        )
        .await;
        assert_eq!(shrunk.minimized, input);
        assert!(shrunk.attempts.is_empty());
        assert!(!shrunk.exhausted);
        assert!(
            shrunk
                .issues
                .iter()
                .any(|issue| issue.code == "SHRINK_EXECUTION_INCOMPLETE")
        );
        assert_eq!(evidence.len(), 1);
        worker.abort().await.unwrap();
    }

    #[tokio::test]
    async fn pure_shrinking_requires_a_failure_and_disabled_budgets_invoke_nothing() {
        let compiled = dctx0_language::compile(&[Source {
            file: "shrink-guards.dctx0".into(),
            text: "workspace W { surface Api { operation calculate(input:Int) -> Int { pure } } }"
                .into(),
        }]);
        let ir = compiled.ir.unwrap();
        let plan = dctx0_verification::project_program(
            &ir,
            "pinned",
            1,
            dctx0_verification::VerificationScope {
                subjects: ["W.Api.calculate".into()].into(),
                tasks: Default::default(),
            },
            &Default::default(),
            Default::default(),
        )
        .operations
        .remove(0);
        let original = Invocation {
            subject: plan.operation.id.clone(),
            arguments: vec![VerificationValue::Integer("10".into())],
        };
        let recipe = dctx0_target_sdk::ExecutionRecipe {
            program: "python3".into(),
            arguments: vec!["-u".into(), "-c".into(),
                "import sys,json\nfor line in sys.stdin:\n p=json.loads(line)\n assert p['request']['kind']=='shutdown', 'Unexpected execution during shrink guard'\n print(json.dumps({'protocol_version':1,'message':{'kind':'reply','id':p['id'],'reply':{'kind':'bye'}}}),flush=True)\n break".into()],
        };
        let mut worker = NativeWorker::spawn(
            &recipe,
            &std::env::current_dir().unwrap(),
            NativeLimits::default(),
        )
        .await
        .unwrap();
        let cancel = NativeCancellation::default();
        for failure in [
            VerificationResult::Pass,
            unknown("NO_OBSERVATION", "Not executed"),
        ] {
            let (report, evidence) = pure(
                &mut worker,
                &ir,
                &plan,
                original.clone(),
                &failure,
                &Default::default(),
                &cancel,
            )
            .await;
            assert_eq!(report.minimized, original);
            assert!(report.attempts.is_empty() && evidence.is_empty());
            assert!(!report.exhausted);
            assert_eq!(report.issues.len(), 1);
            assert_eq!(report.issues[0].code, "SHRINK_FAILURE_REQUIRED");
        }
        let failure = VerificationResult::Fail {
            issue: issue("ACTUAL_FAILURE", "Observed failure"),
        };
        let (report, evidence) = pure(
            &mut worker,
            &ir,
            &plan,
            original.clone(),
            &failure,
            &VerificationBudgets {
                shrink_steps: 0,
                ..Default::default()
            },
            &cancel,
        )
        .await;
        assert_eq!(report.minimized, original);
        assert!(report.attempts.is_empty() && evidence.is_empty());
        assert!(!report.exhausted);
        assert_eq!(report.issues[0].code, "SHRINK_BUDGET_EXHAUSTED");
        let compiled = dctx0_language::compile(&[Source {
            file: "lifecycle-shrink-guards.dctx0".into(),
            text: "workspace W { type Status = Open | Closed\nentity Item { id:String\nstatus:Status\nkey id } surface Api { operation finish(id:String) -> Int } lifecycle Flow for Item using status { state Open initial\nstate Closed terminal\ntransition Api.finish from Open to Closed when true } }".into(),
        }]);
        assert!(
            compiled.diagnostics.is_empty(),
            "{:?}",
            compiled.diagnostics
        );
        let ir = compiled.ir.unwrap();
        let plan = dctx0_verification::project_program(
            &ir,
            "pinned",
            1,
            dctx0_verification::VerificationScope {
                subjects: ["W.Flow".into()].into(),
                tasks: Default::default(),
            },
            &Default::default(),
            Default::default(),
        )
        .lifecycles
        .remove(0);
        let path = &plan.paths[0];
        let input = LifecycleCaseInput {
            keys: vec![VerificationValue::String("chosen".into())],
            setup: vec![],
            arguments: vec![vec![VerificationValue::String("chosen".into())]],
        };
        for result in [
            VerificationResult::Pass,
            unknown("NO_OBSERVATION", "Not executed"),
            failure,
        ] {
            let failed = matches!(result, VerificationResult::Fail { .. });
            let assessment = dctx0_verification::LifecycleAssessment {
                case_id: path.id.clone(),
                result,
                reached_states: vec![],
                completed_transitions: vec![],
                open_transitions: vec![],
            };
            let budgets = VerificationBudgets {
                shrink_steps: if failed { 0 } else { 100 },
                ..Default::default()
            };
            let (report, evidence) = lifecycle(
                &mut worker,
                &ir,
                &plan,
                path,
                &input,
                &assessment,
                (&budgets, &cancel),
            )
            .await;
            assert_eq!(report.minimized, input);
            assert!(report.attempts.is_empty() && evidence.is_empty());
            assert!(!report.exhausted);
            assert_eq!(report.issues.len(), 1);
            assert_eq!(
                report.issues[0].code,
                if failed {
                    "SHRINK_BUDGET_EXHAUSTED"
                } else {
                    "SHRINK_FAILURE_REQUIRED"
                }
            );
        }
        assert_eq!(
            worker
                .request(WorkerRequest::Shutdown, &cancel, &mut |_| Ok(
                    Observation::Unavailable {
                        reason: "No collaborator".into()
                    }
                ))
                .await
                .unwrap(),
            WorkerReply::Bye
        );
        assert_eq!(worker.finish().await.unwrap().exit_code, Some(0));
    }
}
