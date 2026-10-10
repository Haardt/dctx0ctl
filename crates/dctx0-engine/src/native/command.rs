//! Build/test tools use bounded logs, distinct from the JSON-Lines worker protocol.
use super::*;
pub struct NativeCommand {
    pub(super) worker: NativeWorker,
}
impl NativeCommand {
    pub async fn spawn(
        recipe: &ExecutionRecipe,
        directory: &Path,
        limits: NativeLimits,
    ) -> Result<Self, NativeFailure> {
        #[cfg(target_os = "linux")]
        {
            Ok(Self {
                worker: NativeWorker::spawn_streams(recipe, directory, limits, false).await?,
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (recipe, directory, limits);
            Err(NativeFailure::new(
                NativeFailureKind::Unsupported,
                "Native commands require Linux process-group ownership",
            ))
        }
    }
    pub fn pid(&self) -> u32 {
        self.worker.pid()
    }
    pub async fn wait(
        &mut self,
        cancel: &NativeCancellation,
    ) -> Result<NativeAudit, NativeFailure> {
        if self.worker.child.is_none() {
            return Err(NativeFailure::new(
                NativeFailureKind::WorkerExit,
                "Native command already closed",
            ));
        }
        self.worker.input.take();
        let mut faults = self.worker.fault.clone();
        let stream_fault = async {
            loop {
                if let Some(error) = faults.borrow().clone() {
                    return Err(error);
                }
                if faults.changed().await.is_err() {
                    return std::future::pending::<Result<(), NativeFailure>>().await;
                }
            }
        };
        let result = tokio::select! {
            biased;
            _=cancel.cancelled()=>Err(NativeFailure::new(NativeFailureKind::Cancelled,"Native command cancelled")),
            error=stream_fault=>error,
            result=wait_exit(self.worker.pid,self.worker.deadline.saturating_duration_since(Instant::now()))=>result,
        };
        if let Err(mut error) = result {
            if let Err(cleanup) = self.worker.abort().await {
                error.cleanup_error = Some(cleanup.to_string());
            }
            return Err(error);
        }
        self.worker.finish().await.map_err(|mut error| {
            if error.kind == NativeFailureKind::WorkerExit {
                error.kind = NativeFailureKind::Build;
                error.detail = format!(
                    "Native command exited unsuccessfully: {}",
                    self.worker.audit().stderr
                );
            }
            error
        })
    }
}
