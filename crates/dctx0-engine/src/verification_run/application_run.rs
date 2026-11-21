//! Managed application tests. Native runner outcomes and central assessment are
//! independent gates. Only the managed publication wrapper can deliver tasks.
mod publication;
mod replay;
use super::*;
use crate::application_testing_service;
use dctx0_verification::application_session::ApplicationAssessmentReport;

#[cfg(unix)]
struct ApplicationReuseExecution<'a> {
    prepared: &'a mut super::reuse_execution::PreparedReuse,
    configuration_hash: String,
    task_pin: Option<dctx0_core::VerificationTaskPin>,
    selected: Option<dctx0_verification::incremental::ReuseBinding>,
    bundle_artifact: &'a [u8],
}

#[cfg(unix)]
struct SocketDirectory {
    path: Option<PathBuf>,
    device: u64,
    inode: u64,
}
#[cfg(unix)]
impl SocketDirectory {
    fn new() -> Result<Self> {
        use std::os::unix::fs::{DirBuilderExt, MetadataExt};
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| Error::Invalid(error.to_string()))?
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("dctx0-ipc-{:x}-{nonce:x}", std::process::id()));
        if path.join("s").as_os_str().as_encoded_bytes().len() > 100 {
            return Err(Error::Invalid("TMPDIR is too long for a local assessment socket; select a shorter temporary directory".into()));
        }
        fs::DirBuilder::new().mode(0o700).create(&path)?;
        let metadata = fs::symlink_metadata(&path)?;
        Ok(Self {
            path: Some(path),
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
    fn endpoint(&self) -> String {
        self.path
            .as_ref()
            .expect("owned IPC directory")
            .join("s")
            .to_string_lossy()
            .into_owned()
    }
    fn cleanup(&mut self) -> Result<()> {
        use std::os::unix::fs::MetadataExt;
        if let Some(path) = &self.path {
            let metadata = fs::symlink_metadata(path)?;
            if !metadata.is_dir()
                || metadata.file_type().is_symlink()
                || metadata.dev() != self.device
                || metadata.ino() != self.inode
            {
                return Err(Error::Invalid(
                    "Assessment IPC directory was replaced; refusing foreign cleanup".into(),
                ));
            }
            fs::remove_dir(path)?;
            self.path = None;
        }
        Ok(())
    }
}
#[cfg(unix)]
impl Drop for SocketDirectory {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ApplicationNativeRuntime {
    Go,
    Bun,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationNativeRecipe {
    pub schema_version: u32,
    pub runtime: ApplicationNativeRuntime,
    pub root_test: String,
    pub test_directory: String,
    pub prepare: Vec<ExecutionRecipe>,
    pub build: Vec<ExecutionRecipe>,
    pub catalog: Option<ExecutionRecipe>,
    pub run: ExecutionRecipe,
    pub junit: Option<String>,
    #[serde(default)]
    pub entrypoint: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationNativePhase {
    pub phase: String,
    pub recipe: ExecutionRecipe,
    pub runtime: NativeRuntimeIdentity,
    pub audit: NativeAudit,
    pub elapsed_us: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationNativeLimits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_bytes: Option<u64>,
    pub initialization_seconds: u64,
    pub case_seconds: u64,
    pub group_seconds: u64,
    pub cleanup_seconds: u64,
    pub message_bytes: usize,
    pub evidence_bytes: usize,
    pub log_bytes: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationNativeReport {
    pub schema_version: u32,
    pub group: String,
    #[serde(default)]
    pub backend: String,
    #[serde(default)]
    pub target: String,
    pub adoption_authority: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<StoredVerificationRun>,
    #[serde(default)]
    pub adopted_tasks: Vec<i64>,
    #[serde(default)]
    pub adoption_issues: Vec<VerificationIssue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adoption_proposal: Option<dctx0_tasks::AdoptionProposal>,
    #[serde(default)]
    pub limits: Option<ApplicationNativeLimits>,
    #[serde(default)]
    pub replay_of: Option<String>,
    #[serde(default)]
    pub requested_case: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incremental: Option<super::reuse_execution::NativeReuseSummary>,
    pub plan_hash: String,
    #[serde(default)]
    pub plan_obligations: Vec<VerificationIssue>,
    pub source_inputs: BTreeMap<String, String>,
    #[serde(default)]
    pub captured_root: String,
    pub build_inputs: BTreeMap<String, String>,
    pub materialized_inputs: BTreeMap<String, String>,
    #[serde(default)]
    pub toolchain_root: Option<String>,
    #[serde(default)]
    pub toolchain_inputs: BTreeMap<String, String>,
    pub phases: Vec<ApplicationNativePhase>,
    pub assessment: Option<ApplicationAssessmentReport>,
    pub issues: Vec<VerificationIssue>,
    pub result: VerificationResult,
}
fn materialized_inputs(root: &Path, junit: Option<&str>) -> Result<BTreeMap<String, String>> {
    bound_inputs(
        root,
        &["node_modules", ".dctx0ctl/application-tests"],
        junit,
    )
}
fn bound_inputs(
    root: &Path,
    paths: &[&str],
    junit: Option<&str>,
) -> Result<BTreeMap<String, String>> {
    fn visit(
        root: &Path,
        path: &Path,
        remaining: &mut u64,
        count: &mut usize,
        files: &mut BTreeMap<String, String>,
        junit: Option<&str>,
    ) -> Result<()> {
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        let relative = path
            .strip_prefix(root)
            .map_err(|_| Error::Invalid("Materialized input escapes capture".into()))?;
        let name = relative.to_string_lossy().replace('\\', "/");
        if junit == Some(name.as_str()) {
            return Ok(());
        }
        if relative.components().count() > 64 || *count >= 20_000 {
            return Err(Error::Invalid(
                "Materialized inputs exceed depth/count limits".into(),
            ));
        }
        *count += 1;
        if metadata.file_type().is_symlink() {
            let target = fs::canonicalize(path)?;
            if !target.starts_with(fs::canonicalize(root)?) || !target.is_file() {
                return Err(Error::Invalid(
                    "Materialized dependency link escapes capture or aliases a directory".into(),
                ));
            }
            files.insert(
                format!("{name}:symlink"),
                digest(fs::read_link(path)?.to_string_lossy().as_bytes()),
            );
        }
        if path.is_dir() {
            for entry in fs::read_dir(path)? {
                visit(root, &entry?.path(), remaining, count, files, junit)?;
            }
        } else if path.is_file() {
            let length = fs::metadata(path)?.len();
            if length > 268_435_456 || length > *remaining {
                return Err(Error::Invalid(
                    "Materialized inputs exceed byte limits".into(),
                ));
            }
            let bytes = fs::read(path)?;
            if bytes.len() as u64 != length {
                return Err(Error::Invalid(
                    "Materialized input changed while being bound".into(),
                ));
            }
            *remaining -= length;
            files.insert(name, digest(&bytes));
        } else {
            return Err(Error::Invalid(
                "Materialized input is not a regular file".into(),
            ));
        }
        Ok(())
    }
    let mut files = BTreeMap::new();
    let mut remaining = 536_870_912;
    let mut count = 0;
    for name in paths {
        visit(
            root,
            &root.join(name),
            &mut remaining,
            &mut count,
            &mut files,
            junit,
        )?;
    }
    Ok(files)
}
fn expand(recipe: &ExecutionRecipe, plan: &Path, artifacts: &Path) -> ExecutionRecipe {
    let replace = |s: &str| {
        s.replace("{plan}", &plan.to_string_lossy())
            .replace("{artifacts}", &artifacts.to_string_lossy())
    };
    ExecutionRecipe {
        program: replace(&recipe.program),
        arguments: recipe
            .arguments
            .iter()
            .map(|argument| replace(argument))
            .collect(),
    }
}
async fn phase(
    report: &mut ApplicationNativeReport,
    label: &str,
    recipe: ExecutionRecipe,
    directory: &Path,
    limits: NativeLimits,
    assessment: (&str, &Path, &Path),
    cancel: &NativeCancellation,
) -> Result<NativeAudit> {
    let (socket, plan, root) = assessment;
    let started = Instant::now();
    let mut command =
        NativeCommand::spawn_application_test(&recipe, directory, limits, socket, plan, root)
            .await
            .map_err(|error| Error::Invalid(error.to_string()))?;
    let runtime = command.runtime_identity();
    let audit = command
        .wait_status(cancel)
        .await
        .map_err(|error| Error::Invalid(error.to_string()))?;
    report.phases.push(ApplicationNativePhase {
        phase: label.into(),
        recipe,
        runtime,
        audit: audit.clone(),
        elapsed_us: started.elapsed().as_micros().min(u64::MAX as u128) as u64,
    });
    if audit.exit_code != Some(0) {
        return Err(Error::Invalid(format!(
            "Application test {label} exited {:?}: {}",
            audit.exit_code, audit.stderr
        )));
    }
    Ok(audit)
}
fn go_collection(
    stdout: &str,
    root: &str,
    registrations: &[dctx0_verification::application_plan::TestRegistration],
    reused: &BTreeSet<String>,
) -> Result<()> {
    let mut expected = BTreeSet::from([root.to_string()]);
    for registration in registrations {
        expected.insert(registration.test.clone());
        for case in &registration.cases {
            if reused.contains(case) {
                continue;
            }
            expected.insert(format!(
                "{}/case_{}",
                registration.test,
                digest(case.as_bytes())
            ));
        }
    }
    let mut completed = BTreeSet::new();
    let mut started = BTreeSet::new();
    for line in stdout.lines() {
        let event: serde_json::Value = serde_json::from_str(line)?;
        let action = event["Action"]
            .as_str()
            .ok_or_else(|| Error::Invalid("Native Go event lacks Action".into()))?;
        if !matches!(
            action,
            "start" | "run" | "pause" | "cont" | "pass" | "fail" | "skip" | "output" | "bench"
        ) {
            return Err(Error::Invalid("Unknown native Go event action".into()));
        }
        if action == "run"
            && let Some(test) = event["Test"].as_str()
            && (!expected.contains(test) || !started.insert(test.to_string()))
        {
            return Err(Error::Invalid(
                "Unexpected or duplicate native Go test start".into(),
            ));
        }
        if matches!(action, "fail" | "skip") {
            return Err(Error::Invalid("Native Go test failed or skipped".into()));
        }
        if action == "pass"
            && let Some(test) = event["Test"].as_str()
            && (!started.contains(test) || !completed.insert(test.to_string()))
        {
            return Err(Error::Invalid("Duplicate native Go test completion".into()));
        }
    }
    if completed != expected {
        return Err(Error::Invalid(
            "Native Go test collection differs from expected registrations/cases".into(),
        ));
    }
    Ok(())
}
fn bun_catalog(stdout: &str, root: &str, entrypoint: &str) -> Result<()> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Test {
        entrypoint: String,
        name: String,
        skipped: bool,
        focused: bool,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Catalog {
        schema_version: u32,
        collector_version: String,
        library_version: String,
        complete: bool,
        tests: Vec<Test>,
        limitations: Vec<String>,
    }
    if stdout.len() > 1_048_576 {
        return Err(Error::Invalid(
            "Bun registration catalog exceeds 1 MiB".into(),
        ));
    }
    let catalog: Catalog = serde_json::from_str(stdout)?;
    if catalog.schema_version != 1
        || catalog.collector_version != "dctx0-bun-registration-1"
        || catalog.library_version
            != dctx0_target_sdk::application_protocol::TYPESCRIPT_APPLICATION_LIBRARY_VERSION
        || !catalog.complete
        || catalog.tests.len() != 1
        || catalog.limitations.is_empty()
    {
        return Err(Error::Invalid(
            "Bun registration catalog is incomplete or incompatible".into(),
        ));
    }
    let test = &catalog.tests[0];
    if test.name != root || test.entrypoint != entrypoint || test.skipped || test.focused {
        return Err(Error::Invalid(
            "Bun registration catalog differs from its required native root".into(),
        ));
    }
    Ok(())
}
#[cfg(unix)]
fn publish_application_report(path: &Path, contents: &[u8], no_clobber: bool) -> Result<()> {
    if no_clobber {
        if !crate::filesystem::atomic_write_new(path, contents)? {
            return Err(Error::Invalid(format!(
                "Hybrid native report already exists: {}",
                path.display()
            )));
        }
        Ok(())
    } else {
        atomic_write(path, contents)
    }
}

impl Project {
    /// Capture first, build and test only that captured tree. No stored preview
    /// can authorize this run or turn its diagnostic report into task PASS.
    #[cfg(unix)]
    pub async fn run_application_tests(
        &self,
        application: &Path,
        plan: &str,
        group: &str,
        recipe: &str,
        output: &str,
        cancel: &NativeCancellation,
    ) -> Result<ApplicationNativeReport> {
        self.run_application_tests_with_report_policy(
            application,
            plan,
            group,
            recipe,
            output,
            cancel,
            false,
            None,
            None,
        )
        .await
    }

    #[cfg(unix)]
    pub(super) async fn run_application_tests_no_clobber(
        &self,
        application: &Path,
        plan: &str,
        group: &str,
        recipe: &str,
        output: &str,
        guard: (&NativeCancellation, Option<&str>),
    ) -> Result<ApplicationNativeReport> {
        let (cancel, expected_plan_sha256) = guard;
        self.run_application_tests_with_report_policy(
            application,
            plan,
            group,
            recipe,
            output,
            cancel,
            true,
            expected_plan_sha256,
            None,
        )
        .await
    }

    #[cfg(unix)]
    #[allow(clippy::too_many_arguments)]
    async fn run_application_tests_with_report_policy(
        &self,
        application: &Path,
        plan: &str,
        group: &str,
        recipe: &str,
        output: &str,
        cancel: &NativeCancellation,
        no_clobber: bool,
        expected_plan_sha256: Option<&str>,
        mut reuse: Option<&mut ApplicationReuseExecution<'_>>,
    ) -> Result<ApplicationNativeReport> {
        let (root, bundle, ir) =
            self.read_application_testing_bundle_checked(application, plan, expected_plan_sha256)?;
        let (_, config) = self.verification_configuration(Some(&root))?;
        if !output.starts_with(".dctx0ctl/application-tests/") {
            return Err(Error::Invalid(
                "Managed application evidence must be under .dctx0ctl/application-tests/".into(),
            ));
        }
        let (output_directory, output_path) = crate::verification_setup::safe_file(&root, output)?;
        let (_, recipe_path) = crate::verification_setup::safe_file(&root, recipe)?;
        if fs::metadata(&recipe_path)?.len() > 65_536 {
            return Err(Error::Invalid(
                "Application native recipe exceeds 64 KiB".into(),
            ));
        }
        let recipe: ApplicationNativeRecipe = serde_json::from_slice(&fs::read(recipe_path)?)?;
        if recipe.schema_version != 1
            || recipe.root_test.is_empty()
            || recipe.root_test.len() > 200
            || recipe.root_test.contains(['\n', '\r'])
            || recipe.prepare.len() + recipe.build.len() > 16
        {
            return Err(Error::Invalid("Invalid application native recipe".into()));
        }
        if (recipe.runtime == ApplicationNativeRuntime::Go && config.target != "go")
            || (recipe.runtime == ApplicationNativeRuntime::Bun && config.target != "typescript")
        {
            return Err(Error::Invalid(
                "Native recipe runtime differs from project target".into(),
            ));
        }
        let native_group = bundle
            .plan
            .configuration
            .groups
            .iter()
            .find(|g| {
                g.id == group
                    && g.backend
                        == dctx0_verification::application_plan::TestBackend::ApplicationTests
            })
            .ok_or_else(|| Error::Invalid("Unknown native group".into()))?;
        if bundle.plan.registrations.iter().any(|registration| {
            registration.group == group
                && registration.test != recipe.root_test
                && !registration
                    .test
                    .starts_with(&format!("{}/", recipe.root_test))
        }) {
            return Err(Error::Invalid(
                "Native recipe root does not own its registrations".into(),
            ));
        }
        // A runtime receipt must describe exactly one group. Cross-group source
        // case assembly and transactional adoption remain the later cohort gate.
        if bundle
            .plan
            .registrations
            .iter()
            .any(|registration| registration.group != group)
        {
            return Err(Error::Invalid(
                "One managed native run requires one application-test group".into(),
            ));
        }
        let capture = Capture::application_tests(&root, config.capture_budgets())?;
        if let Some(review) = &bundle.decoder_review_hash
            && Capture::go_review_hash(&capture.root, &capture.materialized)? != *review
        {
            return Err(Error::Invalid(
                "DECODER_REVIEW_STALE: captured Go inputs differ from the reviewed plan".into(),
            ));
        }
        let artifacts = capture.root.join(".dctx0ctl/application-tests");
        fs::create_dir_all(&artifacts)?;
        let (_, captured_plan) = crate::verification_setup::safe_file(&capture.root, plan)?;
        fs::create_dir_all(captured_plan.parent().expect("validated plan has parent"))?;
        atomic_write(&captured_plan, &serde_json::to_vec(&bundle)?)?;
        let (_, directory_marker) = crate::verification_setup::safe_file(
            &capture.root,
            &format!("{}/.native-directory", recipe.test_directory),
        )?;
        let native_directory = if recipe.test_directory == "root" {
            capture.root.clone()
        } else {
            directory_marker
                .parent()
                .expect("safe file has parent")
                .to_path_buf()
        };
        if !native_directory.is_dir() {
            return Err(Error::Invalid("Native test directory is missing".into()));
        }
        // Unix socket paths are much shorter than normal filesystem paths.
        // Keep IPC outside the potentially deep capture/artifact directory.
        let mut socket_directory = SocketDirectory::new()?;
        let socket = socket_directory.endpoint();
        let mut limits = NativeLimits {
            environment: config.environment.clone().unwrap_or_default(),
            run: std::time::Duration::from_secs(native_group.resources.timeout_seconds),
            log_bytes: 33_554_432,
            ..Default::default()
        };
        if let Some(execution) = &config.execution {
            limits.initialization =
                std::time::Duration::from_secs(execution.initialization_seconds);
            limits.case = std::time::Duration::from_secs(execution.case_seconds);
            limits.cleanup = std::time::Duration::from_secs(execution.cleanup_seconds);
            limits.message_bytes = execution.message_bytes;
            limits.evidence_bytes = execution.evidence_bytes;
        }
        let mut report = ApplicationNativeReport {
            schema_version: 1,
            group: group.into(),
            backend: "application-tests".into(),
            target: config.target.clone(),
            adoption_authority: false,
            evidence: None,
            adopted_tasks: vec![],
            adoption_issues: vec![],
            adoption_proposal: None,
            captured_root: capture.root.to_string_lossy().into_owned(),
            limits: Some(ApplicationNativeLimits {
                plan_bytes: Some(config.application_plan_bytes()),
                initialization_seconds: limits.initialization.as_secs(),
                case_seconds: limits.case.as_secs(),
                group_seconds: limits.run.as_secs(),
                cleanup_seconds: limits.cleanup.as_secs(),
                message_bytes: limits.message_bytes,
                evidence_bytes: limits.evidence_bytes,
                log_bytes: limits.log_bytes,
            }),
            replay_of: None,
            requested_case: None,
            incremental: None,
            plan_hash: digest(&serde_json::to_vec(&bundle)?),
            plan_obligations: bundle.plan.manifest.open_obligations.clone(),
            source_inputs: capture.files.clone(),
            build_inputs: BTreeMap::new(),
            materialized_inputs: BTreeMap::new(),
            toolchain_root: None,
            toolchain_inputs: BTreeMap::new(),
            phases: vec![],
            assessment: None,
            issues: vec![],
            result: unknown(
                "APPLICATION_RUN_INCOMPLETE",
                "Native execution did not complete",
            ),
        };
        let run_cancel = NativeCancellation::default();
        let mut reused_ids = BTreeSet::new();
        let execution = {
            let execution_future = async {
                if recipe.runtime == ApplicationNativeRuntime::Go {
                    let environment = phase(
                        &mut report,
                        "toolchain",
                        ExecutionRecipe {
                            program: "go".into(),
                            arguments: vec![
                                "env".into(),
                                "-json".into(),
                                "GOROOT".into(),
                                "GOTOOLDIR".into(),
                                "GOVERSION".into(),
                                "CGO_ENABLED".into(),
                            ],
                        },
                        &capture.root,
                        limits.clone(),
                        (&socket, &captured_plan, &capture.root),
                        &run_cancel,
                    )
                    .await?;
                    let environment: serde_json::Value = serde_json::from_str(&environment.stdout)?;
                    let root = environment["GOROOT"]
                        .as_str()
                        .ok_or_else(|| Error::Invalid("Go toolchain has no GOROOT".into()))?;
                    let tools = environment["GOTOOLDIR"]
                        .as_str()
                        .ok_or_else(|| Error::Invalid("Go toolchain has no GOTOOLDIR".into()))?;
                    let toolchain = fs::canonicalize(root)?;
                    if !Path::new(root).is_absolute()
                        || !fs::canonicalize(tools)?.starts_with(&toolchain)
                        || environment["CGO_ENABLED"] != "0"
                    {
                        return Err(Error::Invalid(
                            "Unsupported Go toolchain layout or CGO environment".into(),
                        ));
                    }
                    report.toolchain_inputs =
                        bound_inputs(&toolchain, &["VERSION", "src", "pkg/tool", "lib"], None)?;
                    if report.toolchain_inputs.is_empty() {
                        return Err(Error::Invalid("Go toolchain inputs are unavailable".into()));
                    }
                    report.toolchain_root = Some(toolchain.to_string_lossy().into_owned());
                }
                for (index, command) in recipe.prepare.iter().enumerate() {
                    phase(
                        &mut report,
                        &format!("prepare-{index}"),
                        expand(command, &captured_plan, &artifacts),
                        &capture.root,
                        limits.clone(),
                        (&socket, &captured_plan, &capture.root),
                        &run_cancel,
                    )
                    .await?;
                }
                for (index, command) in recipe.build.iter().enumerate() {
                    phase(
                        &mut report,
                        &format!("build-{index}"),
                        expand(command, &captured_plan, &artifacts),
                        &capture.root,
                        limits.clone(),
                        (&socket, &captured_plan, &capture.root),
                        &run_cancel,
                    )
                    .await?;
                }
                report.build_inputs =
                    Capture::application_inventory(&capture.root, config.capture_budgets())?;
                report.materialized_inputs =
                    materialized_inputs(&capture.root, recipe.junit.as_deref())?;
                if let Some(command) = &recipe.catalog {
                    let catalog = phase(
                        &mut report,
                        "catalog",
                        expand(command, &captured_plan, &artifacts),
                        &native_directory,
                        limits.clone(),
                        (&socket, &captured_plan, &capture.root),
                        &run_cancel,
                    )
                    .await?;
                    match recipe.runtime {
                        ApplicationNativeRuntime::Go => {
                            if catalog
                                .stdout
                                .lines()
                                .filter(|name| *name == recipe.root_test)
                                .count()
                                != 1
                            {
                                return Err(Error::Invalid(
                                    "Native test catalog does not contain its root exactly once"
                                        .into(),
                                ));
                            }
                        }
                        ApplicationNativeRuntime::Bun => {
                            let entrypoint = recipe.entrypoint.as_ref().ok_or_else(|| {
                                Error::Invalid("Bun native entrypoint is required".into())
                            })?;
                            let (_, path) =
                                crate::verification_setup::safe_file(&capture.root, entrypoint)?;
                            if !path.is_file() {
                                return Err(Error::Invalid(
                                    "Bun native entrypoint is absent from captured sources".into(),
                                ));
                            }
                            bun_catalog(&catalog.stdout, &recipe.root_test, entrypoint)?;
                        }
                    }
                } else {
                    return Err(Error::Invalid(
                        "Native registration catalog is required".into(),
                    ));
                }
                let mut reused_records = Vec::new();
                if let Some(reuse) = reuse.as_deref_mut()
                    && let Some(parent) = reuse.prepared.parent.as_ref()
                    && let Some(catalog) =
                        report.phases.iter().find(|phase| phase.phase == "catalog")
                {
                    let mut preview = report.clone();
                    let mut phase = catalog.clone();
                    phase.phase = "native-tests".into();
                    let native_recipe = expand(&recipe.run, &captured_plan, &artifacts);
                    let effective = limits
                        .environment
                        .entries()
                        .map_err(|error| Error::Invalid(error.to_string()))?;
                    let executable = crate::native::identity::resolve_program_with_path(
                        &native_recipe.program,
                        &native_directory,
                        crate::native::environment_value(&effective, "PATH"),
                    )
                    .map_err(|error| Error::Invalid(error.to_string()))?;
                    if fs::metadata(&executable)?.len() > 268_435_456 {
                        return Err(Error::Invalid(
                            "Native executable exceeds identity limit".into(),
                        ));
                    }
                    phase.runtime.executable_hash = digest(&fs::read(executable)?);
                    preview.phases.push(phase);
                    let mut candidate = parent.run.clone();
                    candidate.provenance.snapshot = bundle.plan.program.snapshot.clone();
                    candidate.provenance.check_occurrence =
                        bundle.plan.program.check_occurrence as i64;
                    candidate.provenance.plan_hash =
                        digest(&serde_json::to_vec(&bundle.plan.program)?);
                    candidate.provenance.implementation = report.source_inputs.clone();
                    candidate.provenance.dependencies = report.materialized_inputs.clone();
                    candidate.provenance.build_configuration = report.build_inputs.clone();
                    candidate.provenance.adapter_hash = report.plan_hash.clone();
                    candidate.provenance.configuration_hash = reuse.configuration_hash.clone();
                    candidate.provenance.task_pin = reuse.task_pin.clone();
                    candidate.provenance.incomplete.clear();
                    if let Some(binding) =
                        dctx0_verification::application_reuse::preview_binding_with_bundle(
                            &bundle.plan.program,
                            &candidate,
                            reuse.prepared.receipt_artifact.reference.hash.clone(),
                            &serde_json::to_vec(&preview)?,
                            reuse.bundle_artifact,
                        )
                    {
                        let selection = reuse.prepared.select_binding(&binding)?;
                        reused_ids = selection.reused;
                        reused_records = reuse
                            .prepared
                            .parent
                            .as_ref()
                            .expect("parent present")
                            .run
                            .cases
                            .iter()
                            .filter(|case| reused_ids.contains(&case.id))
                            .cloned()
                            .collect();
                        if !reused_ids.is_empty() {
                            reuse.selected = Some(binding);
                        }
                    }
                }
                let service_cancel = NativeCancellation::default();
                let server = async {
                    let result = application_testing_service::serve_cancellable_with_reuse(
                        &ir,
                        &bundle,
                        group,
                        &socket,
                        &service_cancel,
                        reused_records,
                    )
                    .await;
                    if result.as_ref().is_err()
                        || result.as_ref().is_ok_and(|assessment| {
                            !assessment.finalized
                                || assessment
                                    .issues
                                    .iter()
                                    .any(|issue| issue.code == "APPLICATION_TRANSPORT")
                        })
                    {
                        run_cancel.cancel();
                    }
                    result
                };
                let native = async {
                    let result = async {
                let ready = async {
                    loop {
                        if Path::new(&socket).exists() {
                            break;
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                    }
                };
                tokio::select! {
                    result=tokio::time::timeout(limits.initialization, ready)=>result.map_err(|_|Error::Invalid("Assessment service did not start".into()))?,
                    _=run_cancel.cancelled()=>return Err(Error::Invalid("Native startup cancelled".into())),
                };
                phase(
                    &mut report,
                    "native-tests",
                    expand(&recipe.run, &captured_plan, &artifacts),
                    &native_directory,
                    limits.clone(),
                    (&socket, &captured_plan, &capture.root),
                    &run_cancel,
                )
                .await
                }.await;
                    // After process exit no native background work can legitimately
                    // produce evidence. Cancel an unclosed channel and retain prefix.
                    service_cancel.cancel();
                    result
                };
                let (assessment, native) = tokio::join!(server, native);
                report.assessment = Some(assessment?);
                let native = native?;
                match recipe.runtime {
                    ApplicationNativeRuntime::Go => go_collection(
                        &native.stdout,
                        &recipe.root_test,
                        &bundle.plan.registrations,
                        &reused_ids,
                    )?,
                    ApplicationNativeRuntime::Bun => {
                        let junit = recipe.junit.as_ref().ok_or_else(|| {
                            Error::Invalid("Bun native JUnit report is required".into())
                        })?;
                        let (_, path) = crate::verification_setup::safe_file(&capture.root, junit)?;
                        if fs::metadata(&path)?.len() > 1_048_576 {
                            return Err(Error::Invalid("Bun native JUnit exceeds 1 MiB".into()));
                        }
                        crate::application_test_junit::assess(&fs::read(path)?, &recipe.root_test)
                            .map_err(Error::Invalid)?;
                    }
                }
                if report.build_inputs
                    != Capture::application_inventory(&capture.root, config.capture_budgets())?
                {
                    return Err(Error::Invalid(
                        "Captured inputs changed during native test execution".into(),
                    ));
                }
                if report.materialized_inputs
                    != materialized_inputs(&capture.root, recipe.junit.as_deref())?
                {
                    return Err(Error::Invalid(
                        "Materialized dependencies or native artifacts changed during execution"
                            .into(),
                    ));
                }
                if let Some(toolchain) = &report.toolchain_root
                    && report.toolchain_inputs
                        != bound_inputs(
                            Path::new(toolchain),
                            &["VERSION", "src", "pkg/tool", "lib"],
                            None,
                        )?
                {
                    return Err(Error::Invalid(
                        "Go toolchain inputs changed during execution".into(),
                    ));
                }
                if report.source_inputs
                    != Capture::application_inventory(&root, config.capture_budgets())?
                {
                    return Err(Error::Invalid(
                        "Project sources changed after capture".into(),
                    ));
                }
                let (_, current_bundle, _) = self.read_application_testing_bundle_checked(
                    &root,
                    plan,
                    expected_plan_sha256,
                )?;
                if digest(&serde_json::to_vec(&current_bundle)?) != report.plan_hash {
                    return Err(Error::Invalid(
                        "Published requirements or plan changed during native execution".into(),
                    ));
                }
                Ok::<(), Error>(())
            };
            tokio::pin!(execution_future);
            tokio::select! {
                result=&mut execution_future=>result,
                _=tokio::time::sleep(limits.run)=>{run_cancel.cancel();execution_future.await},
                _=cancel.cancelled()=>{run_cancel.cancel();execution_future.await},
            }
        };
        if let Err(error) = execution {
            report
                .issues
                .push(issue("APPLICATION_NATIVE_RUN", error.to_string()));
        }
        for phase in &report.phases {
            if !phase.runtime.still_current().unwrap_or(false) {
                report.issues.push(issue(
                    "APPLICATION_RUNTIME_CHANGED",
                    format!("{} runtime changed before capture cleanup", phase.phase),
                ));
            }
        }
        if let Err(error) = socket_directory.cleanup() {
            report
                .issues
                .push(issue("APPLICATION_IPC_CLEANUP", error.to_string()));
        }
        report.result = report
            .assessment
            .as_ref()
            .map(|assessment| assessment.result.clone())
            .unwrap_or_else(|| unknown("APPLICATION_RUN_INCOMPLETE", "No assessment"));
        if reuse.as_ref().is_some_and(|state| state.selected.is_some())
            && report.phases.iter().any(|phase| {
                phase.phase == "native-tests" && phase.audit.exit_code.is_some_and(|code| code != 0)
            })
            && !matches!(report.result, VerificationResult::Fail { .. })
        {
            report.result = VerificationResult::Fail {
                issue: issue(
                    "APPLICATION_NATIVE_TEST_FAILURE",
                    "The native test process failed",
                ),
            };
        }
        if !report.issues.is_empty() && !matches!(report.result, VerificationResult::Fail { .. }) {
            report.result = unknown(
                "APPLICATION_RUN_INCOMPLETE",
                "Native collection, build or source binding failed",
            );
        }
        fs::create_dir_all(output_directory)?;
        let contents = serde_json::to_vec_pretty(&report)?;
        publish_application_report(&output_path, &contents, no_clobber)?;
        Ok(report)
    }
    #[cfg(not(unix))]
    pub(super) async fn run_application_tests_no_clobber(
        &self,
        _: &Path,
        _: &str,
        _: &str,
        _: &str,
        _: &str,
        _: (&NativeCancellation, Option<&str>),
    ) -> Result<ApplicationNativeReport> {
        Err(Error::Invalid(
            "Application-test assessment requires local Unix channels".into(),
        ))
    }
    #[cfg(not(unix))]
    pub async fn run_application_tests(
        &self,
        _: &Path,
        _: &str,
        _: &str,
        _: &str,
        _: &str,
        _: &NativeCancellation,
    ) -> Result<ApplicationNativeReport> {
        Err(Error::Invalid(
            "Application-test assessment requires local Unix channels".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dctx0_verification::application_plan::{TestForm, TestRegistration};
    #[cfg(unix)]
    #[test]
    fn hybrid_report_publication_cannot_replace_a_concurrent_writer() {
        let directory = std::env::temp_dir().join(format!(
            "dctx0-report-publish-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&directory).unwrap();
        let report = directory.join("native.json");
        fs::write(&report, b"concurrent evidence").unwrap();
        assert!(publish_application_report(&report, b"new evidence", true).is_err());
        assert_eq!(fs::read(&report).unwrap(), b"concurrent evidence");
        let new = directory.join("fresh.json");
        publish_application_report(&new, b"fresh evidence", true).unwrap();
        assert_eq!(fs::read(&new).unwrap(), b"fresh evidence");
    }

    #[cfg(unix)]
    #[test]
    fn assessment_socket_uses_short_private_directory_and_refuses_foreign_cleanup() {
        use std::os::unix::fs::PermissionsExt;
        let mut directory = SocketDirectory::new().unwrap();
        let path = directory.path.as_ref().unwrap().clone();
        assert!(directory.endpoint().len() <= 100);
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o700
        );
        let displaced = path.with_extension("owned");
        fs::rename(&path, &displaced).unwrap();
        fs::create_dir(&path).unwrap();
        fs::write(path.join("foreign"), "preserved").unwrap();
        assert!(directory.cleanup().is_err());
        assert_eq!(fs::read(path.join("foreign")).unwrap(), b"preserved");
        fs::remove_file(path.join("foreign")).unwrap();
        fs::remove_dir(&path).unwrap();
        fs::rename(&displaced, &path).unwrap();
        directory.cleanup().unwrap();
        assert!(!path.exists());
        directory.cleanup().unwrap();
    }
    #[test]
    fn go_native_collection_rejects_missing_skipped_duplicate_unstarted_and_foreign_tests() {
        let registration = TestRegistration {
            group: "native".into(),
            test: "TestSuite/decoder".into(),
            form: TestForm::Decoder,
            cases: ["case".into()].into(),
        };
        let names = [
            "TestSuite".to_string(),
            registration.test.clone(),
            format!("{}/case_{}", registration.test, digest(b"case")),
        ];
        let mut events = Vec::new();
        for name in &names {
            for action in ["run", "pass"] {
                events.push(serde_json::json!({"Action":action,"Test":name}));
            }
        }
        let encode = |events: &[serde_json::Value]| {
            events
                .iter()
                .map(|event| serde_json::to_string(event).unwrap())
                .collect::<Vec<_>>()
                .join("\n")
        };
        let registrations = [registration];
        assert!(
            go_collection(
                &encode(&events),
                "TestSuite",
                &registrations,
                &BTreeSet::new()
            )
            .is_ok()
        );
        let reused = BTreeSet::from(["case".to_string()]);
        assert!(go_collection(&encode(&events[..4]), "TestSuite", &registrations, &reused).is_ok());
        assert!(
            go_collection(&encode(&events[..2]), "TestSuite", &registrations, &reused).is_err()
        );
        for mutation in 0..8 {
            let mut changed = events.clone();
            match mutation {
                0 => {
                    changed.pop();
                }
                1 => changed[1]["Action"] = "skip".into(),
                2 => changed.push(changed[1].clone()),
                3 => {
                    changed.remove(0);
                }
                4 => changed[0]["Test"] = "Foreign".into(),
                5 => changed[1]["Action"] = "fail".into(),
                6 => changed[1]["Action"] = "invented".into(),
                _ => changed[1] = serde_json::json!({"Test":"TestSuite"}),
            }
            assert!(
                go_collection(
                    &encode(&changed),
                    "TestSuite",
                    &registrations,
                    &BTreeSet::new()
                )
                .is_err(),
                "mutation {mutation}"
            );
        }
        assert!(go_collection("not json", "TestSuite", &registrations, &BTreeSet::new()).is_err());
    }
    #[test]
    fn bun_registration_requires_pinned_complete_unique_unfocused_native_parent() {
        let catalog = serde_json::json!({"schema_version":1,"collector_version":"dctx0-bun-registration-1","library_version":"0.2.0","complete":true,"tests":[{"entrypoint":"cinema.test.ts","name":"CinemaSuite","skipped":false,"focused":false}],"limitations":["No individual native case enumeration"]});
        let assess = |catalog: &serde_json::Value| {
            bun_catalog(&catalog.to_string(), "CinemaSuite", "cinema.test.ts")
        };
        assert!(assess(&catalog).is_ok());
        for mutation in 0..10 {
            let mut changed = catalog.clone();
            match mutation {
                0 => changed["complete"] = false.into(),
                1 => changed["tests"][0]["focused"] = true.into(),
                2 => changed["tests"][0]["skipped"] = true.into(),
                3 => changed["tests"][0]["entrypoint"] = "other.ts".into(),
                4 => changed["tests"][0]["name"] = "Other".into(),
                5 => changed["tests"]
                    .as_array_mut()
                    .unwrap()
                    .push(catalog["tests"][0].clone()),
                6 => changed["schema_version"] = 2.into(),
                7 => changed["library_version"] = "0.1.0".into(),
                8 => changed["invented"] = true.into(),
                _ => changed["limitations"] = serde_json::json!([]),
            }
            assert!(assess(&changed).is_err(), "mutation {mutation}");
        }
        assert!(bun_catalog(&" ".repeat(1_048_577), "CinemaSuite", "cinema.test.ts").is_err());
        let recipe = ExecutionRecipe {
            program: "tool".into(),
            arguments: vec!["{plan}".into(), "{artifacts}/output".into()],
        };
        let expanded = expand(
            &recipe,
            Path::new("/captured/plan"),
            Path::new("/captured/build"),
        );
        assert_eq!(
            expanded.arguments,
            vec!["/captured/plan", "/captured/build/output"]
        );
    }
    #[cfg(unix)]
    #[test]
    fn materialized_inputs_bind_internal_file_links_and_reject_escaping_or_special_inputs() {
        use std::os::unix::{fs::symlink, net::UnixListener};
        let fixture =
            std::env::temp_dir().join(format!("dctx0-bound-links-{}", std::process::id()));
        fs::create_dir_all(fixture.join("node_modules")).unwrap();
        fs::write(fixture.join("actual"), b"dependency").unwrap();
        let outside =
            std::env::temp_dir().join(format!("dctx0-bound-outside-{}", std::process::id()));
        fs::write(&outside, b"foreign").unwrap();
        assert!(
            bound_inputs(&fixture, &[outside.to_str().unwrap()], None)
                .unwrap_err()
                .to_string()
                .contains("escapes capture")
        );
        fs::remove_file(outside).unwrap();
        let link = fixture.join("node_modules/dependency");
        symlink("../actual", &link).unwrap();
        let inventory = bound_inputs(&fixture, &["node_modules", "absent"], None).unwrap();
        assert_eq!(inventory["node_modules/dependency"], digest(b"dependency"));
        assert_eq!(
            inventory["node_modules/dependency:symlink"],
            digest(b"../actual")
        );
        fs::remove_file(&link).unwrap();
        symlink(std::env::temp_dir(), &link).unwrap();
        assert!(
            bound_inputs(&fixture, &["node_modules"], None)
                .unwrap_err()
                .to_string()
                .contains("escapes capture")
        );
        fs::remove_file(&link).unwrap();
        symlink("..", &link).unwrap();
        assert!(bound_inputs(&fixture, &["node_modules"], None).is_err());
        fs::remove_file(&link).unwrap();
        let listener = UnixListener::bind(&link).unwrap();
        assert!(
            bound_inputs(&fixture, &["node_modules"], None)
                .unwrap_err()
                .to_string()
                .contains("not a regular file")
        );
        drop(listener);
        fs::remove_file(&link).unwrap();
        fs::remove_dir_all(fixture).unwrap();
    }

    #[test]
    fn materialized_inputs_enforce_depth_and_sparse_file_size_before_reading() {
        let fixture =
            std::env::temp_dir().join(format!("dctx0-bound-limits-{}", std::process::id()));
        fs::create_dir_all(&fixture).unwrap();
        let mut deep = fixture.clone();
        for _ in 0..65 {
            deep.push("d");
        }
        fs::create_dir_all(&deep).unwrap();
        assert!(
            bound_inputs(&fixture, &["d"], None)
                .unwrap_err()
                .to_string()
                .contains("depth/count")
        );
        fs::remove_dir_all(fixture.join("d")).unwrap();
        let file = fs::File::create(fixture.join("oversized")).unwrap();
        file.set_len(268_435_457).unwrap();
        assert!(
            bound_inputs(&fixture, &["oversized"], None)
                .unwrap_err()
                .to_string()
                .contains("byte limits")
        );
        // An output report is excluded before bytes are loaded, even if large.
        assert!(
            bound_inputs(&fixture, &["oversized"], Some("oversized"))
                .unwrap()
                .is_empty()
        );
        fs::remove_dir_all(fixture).unwrap();
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn assessment_service_rejects_unknown_group_and_invalid_plan_before_binding() {
        let root =
            std::env::temp_dir().join(format!("dctx0-service-invalid-{}", std::process::id()));
        fs::create_dir_all(root.join(".dctx0/testing")).unwrap();
        fs::write(
            root.join("spec.dctx0"),
            "workspace W { value Year = Int { range 3..4 } }",
        )
        .unwrap();
        fs::write(root.join("fixtures.json"), r#"{"examples":{}}"#).unwrap();
        fs::write(root.join(".dctx0/testing/verification.json"),serde_json::json!({"schema_version":1,"target":"typescript","runtime":"bun","adapter":"adapter.ts","fixtures":"fixtures.json","scope":{"subjects":["W.Year"],"tasks":[]},"budgets":VerificationBudgets::default()}).to_string()).unwrap();
        let project = Project::new(&root, None).unwrap();
        let mut bundle=project.prepare_application_tests(&root,&crate::ApplicationTestingPreparation{plan:"plan.json",group:"native",test:"Year",form:TestForm::Decoder,group_seconds:Some(3),stateless_decoders:Default::default(),boundary:dctx0_verification::application_session::ApplicationValueBoundary::ValueConstructor,subjects:Default::default()}).unwrap();
        let (_, _, ir) = project
            .read_application_testing_bundle(&root, "plan.json")
            .unwrap();
        let socket = root.join("must-not-bind");
        for group in ["unknown", "native"] {
            if group == "native" {
                bundle.plan.configuration.groups.clear();
            }
            let result = application_testing_service::serve_cancellable(
                &ir,
                &bundle,
                group,
                socket.to_str().unwrap(),
                &NativeCancellation::default(),
            )
            .await;
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("Invalid assessment group")
            );
            assert!(
                !socket.exists(),
                "Invalid groups must fail before opening a listener"
            );
        }
        fs::remove_dir_all(root).unwrap();
    }
}
