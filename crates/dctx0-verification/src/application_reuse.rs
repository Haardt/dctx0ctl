//! Stable application execution identity derived from a managed native receipt.
use crate::{
    ProgramPlan,
    incremental::{self, ReuseBinding},
};
use dctx0_core::VerificationRun;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const APPLICATION_BUNDLE_MEDIA_TYPE: &str =
    "application/vnd.dctx0.application-testing-bundle+json";

/// Both encodings are retained: the original source file and the canonical
/// bundle actually passed to the captured native runner.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationBundleReceipt {
    pub schema_version: u32,
    pub path: String,
    pub raw_bundle: String,
    pub canonical_bundle: String,
}

#[derive(Deserialize)]
struct Receipt {
    plan_hash: String,
    group: String,
    backend: String,
    target: String,
    source_inputs: BTreeMap<String, String>,
    build_inputs: BTreeMap<String, String>,
    materialized_inputs: BTreeMap<String, String>,
    toolchain_inputs: BTreeMap<String, String>,
    phases: Vec<Phase>,
}
#[derive(Deserialize)]
struct Phase {
    phase: String,
    runtime: Runtime,
}
#[derive(Deserialize)]
struct Runtime {
    executable_hash: String,
}
#[derive(Serialize)]
struct Identity<'a> {
    plan_hash: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    bundle_path: Option<&'a str>,
    group: &'a str,
    backend: &'a str,
    target: &'a str,
    source_inputs: &'a BTreeMap<String, String>,
    build_inputs: &'a BTreeMap<String, String>,
    materialized_inputs: &'a BTreeMap<String, String>,
    toolchain_inputs: &'a BTreeMap<String, String>,
    phases: Vec<(&'a str, &'a str)>,
}

/// Reconstruct the identity from the exact receipt bound by native provenance.
pub fn binding(
    program: &ProgramPlan,
    run: &VerificationRun,
    external_hash: String,
    receipt_json: &[u8],
) -> Option<ReuseBinding> {
    if format!("{:x}", Sha256::digest(receipt_json)) != run.provenance.harness_hash {
        return None;
    }
    preview_binding(program, run, external_hash, receipt_json)
}

/// Construct a provisional identity before execution; callers must rederive it
/// from the final receipt and reject publication if it differs.
pub fn preview_binding(
    program: &ProgramPlan,
    run: &VerificationRun,
    external_hash: String,
    receipt_json: &[u8],
) -> Option<ReuseBinding> {
    project_binding(program, run, external_hash, receipt_json, None)
}

pub fn binding_with_bundle(
    program: &ProgramPlan,
    run: &VerificationRun,
    external_hash: String,
    receipt_json: &[u8],
    bundle_json: &[u8],
) -> Option<ReuseBinding> {
    if format!("{:x}", Sha256::digest(receipt_json)) != run.provenance.harness_hash {
        return None;
    }
    project_binding(program, run, external_hash, receipt_json, Some(bundle_json))
}

/// Provisional identity; the final receipt must be independently revalidated.
pub fn preview_binding_with_bundle(
    program: &ProgramPlan,
    run: &VerificationRun,
    external_hash: String,
    receipt_json: &[u8],
    bundle_json: &[u8],
) -> Option<ReuseBinding> {
    project_binding(program, run, external_hash, receipt_json, Some(bundle_json))
}

