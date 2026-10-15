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
        Ok(Self {
            worker: NativeWorker::spawn_streams(recipe, directory, limits, false).await?,
        })
    }

    pub async fn write_input(&mut self, bytes: &[u8]) -> Result<(), NativeFailure> {
        if bytes.len() > self.worker.limits.message_bytes {
            return Err(NativeFailure::new(
                NativeFailureKind::MessageLimit,
                "Command input exceeds message budget",
            ));
        }
        let input = self.worker.input.as_mut().ok_or_else(|| {
            NativeFailure::new(NativeFailureKind::WorkerExit, "Command stdin closed")
        })?;
        input.write_all(bytes).await.map_err(NativeFailure::io)?;
        input.flush().await.map_err(NativeFailure::io)
    }
    pub fn pid(&self) -> u32 {
        self.worker.pid()
    }
    pub async fn wait(
        &mut self,
        cancel: &NativeCancellation,
    ) -> Result<NativeAudit, NativeFailure> {
        let audit = self.wait_status(cancel).await?;
        if audit.exit_code != Some(0) {
            return Err(NativeFailure::new(
                NativeFailureKind::Build,
                format!(
                    "Native command exited {:?}: {}{}",
                    audit.exit_code, audit.stdout, audit.stderr
                ),
            ));
        }
        Ok(audit)
    }
    pub async fn wait_status(
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
            result=wait_owned(self.worker.child.as_mut().expect("owned child"),self.worker.supervised,self.worker.pid,self.worker.deadline.saturating_duration_since(Instant::now()))=>result,
        };
        if let Err(mut error) = result {
            if let Err(cleanup) = self.worker.abort().await {
                error.cleanup_error = Some(cleanup.to_string());
            }
            return Err(error);
        }
        self.worker.finish_status(false).await
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    #[tokio::test]
    async fn command_status_preserves_separate_streams_and_nonzero_exit() {
        let recipe = ExecutionRecipe {
            program: "sh".into(),
            arguments: vec![
                "-c".into(),
                "printf actual-output; printf actual-error >&2; exit 7".into(),
            ],
        };
        let mut command = NativeCommand::spawn(&recipe, Path::new("/tmp"), NativeLimits::default())
            .await
            .unwrap();
        let audit = command
            .wait_status(&NativeCancellation::default())
            .await
            .unwrap();
        assert_eq!(audit.stdout, "actual-output");
        assert_eq!(audit.stderr, "actual-error");
        assert_eq!(audit.exit_code, Some(7));
        assert!(
            command
                .wait_status(&NativeCancellation::default())
                .await
                .is_err()
        );
    }
    #[tokio::test]
    async fn cancellation_reaps_descendants_that_create_independent_groups() {
        let root =
            std::env::temp_dir().join(format!("dctx0-nested-ownership-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let recipe = ExecutionRecipe {
            program: "sh".into(),
            arguments: vec![
                "-c".into(),
                "setsid sh -c 'echo $$ > child.pid; sleep 100' & wait".into(),
            ],
        };
        let mut command = NativeCommand::spawn(&recipe, &root, NativeLimits::default())
            .await
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !root.join("child.pid").exists() && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let nested = std::fs::read_to_string(root.join("child.pid"))
            .unwrap()
            .trim()
            .parse::<u32>()
            .unwrap();
        tokio::time::sleep(Duration::from_millis(30)).await;
        let cancellation = NativeCancellation::default();
        cancellation.cancel();
        assert_eq!(
            command.wait_status(&cancellation).await.unwrap_err().kind,
            NativeFailureKind::Cancelled
        );
        assert!(!Path::new(&format!("/proc/{nested}")).exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(all(test, target_os = "linux"))]
mod input_tests {
    use super::*;
    #[tokio::test]
    async fn bounded_command_input_round_trips_and_rejects_oversize_and_closed_pipes() {
        let recipe = ExecutionRecipe {
            program: "cat".into(),
            arguments: vec![],
        };
        let mut command = NativeCommand::spawn(
            &recipe,
            Path::new("/tmp"),
            NativeLimits {
                message_bytes: 8,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert!(Path::new(&format!("/proc/{}", command.pid())).exists());
        assert_eq!(
            command.write_input(b"123456789").await.unwrap_err().kind,
            NativeFailureKind::MessageLimit
        );
        command.write_input(b"actual").await.unwrap();
        let audit = command
            .wait_status(&NativeCancellation::default())
            .await
            .unwrap();
        assert_eq!(audit.stdout, "actual");
        assert_eq!(audit.exit_code, Some(0));
        assert_eq!(
            command.write_input(b"").await.unwrap_err().kind,
            NativeFailureKind::WorkerExit
        );
    }
}
