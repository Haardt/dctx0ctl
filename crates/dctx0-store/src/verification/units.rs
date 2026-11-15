//! Additive unit evidence projections. Original run bodies remain authoritative.
use super::*;
use dctx0_core::*;
use dctx0_verification::unit_planner::{self, ValidatedUnitEvidence};
use std::collections::BTreeMap;

pub(crate) const MIGRATION: &str = "
CREATE TABLE verification_unit_bindings(project TEXT NOT NULL REFERENCES projects(project),unit TEXT NOT NULL,fingerprint TEXT NOT NULL,data TEXT NOT NULL,PRIMARY KEY(project,unit,fingerprint));
CREATE TABLE verification_unit_dependencies(project TEXT NOT NULL,unit TEXT NOT NULL,fingerprint TEXT NOT NULL,dependency TEXT NOT NULL,hash TEXT NOT NULL,PRIMARY KEY(project,unit,fingerprint,dependency),FOREIGN KEY(project,unit,fingerprint) REFERENCES verification_unit_bindings(project,unit,fingerprint));
CREATE INDEX verification_dependency_units ON verification_unit_dependencies(project,dependency,unit,fingerprint);
CREATE TABLE verification_unit_attempts(generation INTEGER PRIMARY KEY AUTOINCREMENT,project TEXT NOT NULL,unit TEXT NOT NULL,fingerprint TEXT NOT NULL,outcome TEXT NOT NULL CHECK(outcome IN ('started','pass','fail','unknown')),FOREIGN KEY(project,unit,fingerprint) REFERENCES verification_unit_bindings(project,unit,fingerprint));
CREATE INDEX verification_latest_unit_attempt ON verification_unit_attempts(project,unit,fingerprint,generation DESC);
CREATE TABLE verification_unit_origins(project TEXT NOT NULL,unit TEXT NOT NULL,fingerprint TEXT NOT NULL,generation INTEGER NOT NULL UNIQUE REFERENCES verification_unit_attempts(generation),native_run TEXT NOT NULL REFERENCES verification_runs(id),proof TEXT NOT NULL,PRIMARY KEY(project,unit,fingerprint,generation),FOREIGN KEY(project,unit,fingerprint) REFERENCES verification_unit_bindings(project,unit,fingerprint));
CREATE INDEX verification_unit_origin_candidates ON verification_unit_origins(project,unit,fingerprint,generation DESC);
CREATE TABLE verification_evidence_compositions(id TEXT PRIMARY KEY,project TEXT NOT NULL REFERENCES projects(project),data TEXT NOT NULL);
CREATE TABLE verification_composition_cases(composition TEXT NOT NULL REFERENCES verification_evidence_compositions(id),case_id TEXT NOT NULL,unit TEXT NOT NULL,native_run TEXT REFERENCES verification_runs(id),coverage TEXT NOT NULL,PRIMARY KEY(composition,case_id));
CREATE INDEX verification_composition_native_sources ON verification_composition_cases(native_run,composition);
CREATE TABLE verification_active_sources(owner TEXT NOT NULL,project TEXT NOT NULL REFERENCES projects(project),native_run TEXT NOT NULL REFERENCES verification_runs(id),PRIMARY KEY(owner,project,native_run));
CREATE INDEX verification_source_holds ON verification_active_sources(native_run);
PRAGMA user_version=8;";

fn invalid(message: impl Into<String>) -> Error {
    Error::Verification(message.into())
}
fn generation(value: u64) -> Result<i64> {
    i64::try_from(value).map_err(|_| invalid("UNIT_GENERATION_OUT_OF_RANGE"))
}
fn outcome(value: UnitAttemptOutcome) -> &'static str {
    match value {
        UnitAttemptOutcome::Started => "started",
        UnitAttemptOutcome::Pass => "pass",
        UnitAttemptOutcome::Fail => "fail",
        UnitAttemptOutcome::Unknown => "unknown",
    }
}
fn require_schema(c: &Connection) -> Result<()> {
    if crate::storage::version(c)? < 8 {
        return Err(invalid(
            "Unit evidence requires schema 8; convert legacy storage first",
        ));
    }
    Ok(())
}
fn register_binding(
    c: &Connection,
    project: &str,
    binding: &UnitEvidenceBindingV1,
) -> Result<String> {
    if !unit_planner::valid_unit_binding(binding) {
        return Err(invalid("UNIT_BINDING_INVALID"));
    }
    let fingerprint = unit_planner::evidence_digest(binding)?;
    c.execute(
        "INSERT OR IGNORE INTO verification_unit_bindings VALUES(?1,?2,?3,?4)",
        params![
            project,
            binding.unit.id,
            fingerprint,
            serde_json::to_string(binding)?
        ],
    )?;
    for (path, hash) in &binding.dependencies {
        c.execute(
            "INSERT OR IGNORE INTO verification_unit_dependencies VALUES(?1,?2,?3,?4,?5)",
            params![project, binding.unit.id, fingerprint, path, hash],
        )?;
    }
    Ok(fingerprint)
}

