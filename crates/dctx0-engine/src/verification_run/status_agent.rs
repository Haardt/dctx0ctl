//! Read-only freshness for explicitly authored agent checks. A verifier name,
//! PASS label or native run ID alone never establishes a current binding.
use super::*;
use dctx0_store::{Store, TaskEvidenceReference, VerificationRunContext};
use dctx0_tasks::{CheckStatus, Session};
use dctx0_verification::attestations::{AttestationBinding, attestation_matches};
use serde::{
    Deserialize,
    de::{DeserializeSeed, SeqAccess, Visitor},
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{BufReader, Read},
};

const MAX_ARCHIVE_BYTES: u64 = 1 << 30;

#[derive(Deserialize)]
struct AgentReport {
    schema_version: u32,
    basis: String,
    native_run: String,
    inspection: Option<VerificationAttestation>,
    delivered_implementation: Option<String>,
    tested_implementation: Option<String>,
    full_task_inspection_file: Option<String>,
    full_task_inspection_file_sha256: Option<String>,
    inspection_selector: Option<Selector>,
    task: Option<i64>,
    snapshot: Option<String>,
    check_occurrence: Option<i64>,
    requirements_hash: Option<String>,
    profile_bundle_hash: Option<String>,
    source_inventory_artifact: Option<String>,
}
#[derive(Deserialize)]
struct Selector {
    task: i64,
}
struct NativeBinding {
    context: VerificationRunContext,
    scope: BTreeSet<String>,
    current: Option<bool>,
    snapshot: dctx0_changes::Snapshot,
    profiles: LoadedProfiles,
}
#[derive(Clone)]
struct AuthoredBinding {
    actor: String,
    valid: bool,
}
type ArchiveKey = (String, String, String);
type ArchiveBindings = Option<BTreeMap<i64, AuthoredBinding>>;