fn project_binding(
    program: &ProgramPlan,
    run: &VerificationRun,
    external_hash: String,
    receipt_json: &[u8],
    bundle_json: Option<&[u8]>,
) -> Option<ReuseBinding> {
    let receipt: Receipt = serde_json::from_slice(receipt_json).ok()?;
    if receipt.backend != "application-tests"
        || receipt.target != run.provenance.target
        || receipt.plan_hash != run.provenance.adapter_hash
        || receipt.source_inputs != run.provenance.implementation
        || receipt.build_inputs != run.provenance.build_configuration
        || receipt.materialized_inputs != run.provenance.dependencies
    {
        return None;
    }
    let mut source_inputs = receipt.source_inputs.clone();
    let mut build_inputs = receipt.build_inputs.clone();
    let mut materialized_inputs = receipt.materialized_inputs.clone();
    let mut plan_hash = receipt.plan_hash.clone();
    let mut bundle_path = None;
    if let Some(bundle_json) = bundle_json {
        let bundle: ApplicationBundleReceipt = serde_json::from_slice(bundle_json).ok()?;
        let path = std::path::Path::new(&bundle.path);
        if bundle.schema_version != 1
            || bundle.path.is_empty()
            || bundle.path.contains('\\')
            || path
                .components()
                .any(|component| !matches!(component, std::path::Component::Normal(_)))
        {
            return None;
        }
        bundle_path = Some(bundle.path.clone());
        let raw: serde_json::Value = serde_json::from_str(&bundle.raw_bundle).ok()?;
        let mut canonical: serde_json::Value =
            serde_json::from_str(&bundle.canonical_bundle).ok()?;
        if raw != canonical
            || format!("{:x}", Sha256::digest(bundle.canonical_bundle.as_bytes()))
                != receipt.plan_hash
        {
            return None;
        }
        let plan: crate::application_plan::ApplicationTestPlan =
            serde_json::from_value(canonical.get("plan")?.clone()).ok()?;
        if &plan.program != program || plan.validate().is_err() {
            return None;
        }
        let raw_hash = format!("{:x}", Sha256::digest(bundle.raw_bundle.as_bytes()));
        if let Some(hash) = source_inputs.get(&bundle.path) {
            if hash != &raw_hash {
                return None;
            }
            source_inputs.remove(&bundle.path);
        } else if !bundle
            .path
            .split('/')
            .any(|part| matches!(part, ".dctx0ctl" | ".git" | ".runatron" | "target" | ".bin"))
        {
            return None;
        }
        if let Some(hash) = build_inputs.get(&bundle.path) {
            if hash != &receipt.plan_hash {
                return None;
            }
            build_inputs.remove(&bundle.path);
        } else if !bundle
            .path
            .split('/')
            .any(|part| matches!(part, ".dctx0ctl" | ".git" | ".runatron" | "target" | ".bin"))
        {
            return None;
        }
        if let Some(hash) = materialized_inputs.get(&bundle.path) {
            if hash != &receipt.plan_hash {
                return None;
            }
            materialized_inputs.remove(&bundle.path);
        }
        let normalized = canonical
            .get_mut("plan")?
            .get_mut("program")?
            .as_object_mut()?;
        normalized.insert("snapshot".into(), "".into());
        normalized.insert("check_occurrence".into(), 0.into());
        plan_hash = format!("{:x}", Sha256::digest(serde_json::to_vec(&canonical).ok()?));
    }
    let native: Vec<_> = receipt
        .phases
        .iter()
        .filter(|p| p.phase == "native-tests")
        .collect();
    if native.len() != 1 || native[0].runtime.executable_hash.len() != 64 {
        return None;
    }
    let phases = receipt
        .phases
        .iter()
        .map(|p| (p.phase.as_str(), p.runtime.executable_hash.as_str()))
        .collect();
    let identity = Identity {
        plan_hash: &plan_hash,
        bundle_path: bundle_path.as_deref(),
        group: &receipt.group,
        backend: &receipt.backend,
        target: &receipt.target,
        source_inputs: &source_inputs,
        build_inputs: &build_inputs,
        materialized_inputs: &materialized_inputs,
        toolchain_inputs: &receipt.toolchain_inputs,
        phases,
    };
    let identity_hash = format!("{:x}", Sha256::digest(serde_json::to_vec(&identity).ok()?));
    let mut provenance = run.provenance.clone();
    provenance.implementation = source_inputs;
    provenance.build_configuration = build_inputs;
    provenance.dependencies = materialized_inputs;
    provenance.adapter_hash = plan_hash;
    provenance.harness_hash = identity_hash;
    provenance.runtime_version = native[0].runtime.executable_hash.clone();
    incremental::binding(program, provenance, external_hash).ok()
}
