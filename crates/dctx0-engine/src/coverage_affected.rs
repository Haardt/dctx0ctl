//! Observed overlap candidates, never authority to skip regression tests.
use crate::experimental_coverage::error_text;
use crate::{AccessMode, Project};
use dctx0_store::{CoverageBaseline, Store};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Deserialize)]
struct Diff {
    base_revision: String,
    files: Vec<ChangedFile>,
}
#[cfg(test)]
use dctx0_test_analysis::ChangedRange;
use dctx0_test_analysis::{ChangedFile, old_interval};
#[derive(Debug, Serialize)]
pub struct CoverageAffected {
    pub experimental: bool,
    pub project_root: String,
    pub base_revision: String,
    pub can_skip_other_tests: bool,
    pub baseline_policy: &'static str,
    pub baseline_source_hash: Option<String>,
    pub baseline_environment_hash: Option<String>,
    pub baseline_execution_ids: Vec<i64>,
    pub excluded_execution_count: usize,
    pub candidates: Vec<CoverageCandidate>,
    pub unknowns: Vec<CoverageUnknown>,
    files: Vec<ChangedFile>,
}
#[derive(Debug, Serialize)]
pub struct CoverageCandidate {
    pub test_id: String,
    pub execution_id: i64,
    pub baseline_outcome: String,
    pub command: Option<Vec<String>>,
    pub reasons: Vec<CoverageReason>,
}
#[derive(Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct CoverageReason {
    pub file: String,
    pub old_start: u32,
    pub old_end: u32,
    pub insertion_anchor: bool,
}
#[derive(Debug, Serialize)]
pub struct CoverageUnknown {
    pub file: Option<String>,
    pub code: String,
    pub old_start: Option<u32>,
}
impl CoverageAffected {
    fn unknown(&mut self, file: Option<&str>, code: &str, old_start: Option<u32>) {
        self.unknowns.push(CoverageUnknown {
            file: file.map(str::to_owned),
            code: code.into(),
            old_start,
        });
    }
}
/// Compare the base to staged + unstaged tracked changes and untracked files.
/// Only clean baseline records at that exact commit and one input/environment
/// cohort contribute candidates. This operation performs no writes or test runs.
pub fn affected_project_tests(project: &Project, base: &str) -> Result<CoverageAffected, String> {
    let script = format!(
        "{}\n{}",
        include_str!("coverage_diff.py"),
        include_str!("coverage_collect.py")
    );
    let options = serde_json::json!({"mode":"diff","root":project.root,"base":base});
    let output = std::process::Command::new("python3")
        .arg("-c")
        .arg(script)
        .arg(options.to_string())
        .output()
        .map_err(error_text)?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    let diff: Diff = serde_json::from_slice(&output.stdout).map_err(error_text)?;
    // A first query before any collection must not initialize a database.
    let store = if project.database.exists() {
        Some(project.store(AccessMode::ReadOnly).map_err(error_text)?)
    } else {
        None
    };
    let records = store
        .as_ref()
        .map(|s| s.coverage_baselines(&project.key()))
        .transpose()
        .map_err(error_text)?
        .unwrap_or_default();
    match_ranges(project, store.as_ref(), diff, records)
}
fn match_ranges(
    project: &Project,
    store: Option<&Store>,
    diff: Diff,
    records: Vec<CoverageBaseline>,
) -> Result<CoverageAffected, String> {
    let total = records.len();
    let eligible = |r: &&CoverageBaseline| {
        r.revision.as_deref() == Some(diff.base_revision.as_str())
            && r.dirty == Some(false)
            && r.available == Some(true)
            && r.outcome != "timeout"
            && r.toolchain.is_some()
            && r.source_hash.is_some()
            && r.environment_hash.is_some()
    };
    let cohort = records.iter().find(eligible).map(|r| {
        (
            r.source_hash.clone(),
            r.environment_hash.clone(),
            r.toolchain.clone(),
        )
    });
    let mut seen = BTreeSet::new();
    let selected: Vec<_> = records
        .iter()
        .filter(eligible)
        .filter(|r| {
            Some((
                r.source_hash.clone(),
                r.environment_hash.clone(),
                r.toolchain.clone(),
            )) == cohort
        })
        .filter(|r| {
            seen.insert((
                r.test_id.clone(),
                r.target_kind.clone(),
                r.target_name.clone(),
            ))
        })
        .collect();
    let mut report = CoverageAffected {
        experimental: true,
        project_root: project.key(),
        base_revision: diff.base_revision.clone(),
        can_skip_other_tests: false,
        baseline_policy: "newest clean exact-base source/environment/toolchain cohort; latest execution per test/target",
        baseline_source_hash: cohort.as_ref().and_then(|c| c.0.clone()),
        baseline_environment_hash: cohort.as_ref().and_then(|c| c.1.clone()),
        baseline_execution_ids: selected.iter().map(|r| r.id).collect(),
        excluded_execution_count: total - selected.len(),
        candidates: Vec::new(),
        unknowns: Vec::new(),
        files: Vec::new(),
    };
    if !diff.files.is_empty() {
        report.unknown(
            None,
            "historical_coverage_does_not_prove_new_paths_or_catalog_completeness",
            None,
        );
        if selected.is_empty() {
            report.unknown(None, "no_clean_coverage_at_base_revision", None);
        }
    }
    let project_key = project.key();
    let mut reasons: BTreeMap<i64, BTreeSet<CoverageReason>> = BTreeMap::new();
    for file in &diff.files {
        if file.status == "A" || file.status == "untracked" {
            report.unknown(Some(&file.path), "new_file_has_no_baseline_coverage", None);
            continue;
        }
        if file.binary || file.ranges.is_empty() {
            report.unknown(Some(&file.path), "binary_or_metadata_change", None);
            continue;
        }
        if !file.path.ends_with(".rs")
            || file.path == "build.rs"
            || file.path.ends_with("/build.rs")
        {
            report.unknown(Some(&file.path), "non_rust_or_build_input_change", None);
        }
        for range in &file.ranges {
            let (start, end, insertion) = old_interval(range);
            if insertion {
                report.unknown(
                    Some(&file.path),
                    "inserted_lines_not_observed_in_base",
                    Some(range.old_start),
                );
            }
            let mut any = false;
            let roots: BTreeSet<_> = selected
                .iter()
                .map(|r| r.root.as_deref().unwrap_or(&project_key))
                .collect();
            for root in roots {
                let absolute = std::path::Path::new(root)
                    .join(&file.path)
                    .to_string_lossy()
                    .into_owned();
                let ids: BTreeSet<_> = store
                    .map(|s| s.coverage_range_executions(&absolute, start, end))
                    .transpose()
                    .map_err(error_text)?
                    .unwrap_or_default()
                    .into_iter()
                    .collect();
                for record in &selected {
                    if ids.contains(&record.id) {
                        any = true;
                        reasons
                            .entry(record.id)
                            .or_default()
                            .insert(CoverageReason {
                                file: file.path.clone(),
                                old_start: start,
                                old_end: end,
                                insertion_anchor: insertion,
                            });
                    }
                }
            }
            if !any {
                report.unknown(
                    Some(&file.path),
                    "no_observed_coverage_for_old_range",
                    Some(range.old_start),
                );
            }
        }
    }
    for record in selected {
        if let Some(reasons) = reasons.remove(&record.id) {
            report.candidates.push(CoverageCandidate {
                test_id: record.test_id.clone(),
                execution_id: record.id,
                baseline_outcome: record.outcome.clone(),
                command: test_command(record),
                reasons: reasons.into_iter().collect(),
            });
        }
    }
    report.candidates.sort_by(|a, b| a.test_id.cmp(&b.test_id));
    report.files = diff.files;
    Ok(report)
}