pub(super) struct AgentFreshness<'a> {
    project: &'a Project,
    store: &'a Store,
    session: &'a Session,
    root: Option<&'a Path>,
    inputs: Option<&'a BTreeMap<String, String>>,
    spec_current: Option<bool>,
    native: BTreeMap<String, Option<NativeBinding>>,
    archives: BTreeMap<ArchiveKey, ArchiveBindings>,
    compositions: BTreeMap<String, Option<super::composition_attestations::ComposedBinding>>,
}
impl<'a> AgentFreshness<'a> {
    pub(super) fn new(
        project: &'a Project,
        store: &'a Store,
        session: &'a Session,
        root: Option<&'a Path>,
        inputs: Option<&'a BTreeMap<String, String>>,
        spec_current: Option<bool>,
    ) -> Self {
        Self {
            project,
            store,
            session,
            root,
            inputs,
            spec_current,
            native: BTreeMap::new(),
            archives: BTreeMap::new(),
            compositions: BTreeMap::new(),
        }
    }
    fn native_binding(&self, id: &str) -> Option<NativeBinding> {
        let key = self.project.key();
        let scope = self.store.complete_native_task_scope(&key, id).ok()??;
        let context = self.store.verification_run_context(&key, id).ok()??;
        if !context.provenance.incomplete.is_empty() {
            return None;
        }
        let root = self.root?;
        let profiles = self
            .project
            .profiles(Some(root), Some(&context.provenance.target))
            .ok()
            .flatten()?;
        let snapshot = self.project.load(&context.provenance.snapshot).ok()?;
        let executed_spec_current =
            self.project.sources().ok().is_some_and(|sources| {
                dctx0_changes::source_hash(&sources) == snapshot.source_hash
            });
        let sources =
            super::status::sources_current(self.store, id, &context, self.inputs, self.root)
                .ok()?;
        let requirements = super::status::requirements_current(&context, self.session);
        let current = match (
            sources,
            requirements,
            self.spec_current.filter(|_| executed_spec_current),
        ) {
            (Some(false), _, _) | (_, Some(false), _) | (_, _, Some(false)) => Some(false),
            (Some(true), Some(true), Some(true)) => Some(true),
            _ => None,
        };
        let current = if executed_spec_current {
            current
        } else {
            Some(false)
        };
        Some(NativeBinding {
            context,
            scope,
            current,
            snapshot,
            profiles,
        })
    }
    pub(super) fn current(&mut self, reference: &TaskEvidenceReference) -> Option<bool> {
        let evidence = self
            .store
            .task_check_evidence_at(
                &self.project.key(),
                reference.task_id,
                self.session.event_seq,
            )
            .ok()??;
        if !matches!(
            evidence.verifier_version.as_str(),
            "dctx0-attestation-1" | "dctx0-agent-revalidation-1" | "dctx0-composition-inspection-1"
        ) || evidence.verifier != reference.verifier
            || evidence.status != CheckStatus::Pass
            || evidence.requirements_hash != reference.requirements_hash
            || evidence.implementation != reference.implementation
            || format!("{:x}", Sha256::digest(evidence.report.as_bytes())) != reference.report_hash
        {
            return None;
        }
        if evidence.verifier_version == "dctx0-composition-inspection-1" {
            return self.composed_current(reference, &evidence);
        }
        let report: AgentReport = serde_json::from_str(&evidence.report).ok()?;
        if report.schema_version != 1 || report.native_run.is_empty() {
            return None;
        }
        if !self.native.contains_key(&report.native_run) {
            let native = self.native_binding(&report.native_run);
            self.native.insert(report.native_run.clone(), native);
        }
        let native = self.native[&report.native_run].as_ref()?;
        let tested = dctx0_changes::hash(&native.context.provenance);
        let revalidation = evidence.verifier_version == "dctx0-agent-revalidation-1";
        if revalidation {
            let delivered = self
                .session
                .states
                .get(&reference.task_id)?
                .result
                .as_ref()?
                .implementation
                .as_ref()?;
            if report.basis != "agent-revalidation"
                || report.delivered_implementation.as_ref() != Some(delivered)
                || delivered != &reference.implementation
                || report.tested_implementation.as_ref() != Some(&tested)
            {
                return None;
            }
        } else if reference.implementation != tested {
            return None;
        }
        let member = self.session.member(reference.task_id).ok()?;
        let candidate = &member.candidate;
        let pin = native.context.provenance.task_pin.as_ref()?;
        if pin.requirements.get(&reference.task_id) != Some(&candidate.requirements_hash)
            || !candidate
                .subjects
                .iter()
                .chain(&candidate.types)
                .all(|s| native.scope.contains(s))
        {
            return None;
        }
        let valid = if let Some(inspection) = &report.inspection {
            (report.basis == "agent" || (revalidation && report.basis == "agent-revalidation"))
                && report.full_task_inspection_file.is_none()
                && bound(inspection, reference.task_id, candidate, &native.context)
                && inspection.actor == reference.verifier
        } else {
            if revalidation
                || report.basis != "agent inspection with complete actual native corpus"
                || report.task != Some(reference.task_id)
                || report.snapshot.as_ref() != Some(&native.context.provenance.snapshot)
                || report.check_occurrence != Some(native.context.provenance.check_occurrence)
                || report.requirements_hash.as_ref() != Some(&candidate.requirements_hash)
                || report.profile_bundle_hash.as_ref()
                    != pin.profile_bundles.get(&reference.task_id)
                || !native.context.artifacts.iter().any(|a| {
                    a.media_type == "application/vnd.dctx0.captured-input-inventory+json"
                        && Some(&a.hash) == report.source_inventory_artifact.as_ref()
                })
                || report.inspection_selector.as_ref()?.task != reference.task_id
            {
                return None;
            }
            let file = report.full_task_inspection_file.as_ref()?;
            let hash = report.full_task_inspection_file_sha256.as_ref()?;
            let key = (report.native_run.clone(), file.clone(), hash.clone());
            if !self.archives.contains_key(&key) {
                let bindings =
                    read_archive(self.root?, file, hash, self.session, &native.context).ok();
                self.archives.insert(key.clone(), bindings);
            }
            self.archives[&key]
                .as_ref()?
                .get(&reference.task_id)
                .is_some_and(|b| b.valid && b.actor == reference.verifier)
        };
        if !valid {
            return None;
        }
        let selected = dctx0_tasks::select_profiles(
            &native.snapshot,
            candidate,
            &native.profiles.manifest,
            &native.profiles.documents,
        );
        selected.require_complete().ok()?;
        if pin.profile_bundles.get(&reference.task_id) != Some(&selected.bundle.hash) {
            return Some(false);
        }
        native.current
    }

