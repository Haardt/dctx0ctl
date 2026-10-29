//! Managed native transport with optional Linux process-tree supervision. No worker reply constitutes verification PASS.
mod descendants;
pub(crate) mod identity;
pub(crate) use identity::VerifierIdentity;
pub use identity::{NativeLaunchInputs, NativeRuntimeIdentity};
mod cases;
mod command;
mod lifecycles;
pub use command::*;
mod contracts;
mod operations;
pub use contracts::NativeOperationAssessment;
mod scenarios;
mod session;
mod values;
pub use lifecycles::*;
pub use session::NativeSessionEvidence;
mod streams;
pub use cases::*;
use dctx0_core::{Invocation, Observation};
use dctx0_target_sdk::*;
use serde::{Deserialize, Serialize};
#[cfg(target_os = "linux")]
use std::os::unix::process::CommandExt;
use std::{path::Path, process::Stdio, time::Duration};
use tokio::{
    io::AsyncWriteExt,
    process::{Child, ChildStdin, Command},
    sync::{mpsc, watch},
    task::JoinHandle,
    time::Instant,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NativeFailureKind {
    Unsupported,
    InvalidLimits,
    Unavailable,
    Io,
    Protocol,
    Timeout,
    Cancelled,
    LogLimit,
    MessageLimit,
    WorkerExit,
    Technical,
    Environment,
    Adapter,
    Cleanup,
    Build,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("{kind:?}: {detail}")]
pub struct NativeFailure {
    pub kind: NativeFailureKind,
    pub detail: String,
    pub cleanup_error: Option<String>,
}
impl NativeFailure {
    fn new(kind: NativeFailureKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
            cleanup_error: None,
        }
    }
    fn io(error: std::io::Error) -> Self {
        Self::new(NativeFailureKind::Io, error.to_string())
    }
}
#[derive(Debug, Clone)]
pub struct NativeCancellation {
    signal: watch::Sender<bool>,
}
impl Default for NativeCancellation {
    fn default() -> Self {
        Self {
            signal: watch::channel(false).0,
        }
    }
}
impl NativeCancellation {
    pub fn cancel(&self) {
        self.signal.send_replace(true);
    }
    pub(crate) async fn cancelled(&self) {
        let mut receiver = self.signal.subscribe();
        let _ = receiver.wait_for(|value| *value).await;
    }
}
/// Auto retains Linux process-tree ownership; Child manages only the direct child.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NativeSupervision {
    #[default]
    Auto,
    Child,
}
#[derive(Debug, Clone, Serialize)]
pub struct NativeLimits {
    pub supervision: NativeSupervision,
    pub initialization: Duration,
    pub case: Duration,
    pub run: Duration,
    pub cleanup: Duration,
    pub log_bytes: usize,
    pub message_bytes: usize,
    pub evidence_bytes: usize,
    pub evidence_entries: usize,
}
impl Default for NativeLimits {
    fn default() -> Self {
        Self {
            supervision: NativeSupervision::Auto,
            initialization: Duration::from_secs(30),
            case: Duration::from_secs(5),
            run: Duration::from_secs(120),
            cleanup: Duration::from_secs(5),
            log_bytes: 262144,
            message_bytes: DEFAULT_MESSAGE_BYTES,
            evidence_bytes: 4 * DEFAULT_MESSAGE_BYTES,
            evidence_entries: 10_000,
        }
    }
}
impl NativeLimits {
    pub fn supervision_capabilities(&self) -> NativeSupervisionCapabilities {
        let process_tree = cfg!(target_os = "linux") && self.supervision == NativeSupervision::Auto;
        NativeSupervisionCapabilities {
            process_tree,
            detached_descendants: process_tree,
            collective_memory_limit: false,
        }
    }
    pub(crate) fn validate(&self) -> Result<(), NativeFailure> {
        if [self.initialization, self.case, self.run, self.cleanup]
            .iter()
            .any(|duration| duration.is_zero() || Instant::now().checked_add(*duration).is_none())
            || self.log_bytes == 0
            || self.message_bytes == 0
            || self.message_bytes > MAX_MESSAGE_BYTES
            || self.evidence_bytes == 0
            || self.evidence_bytes > 4 * MAX_MESSAGE_BYTES
            || self.evidence_entries == 0
        {
            return Err(NativeFailure::new(
                NativeFailureKind::InvalidLimits,
                "Native deadlines and size limits must be nonzero; messages cannot exceed the SDK maximum",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeSupervisionCapabilities {
    pub process_tree: bool,
    pub detached_descendants: bool,
    pub collective_memory_limit: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeAudit {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supervision: Option<NativeSupervisionCapabilities>,
    pub pid: u32,
    pub stderr: String,
    #[serde(default)]
    pub stdout: String,
    pub received_messages: usize,
    pub exit_code: Option<i32>,
}
/// Call `finish` or `abort` explicitly. Drop also kills the owned process group.
pub struct NativeWorker {
    child: Option<Child>,
    supervised: bool,
    launch_identity: NativeRuntimeIdentity,
    group_active: bool,
    descendants: descendants::Descendants,
    groups: std::sync::Arc<dyn GroupControl>,
    input: Option<ChildStdin>,
    pid: u32,
    next_id: u64,
    limits: NativeLimits,
    deadline: Instant,
    frames: mpsc::Receiver<Vec<u8>>,
    fault: watch::Receiver<Option<NativeFailure>>,
    tasks: Vec<JoinHandle<()>>,
    logs: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    stdout: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    received: usize,
    session_evidence: Vec<NativeSessionEvidence>,
    evidence_bytes: usize,
    scoped_state_observation: bool,
}
pub type NativeMockHandler<'a> =
    dyn FnMut(&Invocation) -> Result<Observation, NativeFailure> + Send + 'a;
impl NativeWorker {
    pub fn supported() -> bool {
        true
    }
    async fn expired_case(&mut self) -> NativeFailure {
        let mut error = NativeFailure::new(
            NativeFailureKind::Timeout,
            "Whole native case deadline exceeded",
        );
        if let Err(cleanup) = self.abort().await {
            error.cleanup_error = Some(cleanup.to_string());
        }
        error
    }
    pub async fn spawn(
        recipe: &ExecutionRecipe,
        directory: &Path,
        limits: NativeLimits,
    ) -> Result<Self, NativeFailure> {
        Self::spawn_streams(recipe, directory, limits, true).await
    }
    async fn spawn_streams(
        recipe: &ExecutionRecipe,
        directory: &Path,
        limits: NativeLimits,
        protocol: bool,
    ) -> Result<Self, NativeFailure> {
        limits.validate()?;
        let supervised = limits.supervision_capabilities().process_tree;
        #[cfg(target_os = "linux")]
        if supervised {
            subreaper_result(rustix::process::set_child_subreaper(Some(
                rustix::process::Pid::INIT,
            )))?;
        }
        let program = identity::resolve_program(&recipe.program, directory)?;
        let mut command = Command::new(&program);
        if recipe.program == "cargo" {
            command
                .env("CARGO_NET_OFFLINE", "true")
                .env("CARGO_BUILD_JOBS", "2");
            for name in [
                "CARGO_TARGET_DIR",
                "RUSTC_WRAPPER",
                "RUSTC_WORKSPACE_WRAPPER",
                "RUSTFLAGS",
                "CARGO_ENCODED_RUSTFLAGS",
            ] {
                command.env_remove(name);
            }
            command
                .env("CARGO_PROFILE_DEV_DEBUG", "0")
                .env("CARGO_PROFILE_DEV_INCREMENTAL", "false");
        }
        if recipe.program == "go" {
            command
                .env("GOTOOLCHAIN", "local")
                .env("GOPROXY", "off")
                .env("GOWORK", "off")
                .env("CGO_ENABLED", "0")
                .env("GOFLAGS", "-mod=readonly");
        }
        // An inherited ownership chain also identifies detached descendants after
        // their original parent exits, when /proc's PPid relationship is lost.
        let owner = format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|error| NativeFailure::new(
                    NativeFailureKind::Technical,
                    error.to_string()
                ))?
                .as_nanos()
        );
        let ancestors = std::env::var("DCTX0_NATIVE_OWNERS").unwrap_or_default();
        command.env("DCTX0_NATIVE_OWNERS", format!("{ancestors}:{owner}"));
        command
            .args(&recipe.arguments)
            .current_dir(directory)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(target_os = "linux")]
        if supervised {
            command.as_std_mut().process_group(0);
        }
        let launch_identity =
            NativeRuntimeIdentity::capture_launch(&program, recipe, directory, &mut command)?;
        let mut child = command
            .spawn()
            .map_err(|e| NativeFailure::new(NativeFailureKind::Unavailable, e.to_string()))?;
        let pid = child.id().expect("newly spawned child has a positive PID");
        let input = child.stdin.take().expect("stdin was piped");
        let stdout = child.stdout.take().expect("stdout was piped");
        let stderr = child.stderr.take().expect("stderr was piped");
        let (send, frames) = mpsc::channel(2);
        let (fault_send, fault) = watch::channel(None);
        let logs = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let stdout_logs = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let output = if protocol {
            tokio::spawn(streams::protocol(
                stdout,
                send,
                fault_send.clone(),
                limits.message_bytes,
            ))
        } else {
            drop(send);
            tokio::spawn(streams::logs(
                stdout,
                stdout_logs.clone(),
                fault_send.clone(),
                limits.log_bytes,
            ))
        };
        let tasks = vec![
            output,
            tokio::spawn(streams::logs(
                stderr,
                logs.clone(),
                fault_send,
                limits.log_bytes,
            )),
        ];
        let deadline = Instant::now() + limits.run;
        Ok(Self {
            child: Some(child),
            supervised,
            launch_identity,
            group_active: true,
            descendants: if supervised {
                descendants::Descendants::start(pid, owner)
            } else {
                Default::default()
            },
            groups: if supervised {
                std::sync::Arc::new(LinuxGroups)
            } else {
                std::sync::Arc::new(ChildOnly)
            },
            input: Some(input),
            pid,
            next_id: 1,
            limits,
            deadline,
            frames,
            fault,
            tasks,
            logs,
            stdout: stdout_logs,
            received: 0,
            session_evidence: Vec::new(),
            evidence_bytes: 0,
            scoped_state_observation: false,
        })
    }
    pub fn pid(&self) -> u32 {
        self.pid
    }
    pub fn audit(&self) -> NativeAudit {
        NativeAudit {
            supervision: Some(NativeSupervisionCapabilities {
                process_tree: self.supervised,
                detached_descendants: self.supervised,
                collective_memory_limit: false,
            }),
            pid: self.pid,
            stderr: String::from_utf8_lossy(
                &self
                    .logs
                    .lock()
                    .expect("reader holds no lock across awaits"),
            )
            .into_owned(),
            stdout: String::from_utf8_lossy(&self.stdout.lock().expect("stdout guard"))
                .into_owned(),
            received_messages: self.received,
            exit_code: None,
        }
    }
    async fn send(&mut self, id: u64, request: WorkerRequest) -> Result<(), NativeFailure> {
        let bytes = encode_request(
            &RequestEnvelope {
                protocol_version: PROTOCOL_VERSION,
                id,
                request,
            },
            self.limits.message_bytes,
        )
        .map_err(|e| NativeFailure::new(NativeFailureKind::Protocol, e.to_string()))?;
        let input = self.input.as_mut().ok_or_else(|| {
            NativeFailure::new(NativeFailureKind::WorkerExit, "Worker stdin is closed")
        })?;
        input.write_all(&bytes).await.map_err(NativeFailure::io)?;
        input.flush().await.map_err(NativeFailure::io)
    }
    async fn receive(
        &mut self,
        id: u64,
        mocks: &mut NativeMockHandler<'_>,
    ) -> Result<WorkerReply, NativeFailure> {
        let mut mock_ids = std::collections::BTreeSet::new();
        loop {
            let bytes = self.frames.recv().await.ok_or_else(|| {
                NativeFailure::new(
                    NativeFailureKind::WorkerExit,
                    "Worker exited without its mandatory reply",
                )
            })?;
            self.received += 1;
            let packet = decode_message(&bytes, self.limits.message_bytes)
                .map_err(|e| NativeFailure::new(NativeFailureKind::Protocol, e.to_string()))?;
            match packet.message {
                WorkerMessage::Reply { id: actual, reply } => {
                    if actual != id {
                        return Err(NativeFailure::new(
                            NativeFailureKind::Protocol,
                            "Reply ID does not match the pending request",
                        ));
                    }
                    if let WorkerReply::Described { description } = &reply {
                        self.scoped_state_observation = description
                            .capabilities
                            .iter()
                            .any(|capability| capability == "scoped-state-observation");
                    }
                    return Ok(reply);
                }
                WorkerMessage::MockCall {
                    request_id,
                    invocation,
                } => {
                    if !mock_ids.insert(request_id) {
                        return Err(NativeFailure::new(
                            NativeFailureKind::Protocol,
                            "Duplicate collaborator request ID",
                        ));
                    }
                    self.retain_evidence(NativeSessionEvidence::MockRequested {
                        request_id,
                        invocation: invocation.clone(),
                    })
                    .await?;
                    let observation = mocks(&invocation)?;
                    let next = self.next_id;
                    self.next_id += 1;
                    self.send(
                        next,
                        WorkerRequest::MockReply {
                            request_id,
                            observation: observation.clone(),
                        },
                    )
                    .await?;
                    self.retain_evidence(NativeSessionEvidence::MockReplied {
                        request_id,
                        observation,
                    })
                    .await?;
                }
                WorkerMessage::TechnicalError {
                    category, message, ..
                } => {
                    return Err(NativeFailure::new(
                        match category.as_str() {
                            "environment" => NativeFailureKind::Environment,
                            "adapter" => NativeFailureKind::Adapter,
                            "protocol" => NativeFailureKind::Protocol,
                            _ => NativeFailureKind::Technical,
                        },
                        format!("{category}: {message}"),
                    ));
                }
            }
        }
    }
    /// Check shared run limits even when a cached resolver answer needs no I/O.
    pub(crate) fn check_run_limits(
        &self,
        cancellation: &NativeCancellation,
    ) -> Result<(), NativeFailure> {
        if *cancellation.signal.borrow() {
            return Err(NativeFailure::new(
                NativeFailureKind::Cancelled,
                "Native execution was cancelled",
            ));
        }
        if Instant::now() >= self.deadline {
            return Err(NativeFailure::new(
                NativeFailureKind::Timeout,
                "Native run deadline exceeded",
            ));
        }
        Ok(())
    }
    pub async fn request(
        &mut self,
        request: WorkerRequest,
        cancellation: &NativeCancellation,
        mocks: &mut NativeMockHandler<'_>,
    ) -> Result<WorkerReply, NativeFailure> {
        let timeout = if matches!(
            request,
            WorkerRequest::Begin { .. } | WorkerRequest::Describe
        ) {
            self.limits.initialization
        } else {
            self.limits.case
        };
        let until = (Instant::now() + timeout).min(self.deadline);
        let id = self.next_id;
        self.next_id += 1;
        let mut fault = self.fault.clone();
        let action = async {
            self.send(id, request).await?;
            self.receive(id, mocks).await
        };
        let result = tokio::select! {
         biased;
         _=cancellation.cancelled()=>Err(NativeFailure::new(NativeFailureKind::Cancelled,"Native execution was cancelled")),
         failure=async {loop {if let Some(error)=fault.borrow().clone(){break error;}
                if fault.changed().await.is_err(){std::future::pending::<()>().await;}}}=>Err(failure),
         _=tokio::time::sleep_until(until)=>Err(NativeFailure::new(NativeFailureKind::Timeout,"Native initialization/case/run deadline exceeded")),
         result=action=>result,
        };
        match result {
            Ok(reply) => Ok(reply),
            Err(mut error) => {
                if let Err(cleanup) = self.abort().await {
                    error.cleanup_error = Some(cleanup.to_string());
                }
                Err(error)
            }
        }
    }
    pub async fn finish(&mut self) -> Result<NativeAudit, NativeFailure> {
        self.finish_status(true).await
    }
    async fn finish_status(&mut self, require_success: bool) -> Result<NativeAudit, NativeFailure> {
        self.input.take();
        let child = self.child.as_mut().ok_or_else(|| {
            NativeFailure::new(NativeFailureKind::WorkerExit, "Worker already closed")
        })?;
        if let Err(error) = wait_owned(child, self.supervised, self.pid, self.limits.cleanup).await
        {
            self.abort().await?;
            return Err(error);
        }
        self.descendants.kill(self.pid)?;
        self.groups.kill(self.pid)?;
        let status = child.wait().await.map_err(NativeFailure::io)?;
        reap_group_with(self.pid, self.limits.cleanup, self.groups.as_ref()).await?;
        self.descendants.reap(self.limits.cleanup).await?;
        self.group_active = false;
        self.child.take();
        let drain = async {
            let mut extra = false;
            while self.frames.recv().await.is_some() {
                extra = true;
            }
            for task in &mut self.tasks {
                let _ = task.await;
            }
            extra
        };
        let extra = match tokio::time::timeout(self.limits.cleanup, drain).await {
            Ok(extra) => extra,
            Err(_) => {
                for task in &self.tasks {
                    task.abort();
                }
                self.tasks.clear();
                return Err(NativeFailure::new(
                    NativeFailureKind::Cleanup,
                    "Worker streams remained open after managed-group cleanup",
                ));
            }
        };
        self.tasks.clear();
        if extra {
            return Err(NativeFailure::new(
                NativeFailureKind::Protocol,
                "Unsolicited messages after shutdown",
            ));
        }
        if let Some(error) = self.fault.borrow().clone() {
            return Err(error);
        }
        let mut audit = self.audit();
        audit.exit_code = status.code();
        if require_success && !status.success() {
            return Err(NativeFailure::new(
                NativeFailureKind::WorkerExit,
                format!("Native worker exited with {status}"),
            ));
        }
        Ok(audit)
    }
    pub async fn abort(&mut self) -> Result<(), NativeFailure> {
        if !self.group_active {
            return Ok(());
        }
        self.input.take();
        self.descendants.kill(self.pid)?;
        self.groups.kill(self.pid)?;
        if let Some(child) = self.child.as_mut() {
            child.start_kill().map_err(NativeFailure::io)?;
            bounded_reap(child.wait(), self.limits.cleanup).await?;
        }
        self.child.take();
        reap_group_with(self.pid, self.limits.cleanup, self.groups.as_ref()).await?;
        self.descendants.reap(self.limits.cleanup).await?;
        self.group_active = false;
        for task in self.tasks.drain(..) {
            task.abort();
            let _ = task.await;
        }
        Ok(())
    }
}
trait GroupControl: Send + Sync {
    fn kill(&self, pid: u32) -> Result<(), NativeFailure>;
    fn reap(&self, pid: u32) -> Result<ReapStep, NativeFailure>;
}
struct LinuxGroups;
struct ChildOnly;
impl GroupControl for ChildOnly {
    fn kill(&self, _pid: u32) -> Result<(), NativeFailure> {
        Ok(())
    }
    fn reap(&self, _pid: u32) -> Result<ReapStep, NativeFailure> {
        Ok(ReapStep::Empty)
    }
}
enum ReapStep {
    Reaped,
    Empty,
    Pending,
}
#[cfg(target_os = "linux")]
fn signal_result(result: rustix::io::Result<()>) -> Result<(), NativeFailure> {
    match result {
        Ok(()) | Err(rustix::io::Errno::SRCH) => Ok(()),
        Err(error) => Err(NativeFailure::new(
            NativeFailureKind::Cleanup,
            error.to_string(),
        )),
    }
}
#[cfg(target_os = "linux")]
fn reap_result(
    result: rustix::io::Result<Option<(rustix::process::Pid, rustix::process::WaitStatus)>>,
) -> Result<ReapStep, NativeFailure> {
    match result {
        Ok(Some(_)) => Ok(ReapStep::Reaped),
        Err(rustix::io::Errno::CHILD) => Ok(ReapStep::Empty),
        Ok(None) => Ok(ReapStep::Pending),
        Err(error) => Err(NativeFailure::new(
            NativeFailureKind::Cleanup,
            error.to_string(),
        )),
    }
}
impl GroupControl for LinuxGroups {
    #[cfg(target_os = "linux")]
    fn kill(&self, pid: u32) -> Result<(), NativeFailure> {
        let group = rustix::process::Pid::from_raw(pid as i32).expect("PID came from OS");
        signal_result(rustix::process::kill_process_group(
            group,
            rustix::process::Signal::KILL,
        ))
    }
    #[cfg(target_os = "linux")]
    fn reap(&self, pid: u32) -> Result<ReapStep, NativeFailure> {
        let group = rustix::process::Pid::from_raw(pid as i32).expect("PID came from OS");
        reap_result(rustix::process::waitpgid(
            group,
            rustix::process::WaitOptions::NOHANG,
        ))
    }
    #[cfg(not(target_os = "linux"))]
    fn kill(&self, _pid: u32) -> Result<(), NativeFailure> {
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    fn reap(&self, _pid: u32) -> Result<ReapStep, NativeFailure> {
        Ok(ReapStep::Empty)
    }
}
async fn reap_group_with(
    pid: u32,
    timeout: Duration,
    groups: &dyn GroupControl,
) -> Result<(), NativeFailure> {
    let deadline = Instant::now() + timeout;
    loop {
        match groups.reap(pid)? {
            ReapStep::Reaped => {}
            ReapStep::Empty => return Ok(()),
            ReapStep::Pending => {
                if Instant::now() >= deadline {
                    return Err(NativeFailure::new(
                        NativeFailureKind::Cleanup,
                        "Managed descendants could not be reaped",
                    ));
                }
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        }
    }
}
async fn bounded_reap<T>(
    future: impl std::future::Future<Output = std::io::Result<T>>,
    timeout: Duration,
) -> Result<T, NativeFailure> {
    tokio::time::timeout(timeout, future)
        .await
        .map_err(|_| {
            NativeFailure::new(
                NativeFailureKind::Cleanup,
                "Native leader could not be reaped",
            )
        })?
        .map_err(NativeFailure::io)
}
impl Drop for NativeWorker {
    fn drop(&mut self) {
        if self.group_active {
            // Discover children before killing their parent. A group-only wait
            // can see ECHILD during the short interval before reparenting.
            let _ = self.descendants.kill(self.pid);
            let _ = self.groups.kill(self.pid);
            if let Some(child) = self.child.as_mut() {
                let _ = child.start_kill();
            }
            self.child.take();
            let pid = self.pid;
            let timeout = self.limits.cleanup;
            let descendants = std::mem::take(&mut self.descendants);
            #[cfg(target_os = "linux")]
            let groups = self.groups.clone();
            #[cfg(target_os = "linux")]
            std::thread::spawn(move || {
                let deadline = Instant::now() + timeout;
                loop {
                    let group_empty = matches!(groups.reap(pid), Ok(ReapStep::Empty));
                    let descendants_empty = matches!(descendants.reap_pending(), Ok(false));
                    if (group_empty && descendants_empty) || Instant::now() >= deadline {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
            });
            #[cfg(not(target_os = "linux"))]
            let _ = (pid, timeout, descendants);
            for task in self.tasks.drain(..) {
                task.abort();
            }
        }
    }
}
#[cfg(target_os = "linux")]
fn subreaper_result(result: rustix::io::Result<()>) -> Result<(), NativeFailure> {
    result.map_err(|error| NativeFailure::new(NativeFailureKind::Unavailable, error.to_string()))
}

#[cfg(target_os = "linux")]
async fn wait_exit(pid: u32, timeout: Duration) -> Result<(), NativeFailure> {
    let group = rustix::process::Pid::from_raw(pid as i32).expect("PID came from OS");
    let deadline = Instant::now() + timeout;
    loop {
        if rustix::process::waitid(
            rustix::process::WaitId::Pid(group),
            rustix::process::WaitIdOptions::EXITED
                | rustix::process::WaitIdOptions::NOHANG
                | rustix::process::WaitIdOptions::NOWAIT,
        )
        .map_err(|e| NativeFailure::new(NativeFailureKind::Cleanup, e.to_string()))?
        .is_some()
        {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(NativeFailure::new(
                NativeFailureKind::Timeout,
                "Worker did not exit after shutdown",
            ));
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}
async fn wait_owned(
    child: &mut Child,
    supervised: bool,
    pid: u32,
    timeout: Duration,
) -> Result<(), NativeFailure> {
    #[cfg(target_os = "linux")]
    if supervised {
        return wait_exit(pid, timeout).await;
    }
    let _ = (supervised, pid);
    tokio::time::timeout(timeout, child.wait())
        .await
        .map_err(|_| {
            NativeFailure::new(
                NativeFailureKind::Timeout,
                "Worker did not exit before deadline",
            )
        })?
        .map_err(NativeFailure::io)?;
    Ok(())
}

#[cfg(test)]
fn python_fixture(source: &str) -> ExecutionRecipe {
    let program = std::env::var("DCTX0_TEST_PYTHON")
        .expect("Run process tests with the runner's absolute DCTX0_TEST_PYTHON interpreter");
    assert!(Path::new(&program).is_absolute());
    ExecutionRecipe {
        program,
        arguments: vec!["-u".into(), "-c".into(), source.into()],
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    struct UnavailableGroups {
        pending: bool,
    }
    impl GroupControl for UnavailableGroups {
        fn kill(&self, _pid: u32) -> Result<(), NativeFailure> {
            Err(NativeFailure::new(
                NativeFailureKind::Cleanup,
                "Group signal denied",
            ))
        }
        fn reap(&self, _pid: u32) -> Result<ReapStep, NativeFailure> {
            if self.pending {
                Ok(ReapStep::Pending)
            } else {
                Err(NativeFailure::new(
                    NativeFailureKind::Cleanup,
                    "Group wait unavailable",
                ))
            }
        }
    }
    #[tokio::test]
    async fn kernel_cleanup_failures_and_nonresponsive_waits_remain_explicit() {
        assert!(subreaper_result(Ok(())).is_ok());
        assert_eq!(
            subreaper_result(Err(rustix::io::Errno::PERM))
                .unwrap_err()
                .kind,
            NativeFailureKind::Unavailable
        );
        assert!(signal_result(Err(rustix::io::Errno::SRCH)).is_ok());
        assert!(matches!(reap_result(Ok(None)), Ok(ReapStep::Pending)));
        assert_eq!(
            signal_result(Err(rustix::io::Errno::PERM))
                .unwrap_err()
                .kind,
            NativeFailureKind::Cleanup
        );
        assert!(matches!(
            reap_result(Err(rustix::io::Errno::INVAL)),
            Err(NativeFailure {
                kind: NativeFailureKind::Cleanup,
                ..
            })
        ));
        assert_eq!(
            reap_group_with(
                42,
                Duration::from_millis(2),
                &UnavailableGroups { pending: true }
            )
            .await
            .unwrap_err()
            .kind,
            NativeFailureKind::Cleanup
        );
        assert_eq!(
            reap_group_with(
                42,
                Duration::from_millis(2),
                &UnavailableGroups { pending: false }
            )
            .await
            .unwrap_err()
            .kind,
            NativeFailureKind::Cleanup
        );
        assert_eq!(
            bounded_reap(
                std::future::pending::<std::io::Result<()>>(),
                Duration::from_millis(2)
            )
            .await
            .unwrap_err()
            .kind,
            NativeFailureKind::Cleanup
        );
        assert_eq!(
            bounded_reap(
                async { Err::<(), _>(std::io::Error::other("wait unavailable")) },
                Duration::from_millis(2)
            )
            .await
            .unwrap_err()
            .kind,
            NativeFailureKind::Io
        );
    }
    #[tokio::test]
    async fn request_failure_preserves_failed_cleanup_and_later_owned_cleanup_can_recover() {
        let recipe = ExecutionRecipe {
            program: "python3".into(),
            arguments: vec!["-c".into(), "import time;time.sleep(300)".into()],
        };
        let mut worker = NativeWorker::spawn(&recipe, Path::new("/tmp"), NativeLimits::default())
            .await
            .unwrap();
        worker.groups = std::sync::Arc::new(UnavailableGroups { pending: false });
        let cancel = NativeCancellation::default();
        assert!(worker.check_run_limits(&cancel).is_ok());
        let deadline = worker.deadline;
        worker.deadline = Instant::now();
        assert_eq!(
            worker.check_run_limits(&cancel).unwrap_err().kind,
            NativeFailureKind::Timeout
        );
        worker.deadline = deadline;
        cancel.cancel();
        assert_eq!(
            worker.check_run_limits(&cancel).unwrap_err().kind,
            NativeFailureKind::Cancelled
        );
        let error = worker
            .request(WorkerRequest::Describe, &cancel, &mut cases::reject_mock)
            .await
            .unwrap_err();
        assert_eq!(error.kind, NativeFailureKind::Cancelled);
        assert!(error.cleanup_error.unwrap().contains("Group signal denied"));
        let expiry = worker.expired_case().await;
        assert_eq!(expiry.kind, NativeFailureKind::Timeout);
        assert!(
            expiry
                .cleanup_error
                .unwrap()
                .contains("Group signal denied")
        );
        worker.groups = std::sync::Arc::new(LinuxGroups);
        worker.abort().await.unwrap();
        assert_eq!(
            wait_exit(worker.pid, Duration::from_millis(20))
                .await
                .unwrap_err()
                .kind,
            NativeFailureKind::Cleanup
        );
    }
    #[tokio::test]
    async fn command_cancellation_keeps_cleanup_failure_and_allows_owned_recovery() {
        let mut command = NativeCommand::spawn(
            &ExecutionRecipe {
                program: "python3".into(),
                arguments: vec!["-c".into(), "import time;time.sleep(300)".into()],
            },
            Path::new("/tmp"),
            NativeLimits::default(),
        )
        .await
        .unwrap();
        command.worker.groups = std::sync::Arc::new(UnavailableGroups { pending: false });
        let cancel = NativeCancellation::default();
        cancel.cancel();
        let error = command.wait(&cancel).await.unwrap_err();
        assert_eq!(error.kind, NativeFailureKind::Cancelled);
        assert!(error.cleanup_error.unwrap().contains("Group signal denied"));
        command.worker.groups = std::sync::Arc::new(LinuxGroups);
        command.worker.abort().await.unwrap();
    }

    #[tokio::test]
    async fn request_encoding_limits_abort_and_reap_the_actual_worker() {
        let recipe = ExecutionRecipe {
            program: "python3".into(),
            arguments: vec!["-c".into(), "import time;time.sleep(300)".into()],
        };
        let mut worker = NativeWorker::spawn(
            &recipe,
            Path::new("/tmp"),
            NativeLimits {
                message_bytes: 1,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(
            worker
                .request(
                    WorkerRequest::Describe,
                    &NativeCancellation::default(),
                    &mut cases::reject_mock
                )
                .await
                .unwrap_err()
                .kind,
            NativeFailureKind::Protocol
        );
        assert!(!worker.group_active);
        assert!(worker.child.is_none());
    }
}

#[cfg(all(test, target_os = "linux"))]
mod cleanup_retry_tests {
    use super::*;
    struct UnavailableWait;
    impl GroupControl for UnavailableWait {
        fn kill(&self, pid: u32) -> Result<(), NativeFailure> {
            LinuxGroups.kill(pid)
        }
        fn reap(&self, _pid: u32) -> Result<ReapStep, NativeFailure> {
            Err(NativeFailure::new(
                NativeFailureKind::Cleanup,
                "Wait temporarily unavailable",
            ))
        }
    }
    #[tokio::test]
    async fn cleanup_retries_after_leader_reaping_without_claiming_a_completed_group() {
        let recipe = ExecutionRecipe {
            program: "python3".into(),
            arguments: vec!["-c".into(), "import time;time.sleep(300)".into()],
        };
        let mut worker = NativeWorker::spawn(&recipe, Path::new("/tmp"), NativeLimits::default())
            .await
            .unwrap();
        worker.groups = std::sync::Arc::new(UnavailableWait);
        assert_eq!(
            worker.abort().await.unwrap_err().kind,
            NativeFailureKind::Cleanup
        );
        assert!(worker.child.is_none());
        assert!(worker.group_active);
        worker.groups = std::sync::Arc::new(LinuxGroups);
        worker.abort().await.unwrap();
        assert!(!worker.group_active);
    }
}