pub(crate) fn validate_origin(
    c: &Connection,
    project: &str,
    binding: UnitEvidenceBindingV1,
    proof: UnitCompletionProofV1,
) -> Result<ValidatedUnitEvidence> {
    let saved = codec::read_run(c, project, &proof.native_run)?;
    let mut artifacts = BTreeMap::new();
    for hash in &proof.artifacts {
        artifacts.insert(
            hash.clone(),
            codec::read_artifact(c, &proof.native_run, hash)?.contents,
        );
    }
    unit_planner::native::validate_native_unit(binding, proof, &saved.id, &saved.run, &artifacts)
        .map_err(invalid)
}

/// Revalidate a composition inside the caller's publication transaction. The
/// caller separately captures current filesystem/environment bindings; equality
/// here prevents publishing a historical composition under different inputs.
pub(crate) fn validate_current_composition(
    c: &Connection,
    project: &str,
    id: &str,
    current: &[UnitEvidenceBindingV1],
    required: &BTreeSet<String>,
) -> Result<EvidenceCompositionV1> {
    require_schema(c)?;
    let data: String = c.query_row(
        "SELECT data FROM verification_evidence_compositions WHERE project=?1 AND id=?2",
        params![project, id],
        |row| row.get(0),
    )?;
    let composition: EvidenceCompositionV1 = serde_json::from_str(&data)?;
    if unit_planner::evidence_digest(&(project, &composition))? != id {
        return Err(invalid("COMPOSITION_DIGEST_INVALID"));
    }
    unit_planner::validate_composition(&composition).map_err(invalid)?;
    unit_planner::validate_unit_partition(required, current.iter().map(|b| &b.unit))
        .map_err(invalid)?;
    if &composition.required_cases != required || composition.units.len() != current.len() {
        return Err(invalid("COMPOSITION_CURRENT_SCOPE_CHANGED"));
    }
    for entry in &composition.units {
        if !matches!(
            entry.coverage,
            UnitCoverage::ReusedPass | UnitCoverage::FreshPass
        ) || !current.iter().any(|binding| binding == &entry.binding)
        {
            return Err(invalid("COMPOSITION_CURRENT_BINDING_OR_OUTCOME_INVALID"));
        }
        let proof = entry
            .proof
            .as_ref()
            .ok_or_else(|| invalid("COMPOSITION_PROOF_MISSING"))?;
        validate_origin(c, project, entry.binding.clone(), proof.clone())?;
        let stored: Option<String> = c.query_row(
            "SELECT o.proof FROM verification_unit_origins o JOIN verification_unit_attempts a USING(generation) WHERE o.project=?1 AND o.unit=?2 AND o.fingerprint=?3 AND o.generation=?4 AND a.outcome='pass' AND NOT EXISTS(SELECT 1 FROM verification_unit_attempts newer WHERE newer.project=o.project AND newer.unit=o.unit AND newer.fingerprint=o.fingerprint AND newer.generation>o.generation)",
            params![project, proof.unit, proof.fingerprint, generation(proof.generation)?],
            |row| row.get(0),
        ).optional()?;
        if stored.as_deref() != Some(serde_json::to_string(proof)?.as_str()) {
            return Err(invalid("COMPOSITION_ORIGIN_STALE"));
        }
    }
    Ok(composition)
}

impl Store {
    /// Load one indexed closure recipe, not historical case bodies. This is a
    /// planning hint; positive candidates still validate their original artifacts.
    pub fn latest_unit_binding(
        &self,
        project: &str,
        unit: &str,
    ) -> Result<Option<UnitEvidenceBindingV1>> {
        if crate::storage::version(&self.connection)? < 8 {
            return Ok(None);
        }
        let data: Option<String> = self.connection.query_row("SELECT b.data FROM verification_unit_origins o JOIN verification_unit_bindings b USING(project,unit,fingerprint) WHERE o.project=?1 AND o.unit=?2 ORDER BY o.generation DESC LIMIT 1",params![project,unit],|row|row.get(0)).optional()?;
        data.map(|data| serde_json::from_str(&data).map_err(Error::from))
            .transpose()
    }
    /// Commit intent before process startup. An interrupted attempt remains visible.
    pub fn begin_unit_attempt(
        &mut self,
        project: &str,
        binding: &UnitEvidenceBindingV1,
    ) -> Result<UnitAttemptV1> {
        self.require_write()?;
        require_schema(&self.connection)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let fingerprint = register_binding(&tx, project, binding)?;
        tx.execute("INSERT INTO verification_unit_attempts(project,unit,fingerprint,outcome) VALUES(?1,?2,?3,'started')",
            params![project,binding.unit.id,fingerprint])?;
        let generation = tx.last_insert_rowid() as u64;
        tx.commit()?;
        Ok(UnitAttemptV1 {
            unit: binding.unit.id.clone(),
            fingerprint,
            generation,
            outcome: UnitAttemptOutcome::Started,
        })
    }