fn test_command(record: &CoverageBaseline) -> Option<Vec<String>> {
    let mut command = vec![
        "cargo".into(),
        "test".into(),
        "--locked".into(),
        "--package".into(),
        record.package_name.clone()?,
    ];
    match record.target_kind.as_deref()? {
        "lib" | "proc-macro" => command.push("--lib".into()),
        kind @ ("test" | "bin" | "example") => {
            command.push(format!("--{kind}"));
            command.push(record.target_name.clone()?);
        }
        _ => return None,
    }
    command.extend(["--".into(), "--exact".into(), record.test_name.clone()?]);
    Some(command)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dctx0_store::CoverageExecution;
    use serde_json::json;
    fn execution(name: &str, environment: &str) -> CoverageExecution {
        CoverageExecution {
            test_id: name.into(),
            outcome: "pass".into(),
            duration_ms: 1.0,
            peak_rss_bytes: None,
            provenance: json!({"revision":"base","dirty_worktree":false,"coverage_available":true,"source_fingerprint":"source","environment_hash":environment,"rustc":"rustc-test","project_root":"/historical","package_name":"example","test_name":name,"test_target_name":"integration","test_target_kind":"test"}),
            llvm_export: json!({"type":"llvm.coverage.json.export","data":[{"files":[{"filename":"/historical/src/lib.rs","segments":[[2,1,1,true,true,false],[5,1,0,true,true,false],[8,1,1,true,true,false],[9,1,0,true,true,false]]}]}]}),
        }
    }
    fn changed(path: &str, status: &str, ranges: Vec<ChangedRange>) -> ChangedFile {
        ChangedFile {
            path: path.into(),
            status: status.into(),
            binary: false,
            ranges,
        }
    }
    fn range(line: u32, count: u32) -> ChangedRange {
        ChangedRange {
            old_start: line,
            old_count: count,
            new_start: line,
            new_count: 1,
        }
    }
    #[test]
    fn candidates_pin_one_baseline_cohort_and_preserve_unknowns_and_history() {
        let mut store =
            Store::open(std::path::Path::new(":memory:"), AccessMode::ReadWrite).unwrap();
        let project = Project {
            root: "/current".into(),
            database: ":memory:".into(),
        };
        let mut dirty = execution("dirty", "current");
        dirty.provenance["dirty_worktree"] = json!(true);
        let mut wrong = execution("wrong-revision", "current");
        wrong.provenance["revision"] = json!("another");
        let mut timeout = execution("timeout", "current");
        timeout.outcome = "timeout".into();
        let mut unavailable = execution("unavailable", "current");
        unavailable.provenance["coverage_available"] = json!(false);
        let mut legacy = execution("legacy", "current");
        legacy.provenance["rustc"] = json!(null);
        let ids = store
            .record_coverage(
                &project.key(),
                &[
                    execution("older-env", "old"),
                    execution("works", "current"),
                    dirty,
                    wrong,
                    timeout,
                    unavailable,
                    legacy,
                    execution("works", "current"),
                ],
            )
            .unwrap();
        let records = store.coverage_baselines(&project.key()).unwrap();
        let report = match_ranges(
            &project,
            Some(&store),
            Diff {
                base_revision: "base".into(),
                files: vec![
                    changed(
                        "src/lib.rs",
                        "M",
                        vec![range(3, 1), range(7, 0), range(100, 1)],
                    ),
                    changed("new.rs", "A", vec![]),
                    changed("untracked.rs", "untracked", vec![]),
                    changed("Cargo.toml", "M", vec![range(1, 1)]),
                    ChangedFile {
                        binary: true,
                        ..changed("data.bin", "M", vec![])
                    },
                    changed("mode.rs", "M", vec![]),
                ],
            },
            records,
        )
        .unwrap();
        assert_eq!(report.baseline_execution_ids, vec![*ids.last().unwrap()]);
        assert_eq!(report.excluded_execution_count, 7);
        assert_eq!(report.candidates.len(), 1);
        assert_eq!(report.candidates[0].test_id, "works");
        assert_eq!(report.candidates[0].reasons.len(), 2);
        assert!(
            report.candidates[0]
                .reasons
                .iter()
                .any(|r| r.insertion_anchor)
        );
        assert_eq!(
            report.candidates[0].command.as_ref().unwrap(),
            &[
                "cargo",
                "test",
                "--locked",
                "--package",
                "example",
                "--test",
                "integration",
                "--",
                "--exact",
                "works"
            ]
        );
        let codes: BTreeSet<_> = report.unknowns.iter().map(|u| u.code.as_str()).collect();
        for code in [
            "new_file_has_no_baseline_coverage",
            "binary_or_metadata_change",
            "non_rust_or_build_input_change",
            "inserted_lines_not_observed_in_base",
            "no_observed_coverage_for_old_range",
        ] {
            assert!(codes.contains(code));
        }
        assert!(!report.can_skip_other_tests);
        assert_eq!(store.coverage_baselines(&project.key()).unwrap().len(), 8);
        store
            .record_coverage(&project.key(), &[execution("new-env", "new")])
            .unwrap();
        let report = match_ranges(
            &project,
            Some(&store),
            Diff {
                base_revision: "base".into(),
                files: vec![changed("src/lib.rs", "D", vec![range(2, 3)])],
            },
            store.coverage_baselines(&project.key()).unwrap(),
        )
        .unwrap();
        assert_eq!(report.candidates[0].test_id, "new-env");
    }
    #[test]
    fn missing_baselines_are_unknown_and_commands_are_explicitly_supported() {
        let project = Project {
            root: "/current".into(),
            database: ":memory:".into(),
        };
        let report = match_ranges(
            &project,
            None,
            Diff {
                base_revision: "base".into(),
                files: vec![changed("src/lib.rs", "M", vec![range(3, 1)])],
            },
            vec![],
        )
        .unwrap();
        assert!(report.candidates.is_empty());
        assert!(
            report
                .unknowns
                .iter()
                .any(|u| u.code == "no_clean_coverage_at_base_revision")
        );
        let report = match_ranges(
            &project,
            None,
            Diff {
                base_revision: "base".into(),
                files: vec![],
            },
            vec![],
        )
        .unwrap();
        assert!(report.unknowns.is_empty());
        let mut store =
            Store::open(std::path::Path::new(":memory:"), AccessMode::ReadWrite).unwrap();
        store
            .record_coverage(&project.key(), &[execution("t", "env")])
            .unwrap();
        let mut record = store.coverage_baselines(&project.key()).unwrap().remove(0);
        for kind in ["lib", "proc-macro", "bin", "example"] {
            record.target_kind = Some(kind.into());
            assert!(test_command(&record).is_some());
        }
        record.target_kind = Some("custom".into());
        assert!(test_command(&record).is_none());
        record.package_name = None;
        assert!(test_command(&record).is_none());
        let first = execution("same-name", "env");
        let mut second = execution("same-name", "env");
        second.provenance["test_target_kind"] = json!("bin");
        store
            .record_coverage(&project.key(), &[first, second])
            .unwrap();
        let report = match_ranges(
            &project,
            Some(&store),
            Diff {
                base_revision: "base".into(),
                files: vec![changed("src/lib.rs", "M", vec![range(3, 1)])],
            },
            store.coverage_baselines(&project.key()).unwrap(),
        )
        .unwrap();
        assert_eq!(
            report
                .candidates
                .iter()
                .filter(|c| c.test_id == "same-name")
                .count(),
            2
        );
    }
}
