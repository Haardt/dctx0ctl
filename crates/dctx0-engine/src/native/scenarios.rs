//! Native observations drive the pure central example state machine.
use super::*;
use dctx0_core::{VerificationBudgets, WorkspaceIr};
use dctx0_verification::{ExamplePlan, Scenario, ScenarioEvidence, ScenarioStep};
impl NativeWorker {
    pub async fn execute_example(
        &mut self,
        ir: &WorkspaceIr,
        example: &ExamplePlan,
        budgets: &VerificationBudgets,
        cancellation: &NativeCancellation,
    ) -> Result<ScenarioEvidence, NativeFailure> {
        self.clear_evidence();
        match tokio::time::timeout(
            self.limits.case,
            self.example_session(ir, example, budgets, cancellation),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => Err(self.expired_case().await),
        }
    }
    async fn example_session(
        &mut self,
        ir: &WorkspaceIr,
        example: &ExamplePlan,
        budgets: &VerificationBudgets,
        cancellation: &NativeCancellation,
    ) -> Result<ScenarioEvidence, NativeFailure> {
        self.begin_case(&example.subject, cancellation).await?;
        let mut scenario = Scenario::new(ir, example, budgets);
        let result = loop {
            match scenario.advance() {
                ScenarioStep::Complete(result) => break *result,
                ScenarioStep::Invoke(invocation) => {
                    let session::Invoked {
                        observation,
                        events,
                        calls,
                        ..
                    } = self
                        .invoke_case(invocation, cancellation, &mut |call| {
                            Ok(scenario.collaborator(call))
                        })
                        .await?;
                    scenario.deliver_observed(observation, events, calls);
                }
            }
        };
        self.end_case(cancellation).await?;
        Ok(result)
    }
}