    /// Failed and incomplete attempts cannot later be rewritten into PASS.
    /// PASS is recorded only together with independently validated native proof.
    pub fn finish_nonpass_unit_attempt(
        &mut self,
        project: &str,
        attempt: &UnitAttemptV1,
        result: UnitAttemptOutcome,
    ) -> Result<()> {
        self.require_write()?;
        require_schema(&self.connection)?;
        if !matches!(
            result,
            UnitAttemptOutcome::Fail | UnitAttemptOutcome::Unknown
        ) {
            return Err(invalid("Invalid nonpass outcome"));
        }
        let changed = self.connection.execute("UPDATE verification_unit_attempts SET outcome=?5 WHERE project=?1 AND unit=?2 AND fingerprint=?3 AND generation=?4 AND outcome='started'",
            params![project,attempt.unit,attempt.fingerprint,generation(attempt.generation)?,outcome(result)])?;
        if changed != 1 {
            return Err(invalid("UNIT_ATTEMPT_CONFLICT"));
        }
        Ok(())
    }

    pub fn unit_attempts(
        &self,
        project: &str,
        unit: &str,
        fingerprint: &str,
    ) -> Result<Vec<UnitAttemptV1>> {
        require_schema(&self.connection)?;
        let mut statement = self.connection.prepare("SELECT generation,outcome FROM verification_unit_attempts WHERE project=?1 AND unit=?2 AND fingerprint=?3 ORDER BY generation DESC")?;
        let rows = statement.query_map(params![project, unit, fingerprint], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.map(|row| {
            let (generation, status) = row?;
            let result = match status.as_str() {
                "started" => UnitAttemptOutcome::Started,
                "pass" => UnitAttemptOutcome::Pass,
                "fail" => UnitAttemptOutcome::Fail,
                "unknown" => UnitAttemptOutcome::Unknown,
                _ => return Err(invalid("Invalid attempt outcome")),
            };
            Ok(UnitAttemptV1 {
                unit: unit.into(),
                fingerprint: fingerprint.into(),
                generation: generation as u64,
                outcome: result,
            })
        })
        .collect()
    }

    /// Persist retention holds before loading native bodies or starting processes.
    pub fn retain_unit_sources(
        &mut self,
        project: &str,
        owner: &str,
        sources: &BTreeSet<String>,
    ) -> Result<()> {
        self.require_write()?;
        require_schema(&self.connection)?;
        if owner.is_empty() {
            return Err(invalid("Empty evidence owner"));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        for source in sources {
            let exists: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM verification_runs WHERE project=?1 AND id=?2 AND pruned_at IS NULL)",params![project,source], |r| r.get(0))?;
            if !exists {
                return Err(invalid("Native source unavailable for retention"));
            }
            tx.execute(
                "INSERT OR IGNORE INTO verification_active_sources VALUES(?1,?2,?3)",
                params![owner, project, source],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Original run/artifact hashes are checked again on every positive selection.
    pub fn validate_unit_origin(
        &self,
        project: &str,
        binding: UnitEvidenceBindingV1,
        proof: UnitCompletionProofV1,
    ) -> Result<ValidatedUnitEvidence> {
        require_schema(&self.connection)?;
        let tx = self.connection.unchecked_transaction()?;
        let validated = validate_origin(&tx, project, binding, proof)?;
        tx.commit()?;
        Ok(validated)
    }

    pub fn record_unit_origin(
        &mut self,
        project: &str,
        binding: UnitEvidenceBindingV1,
        proof: UnitCompletionProofV1,
    ) -> Result<()> {
        self.require_write()?;
        require_schema(&self.connection)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let candidate = validate_origin(&tx, project, binding, proof)?;
        let binding = candidate.binding();
        let proof = candidate.proof();
        let changed = tx.execute("UPDATE verification_unit_attempts SET outcome='pass' WHERE project=?1 AND unit=?2 AND fingerprint=?3 AND generation=?4 AND outcome='started'",
            params![project,binding.unit.id,proof.fingerprint,generation(proof.generation)?])?;
        if changed != 1 {
            return Err(invalid("UNIT_ATTEMPT_CONFLICT"));
        }
        let held: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM verification_active_sources WHERE project=?1 AND native_run=?2)",params![project,proof.native_run], |r| r.get(0))?;
        if !held {
            return Err(invalid("UNIT_NATIVE_SOURCE_NOT_RETAINED"));
        }
        tx.execute(
            "INSERT INTO verification_unit_origins VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                project,
                binding.unit.id,
                proof.fingerprint,
                generation(proof.generation)?,
                proof.native_run,
                serde_json::to_string(proof)?
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Index results are candidates only; each original is revalidated below.
    pub fn unit_evidence_candidates(
        &self,
        project: &str,
        unit: &str,
        fingerprint: &str,
    ) -> Result<Vec<ValidatedUnitEvidence>> {
        require_schema(&self.connection)?;
        let mut statement = self.connection.prepare("SELECT b.data,o.proof FROM verification_unit_origins o JOIN verification_unit_bindings b USING(project,unit,fingerprint) JOIN verification_unit_attempts a USING(generation) WHERE o.project=?1 AND o.unit=?2 AND o.fingerprint=?3 AND a.outcome='pass' AND o.generation=(SELECT max(newer.generation) FROM verification_unit_attempts newer WHERE newer.project=o.project AND newer.unit=o.unit AND newer.fingerprint=o.fingerprint)")?;
        let rows = statement
            .query_map(params![project, unit, fingerprint], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(binding, proof)| {
                self.validate_unit_origin(
                    project,
                    serde_json::from_str(&binding)?,
                    serde_json::from_str(&proof)?,
                )
            })
            .collect()
    }

    /// Store technical coverage separately from task delivery. Current task,
    /// profile, claim and review guards remain mandatory for task publication.
    pub fn record_evidence_composition(
        &mut self,
        project: &str,
        composition: &EvidenceCompositionV1,
    ) -> Result<String> {
        self.require_write()?;
        require_schema(&self.connection)?;
        unit_planner::validate_composition(composition).map_err(invalid)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        for unit in &composition.units {
            if let Some(proof) = &unit.proof {
                validate_origin(&tx, project, unit.binding.clone(), proof.clone())?;
                let stored: Option<String> = tx.query_row("SELECT o.proof FROM verification_unit_origins o JOIN verification_unit_attempts a USING(generation) WHERE o.project=?1 AND o.unit=?2 AND o.fingerprint=?3 AND o.generation=?4 AND a.outcome='pass' AND NOT EXISTS(SELECT 1 FROM verification_unit_attempts newer WHERE newer.project=o.project AND newer.unit=o.unit AND newer.fingerprint=o.fingerprint AND newer.generation>o.generation)",
                    params![project,proof.unit,proof.fingerprint,generation(proof.generation)?],|row|row.get(0)).optional()?;
                if stored.as_deref() != Some(serde_json::to_string(proof)?.as_str()) {
                    return Err(invalid("COMPOSITION_ORIGIN_STALE"));
                }
            }
        }
        let id = unit_planner::evidence_digest(&(project, composition))?;
        tx.execute(
            "INSERT OR IGNORE INTO verification_evidence_compositions VALUES(?1,?2,?3)",
            params![id, project, serde_json::to_string(composition)?],
        )?;
        for unit in &composition.units {
            for case in &unit.binding.unit.cases {
                tx.execute(
                    "INSERT OR IGNORE INTO verification_composition_cases VALUES(?1,?2,?3,?4,?5)",
                    params![
                        id,
                        case,
                        unit.binding.unit.id,
                        unit.proof.as_ref().map(|p| p.native_run.as_str()),
                        serde_json::to_string(&unit.coverage)?
                    ],
                )?;
            }
        }
        tx.commit()?;
        Ok(id)
    }

    /// Reading historical technical coverage does not assert current bindings.
    pub fn evidence_composition(&self, project: &str, id: &str) -> Result<EvidenceCompositionV1> {
        require_schema(&self.connection)?;
        let data: String = self.connection.query_row(
            "SELECT data FROM verification_evidence_compositions WHERE project=?1 AND id=?2",
            params![project, id],
            |row| row.get(0),
        )?;
        let composition: EvidenceCompositionV1 = serde_json::from_str(&data)?;
        if unit_planner::evidence_digest(&(project, &composition))? != id {
            return Err(invalid("COMPOSITION_DIGEST_INVALID"));
        }
        unit_planner::validate_composition(&composition).map_err(invalid)?;
        Ok(composition)
    }
}