    fn composed_current(
        &mut self,
        reference: &TaskEvidenceReference,
        evidence: &dctx0_tasks::CheckEvidence,
    ) -> Option<bool> {
        let report: dctx0_store::ComposedTaskReport =
            serde_json::from_str(&evidence.report).ok()?;
        if report.schema_version != 1
            || report.basis != "native-evidence-composition+authored-inspection"
        {
            return None;
        }
        let key = dctx0_changes::hash(&(
            &report.composition,
            &report.unit_configuration,
            &report.input_hashes,
        ));
        if !self.compositions.contains_key(&key) {
            let binding = super::composition_attestations::current_task_binding(
                self.project,
                self.store,
                self.root?,
                &report,
            )
            .ok();
            self.compositions.insert(key.clone(), binding);
        }
        let binding = self.compositions[&key].as_ref()?;
        if !binding.current || self.spec_current == Some(false) {
            return Some(false);
        }
        if self.spec_current != Some(true) {
            return None;
        }
        let member = self.session.member(reference.task_id).ok()?;
        let candidate = &member.candidate;
        let subject = format!("T{}", reference.task_id);
        let requirements = candidate
            .subjects
            .iter()
            .chain(&candidate.types)
            .chain(&candidate.facts)
            .cloned()
            .collect();
        let plan_hash = dctx0_verification::unit_planner::evidence_digest(&binding.program).ok()?;
        let identity =
            dctx0_changes::hash(&(&report.composition, &plan_hash, &binding.implementation));
        if report.inspection.actor != reference.verifier
            || report.inspection.result != VerificationResult::Pass
            || i64::try_from(binding.program.check_occurrence).ok() != Some(report.check_occurrence)
            || identity != reference.implementation
            || !attestation_matches(
                &report.inspection,
                &AttestationBinding {
                    subject: &subject,
                    snapshot: &binding.program.snapshot,
                    plan_hash: &plan_hash,
                    requirements: &requirements,
                    implementation_artifacts: &binding.implementation,
                },
            )
        {
            return None;
        }
        let selected = dctx0_tasks::select_profiles(
            &binding.snapshot,
            candidate,
            &binding.profiles.manifest,
            &binding.profiles.documents,
        );
        selected.require_complete().ok()?;
        if selected.bundle.hash != report.profile_bundle_hash {
            return Some(false);
        }
        Some(true)
    }
}
fn bound(
    inspection: &VerificationAttestation,
    task: i64,
    candidate: &dctx0_tasks::Candidate,
    context: &VerificationRunContext,
) -> bool {
    let subject = format!("T{task}");
    let requirements = candidate
        .subjects
        .iter()
        .chain(&candidate.types)
        .chain(&candidate.facts)
        .cloned()
        .collect();
    inspection.result == VerificationResult::Pass
        && attestation_matches(
            inspection,
            &AttestationBinding {
                subject: &subject,
                snapshot: &context.provenance.snapshot,
                plan_hash: &context.provenance.plan_hash,
                requirements: &requirements,
                implementation_artifacts: &context.provenance.implementation,
            },
        )
}

// Project each archive record once and discard its duplicate source inventory.
// Peak memory is one authored report, not 600 complete copies of the inventory.
struct ArchiveSeed<'a> {
    session: &'a Session,
    context: &'a VerificationRunContext,
}
impl<'de> DeserializeSeed<'de> for ArchiveSeed<'_> {
    type Value = BTreeMap<i64, AuthoredBinding>;
    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        d: D,
    ) -> std::result::Result<Self::Value, D::Error> {
        d.deserialize_seq(self)
    }
}
impl<'de> Visitor<'de> for ArchiveSeed<'_> {
    type Value = BTreeMap<i64, AuthoredBinding>;
    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("an array of exact task inspections")
    }
    fn visit_seq<A: SeqAccess<'de>>(
        self,
        mut seq: A,
    ) -> std::result::Result<Self::Value, A::Error> {
        let mut result = BTreeMap::new();
        while let Some(item) = seq.next_element::<TaskAttestation>()? {
            let valid = self
                .session
                .member(item.task)
                .is_ok_and(|m| bound(&item.inspection, item.task, &m.candidate, self.context));
            if result
                .insert(
                    item.task,
                    AuthoredBinding {
                        actor: item.inspection.actor,
                        valid,
                    },
                )
                .is_some()
            {
                return Err(serde::de::Error::custom(
                    "duplicate archived task inspection",
                ));
            }
        }
        Ok(result)
    }
}
struct HashingRead<R> {
    input: R,
    hash: Sha256,
    bytes: u64,
}
impl<R: Read> Read for HashingRead<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let n = self.input.read(buffer)?;
        self.hash.update(&buffer[..n]);
        self.bytes += n as u64;
        Ok(n)
    }
}
fn read_archive(
    root: &Path,
    relative: &str,
    expected_hash: &str,
    session: &Session,
    context: &VerificationRunContext,
) -> Result<BTreeMap<i64, AuthoredBinding>> {
    if expected_hash.len() != 64 || !expected_hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::Invalid("Invalid inspection archive digest".into()));
    }
    let (_, path) = crate::verification_setup::safe_file(root, relative)?;
    let file = fs::File::open(path)?;
    if file.metadata()?.len() > MAX_ARCHIVE_BYTES {
        return Err(Error::Invalid(
            "Inspection archive exceeds status bound".into(),
        ));
    }
    let mut reader = HashingRead {
        input: BufReader::new(file.take(MAX_ARCHIVE_BYTES + 1)),
        hash: Sha256::new(),
        bytes: 0,
    };
    let mut deserializer = serde_json::Deserializer::from_reader(&mut reader);
    let bindings = ArchiveSeed { session, context }.deserialize(&mut deserializer)?;
    deserializer.end()?;
    if reader.bytes > MAX_ARCHIVE_BYTES || format!("{:x}", reader.hash.finalize()) != expected_hash
    {
        return Err(Error::Invalid("Inspection archive digest mismatch".into()));
    }
    Ok(bindings)
}
