//! Explicit repository self-verification. The development task database is never opened.
use crate::verification_run::rust_build;
use crate::*;
use dctx0_core::{VerificationIssue, VerificationResult};
use dctx0_target_sdk::{ExecutionRecipe, HarnessGenerator};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Serialize)]
pub struct SelfVerificationReport {
    pub schema_version: u32,
    pub source_root: PathBuf,
    pub captured_root: PathBuf,
    pub report_path: PathBuf,
    pub source_files: BTreeMap<String, String>,
    pub source_current: bool,
    pub partial: bool,
    pub preparation_ms: u128,
    pub preparation_peak_rss_bytes: Option<u64>,
    pub orchestrator_peak_rss_bytes: Option<u64>,
    pub cli_build_ms: u128,
    pub execution_ms: u128,
    pub cli_hash: String,
    pub native: NativeVerificationReport,
    pub machine_complete: bool,
    pub selected_machine_complete: bool,
    pub review_obligations: Vec<VerificationIssue>,
    pub runtime_measurements: Vec<dctx0_target_sdk::WorkerReply>,
}
const ROOTS: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "install.sh",
    "install.ps1",
    ".github",
    "scripts",
    "crates",
    "targets",
    "skills",
    "editors",
    "ui/site",
    "fixtures",
    "dctx0",
    ".dctx0/testing",
];
fn inventory(root: &Path) -> Result<BTreeMap<String, String>> {
    fn walk(
        root: &Path,
        relative: &Path,
        out: &mut BTreeMap<String, String>,
        remaining: &mut u64,
    ) -> Result<()> {
        if relative.components().count() > 64 || out.len() >= 20_000 {
            return Err(Error::Invalid(
                "Self-verification capture budget exceeded".into(),
            ));
        }
        let path = root.join(relative);
        let meta = fs::symlink_metadata(&path)?;
        if meta.file_type().is_symlink() {
            return Err(Error::Invalid(format!(
                "Self-verification input is a symlink: {}",
                path.display()
            )));
        }
        if meta.is_dir() {
            let mut entries = fs::read_dir(&path)?.collect::<std::io::Result<Vec<_>>>()?;
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                if matches!(
                    entry.file_name().to_str(),
                    Some(
                        "target"
                            | "node_modules"
                            | ".git"
                            | ".dctx0ctl"
                            | ".bin"
                            | "executables"
                            | "__pycache__"
                    )
                ) {
                    continue;
                }
                walk(root, &relative.join(entry.file_name()), out, remaining)?;
            }
        } else if meta.is_file() {
            if matches!(
                path.extension().and_then(|s| s.to_str()),
                Some("sqlite3" | "sqlite" | "db")
            ) {
                return Ok(());
            }
            let mut bytes = Vec::new();
            fs::File::open(&path)?
                .take(16_777_217)
                .read_to_end(&mut bytes)?;
            if bytes.len() > 16_777_216 || bytes.len() as u64 > *remaining {
                return Err(Error::Invalid(
                    "Self-verification source size budget exceeded".into(),
                ));
            }
            *remaining -= bytes.len() as u64;
            let name = relative
                .to_str()
                .ok_or_else(|| Error::Invalid("Non UTF-8 self-verification path".into()))?;
            if name.contains('\\') {
                return Err(Error::Invalid("Ambiguous self-verification path".into()));
            }
            out.insert(name.into(), format!("{:x}", Sha256::digest(&bytes)));
        } else {
            return Err(Error::Invalid("Nonregular self-verification input".into()));
        }
        Ok(())
    }
    let mut files = BTreeMap::new();
    let mut remaining = 536_870_912;
    for relative in ROOTS {
        walk(root, Path::new(relative), &mut files, &mut remaining)?;
    }
    Ok(files)
}
fn process_peak_rss() -> Option<u64> {
    fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find_map(|line| {
            line.strip_prefix("VmHWM:")
                .and_then(|value| value.split_whitespace().next())
                .and_then(|value| value.parse::<u64>().ok())
                .and_then(|kb| kb.checked_mul(1024))
        })
}
fn capture_stamp(now: SystemTime) -> Result<u128> {
    Ok(now
        .duration_since(UNIX_EPOCH)
        .map_err(|error| Error::Invalid(error.to_string()))?
        .as_nanos())
}
/// Capture and build before executing. Review UNKNOWN remains distinct from machine completeness.
pub async fn verify_self(
    root: &Path,
    subjects: BTreeSet<String>,
    generator: &dyn HarnessGenerator,
    provider: &dyn dctx0_core::TargetProfileProvider,
    cancellation: &NativeCancellation,
) -> Result<SelfVerificationReport> {
    let start = Instant::now();
    let root = fs::canonicalize(root)?;
    let files = inventory(&root)?;
    let stamp = capture_stamp(SystemTime::now())?;
    let destination = root
        .join("target/self-verification")
        .join(format!("{stamp}-{}", std::process::id()));
    fs::create_dir_all(&destination)?;
    for (name, hash) in &files {
        let bytes = fs::read(root.join(name))?;
        if format!("{:x}", Sha256::digest(&bytes)) != *hash {
            return Err(Error::Invalid(format!(
                "Self-verification source changed during capture: {name}"
            )));
        }
        let path = destination.join(name);
        fs::create_dir_all(
            path.parent()
                .ok_or_else(|| Error::Invalid("Capture path lacks parent".into()))?,
        )?;
        fs::write(path, bytes)?;
    }
    let project = Project::new(&destination.join("dctx0"), None)?;
    let check = project.check()?;
    if !check.ok {
        return Err(Error::Invalid(format!(
            "Self specification rejected: {:?}",
            check.diagnostics
        )));
    }
    create_profiles(&destination, provider, true, false, false)?;
    project.task_plan("latest")?;
    let preparation_ms = start.elapsed().as_millis();
    let preparation_peak_rss_bytes = process_peak_rss();
    let build = Instant::now();
    let receipt = rust_build::prepare(&destination, &destination, cancellation).await?;
    let recipe = ExecutionRecipe {
        program: "cargo".into(),
        arguments: vec![
            "build".into(),
            "--release".into(),
            "--locked".into(),
            "--offline".into(),
            "--all-features".into(),
            "-p".into(),
            "dctx0-cli".into(),
            "--target-dir".into(),
            receipt.target_directory.clone(),
        ],
    };
    let mut command = NativeCommand::spawn(
        &recipe,
        &destination,
        NativeLimits {
            run: Duration::from_secs(900),
            log_bytes: 4_194_304,
            ..Default::default()
        },
    )
    .await
    .map_err(|e| Error::Invalid(e.to_string()))?;
    let audit = command
        .wait(cancellation)
        .await
        .map_err(|e| Error::Invalid(e.to_string()))?;
    let receipt = rust_build::finish_named(
        receipt,
        &destination,
        "dctx0",
        &format!(
            ".dctx0/testing/rust/executables/dctx0{}",
            std::env::consts::EXE_SUFFIX
        ),
    )?;
    let cli_hash = receipt.binary_hash.clone();
    fs::write(
        destination.join(".dctx0/testing/rust/cli-build-receipt.json"),
        serde_json::to_vec_pretty(&receipt)?,
    )?;
    fs::write(
        destination.join(".dctx0/testing/rust/cli-build-audit.json"),
        serde_json::to_vec_pretty(&audit)?,
    )?;
    let cli_build_ms = build.elapsed().as_millis();
    let execution = Instant::now();
    let partial = !subjects.is_empty();
    let native = project
        .run_verification_with_limits(
            Some(&destination),
            VerificationSelection {
                subjects,
                ..Default::default()
            },
            generator,
            cancellation,
            true,
            None,
            NativeLimits {
                initialization: Duration::from_secs(60),
                case: Duration::from_secs(60),
                run: Duration::from_secs(900),
                cleanup: Duration::from_secs(5),
                ..Default::default()
            },
        )
        .await?;
    let source_current = inventory(&root)? == files;
    rust_build::validate(&receipt)?;
    let review_obligations = native
        .evidence
        .run
        .open_obligations
        .iter()
        .filter(|issue| {
            issue.code == "MANUAL_REQUIREMENT"
                || issue.code == "SUITE_INSPECTION_REQUIRED"
                || issue.code == "POLICY_REQUIRES_REVIEW"
        })
        .cloned()
        .collect::<Vec<_>>();
    let selected_machine_complete = source_current
        && native.evidence.run.provenance.incomplete.is_empty()
        && native.evidence.run.required_cases
            == native
                .evidence
                .run
                .cases
                .iter()
                .map(|case| case.id.clone())
                .collect::<BTreeSet<_>>()
        && native.evidence.run.required_cases.len() == native.evidence.run.cases.len()
        && !native.evidence.run.cases.is_empty()
        && native
            .evidence
            .run
            .cases
            .iter()
            .all(|case| case.result == VerificationResult::Pass)
        && native.evidence.run.open_obligations.len() == review_obligations.len();
    let machine_complete = selected_machine_complete && !partial;
    let store = project.store(AccessMode::ReadOnly)?;
    let runtime_measurements = native
        .evidence
        .run
        .artifacts
        .iter()
        .filter(|reference| reference.media_type == "application/vnd.dctx0.runtime-metrics+json")
        .map(|reference| {
            let artifact = store.verification_artifact(&native.evidence.id, &reference.hash)?;
            Ok(serde_json::from_slice(&artifact.contents)?)
        })
        .collect::<Result<Vec<dctx0_target_sdk::WorkerReply>>>()?;
    let report = SelfVerificationReport {
        schema_version: 1,
        source_root: root,
        captured_root: destination.clone(),
        report_path: destination.join(".dctx0ctl/self-verification.json"),
        source_files: files,
        source_current,
        partial,
        preparation_ms,
        preparation_peak_rss_bytes,
        orchestrator_peak_rss_bytes: process_peak_rss(),
        cli_build_ms,
        execution_ms: execution.elapsed().as_millis(),
        cli_hash,
        native,
        machine_complete,
        selected_machine_complete,
        review_obligations,
        runtime_measurements,
    };
    // Output is derived state: it must never invalidate its own input inventory.
    fs::create_dir_all(destination.join(".dctx0ctl"))?;
    fs::write(&report.report_path, serde_json::to_vec_pretty(&report)?)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn self_inventory_captures_embedded_inputs_and_ignores_databases_builds_and_modules() {
        let root =
            std::env::temp_dir().join(format!("dctx0-self-inventory-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        for relative in ROOTS {
            if relative.ends_with(".toml")
                || relative.ends_with(".lock")
                || relative.ends_with(".sh")
                || relative.ends_with(".ps1")
            {
                fs::write(root.join(relative), "fixture").unwrap();
            } else {
                fs::create_dir_all(root.join(relative)).unwrap();
                fs::write(root.join(relative).join("source.txt"), "input").unwrap();
            }
        }
        for excluded in [
            "crates/target/output",
            "editors/node_modules/dep",
            "dctx0/.dctx0ctl/state.sqlite3",
        ] {
            let path = root.join(excluded);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "excluded").unwrap();
        }
        let before = inventory(&root).unwrap();
        assert_eq!(before.len(), ROOTS.len());
        fs::write(root.join("editors/source.txt"), "changed embedded resource").unwrap();
        assert_ne!(before, inventory(&root).unwrap());
        #[cfg(unix)]
        {
            use std::{ffi::OsString, os::unix::ffi::OsStringExt};
            let invalid = root.join("crates").join(OsString::from_vec(vec![0xff]));
            fs::write(&invalid, "uncapturable input").unwrap();
            assert!(
                matches!(inventory(&root), Err(Error::Invalid(message)) if message.contains("UTF-8"))
            );
            fs::remove_file(invalid).unwrap();
            std::os::unix::fs::symlink(root.join("Cargo.toml"), root.join("crates/link")).unwrap();
            assert!(
                matches!(inventory(&root),Err(Error::Invalid(message)) if message.contains("symlink"))
            );
            fs::remove_file(root.join("crates/link")).unwrap();
        }
        fs::remove_file(root.join("Cargo.lock")).unwrap();
        assert!(inventory(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn capture_timestamps_reject_pre_epoch_clocks_without_unsigned_wrapping() {
        assert_eq!(capture_stamp(UNIX_EPOCH).unwrap(), 0);
        assert_eq!(
            capture_stamp(UNIX_EPOCH + Duration::from_nanos(1)).unwrap(),
            1
        );
        assert!(capture_stamp(UNIX_EPOCH - Duration::from_nanos(1)).is_err());
    }
}

#[cfg(all(test, target_os = "linux"))]
mod rejection_tests {
    use super::*;
    #[tokio::test]
    async fn self_verification_rejects_invalid_spec_before_building_or_mutating_original_history() {
        let root = std::env::temp_dir().join(format!("dctx0-self-invalid-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        for relative in ROOTS {
            let path = root.join(relative);
            if relative.ends_with(".toml")
                || relative.ends_with(".lock")
                || relative.ends_with(".sh")
                || relative.ends_with(".ps1")
            {
                fs::write(path, "fixture").unwrap();
            } else {
                fs::create_dir_all(path).unwrap();
            }
        }
        fs::write(root.join("dctx0/w.dctx0"), "workspace W {").unwrap();
        let result = verify_self(
            &root,
            BTreeSet::new(),
            &dctx0_target_rust::RustHarness,
            &dctx0_target_rust::Provider,
            &NativeCancellation::default(),
        )
        .await;
        assert!(
            matches!(result,Err(Error::Invalid(message)) if message.contains("Self specification rejected"))
        );
        assert!(!root.join("dctx0/.dctx0ctl").exists());
        assert!(process_peak_rss().is_some_and(|bytes| bytes > 0));
        fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn a_valid_spec_without_the_required_cli_package_cannot_publish_self_evidence() {
        let root = std::env::temp_dir().join(format!("dctx0-self-no-cli-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        for relative in ROOTS {
            let path = root.join(relative);
            if relative.ends_with(".toml")
                || relative.ends_with(".lock")
                || relative.ends_with(".sh")
                || relative.ends_with(".ps1")
            {
                fs::write(path, "fixture").unwrap();
            } else {
                fs::create_dir_all(path).unwrap();
            }
        }
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers=[\"crates/no-cli\"]\nresolver=\"3\"\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("crates/no-cli/src")).unwrap();
        fs::write(
            root.join("crates/no-cli/Cargo.toml"),
            "[package]\nname=\"no-cli\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
        )
        .unwrap();
        fs::write(
            root.join("crates/no-cli/src/lib.rs"),
            "pub fn actual() {}\n",
        )
        .unwrap();
        fs::remove_file(root.join("Cargo.lock")).unwrap();
        assert!(
            std::process::Command::new("cargo")
                .args(["generate-lockfile", "--offline"])
                .current_dir(&root)
                .status()
                .unwrap()
                .success()
        );
        fs::write(root.join("dctx0/w.dctx0"), "workspace W { value N = Int }").unwrap();
        let result = verify_self(
            &root,
            BTreeSet::new(),
            &dctx0_target_rust::RustHarness,
            &dctx0_target_rust::Provider,
            &NativeCancellation::default(),
        )
        .await;
        assert!(
            matches!(&result, Err(Error::Invalid(message)) if message.starts_with("Build:")),
            "{result:?}"
        );
        assert!(!root.join("dctx0/.dctx0ctl").exists());
        for entry in fs::read_dir(root.join("target/self-verification")).unwrap() {
            assert!(
                !entry
                    .unwrap()
                    .path()
                    .join(".dctx0ctl/self-verification.json")
                    .exists()
            );
        }
        fs::remove_dir_all(root).unwrap();
    }
}
