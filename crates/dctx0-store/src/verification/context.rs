//! Small saved execution context; never decompress the case corpus for status.
use super::*;
use dctx0_core::VerificationProvenance;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationRunContext {
    pub provenance: VerificationProvenance,
    pub artifacts: Vec<VerificationArtifactRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskEvidenceReference {
    pub task_id: i64,
    pub requirements_hash: String,
    pub implementation: String,
    pub report_hash: String,
    pub status: String,
    pub verifier: String,
    #[serde(default)]
    pub native_run: Option<String>,
    #[serde(default)]
    pub inspection_artifact: Option<String>,
}

impl Store {
    /// Load only the latest check body for one task at the requested watermark.
    /// Metadata readers need not expand every historical task state.
    pub fn task_check_evidence_at(
        &self,
        project: &str,
        task: i64,
        event_sequence: i64,
    ) -> Result<Option<dctx0_tasks::CheckEvidence>> {
        let raw: Option<String> = self.connection.query_row(
            "SELECT state_data FROM task_events WHERE project=?1 AND task_id=?2 AND state_data IS NOT NULL AND seq<=?3 ORDER BY seq DESC LIMIT 1",
            params![project, task, event_sequence], |row| row.get(0),
        ).optional()?;
        raw.map(|raw| {
            let state: dctx0_tasks::State = crate::storage::load(&self.connection, &raw)?;
            Ok(state.evidence.last().cloned())
        })
        .transpose()
        .map(Option::flatten)
    }

    /// Verify the retained native header and complete finite inventory without
    /// decompressing case bodies. Free-text manual obligations remain separate.
    pub fn complete_native_task_scope(
        &self,
        project: &str,
        run: &str,
    ) -> Result<Option<BTreeSet<String>>> {
        let summary = self.verification_run_summary(project, run)?;
        if summary.origin != dctx0_core::VerificationRunOrigin::Native
            || !summary.details_available
            || !summary.case_inventory_complete
            || summary.required_case_count == 0
            || summary.case_count != summary.required_case_count
            || summary.missing_case_count != 0
            || summary.outcomes != [summary.required_case_count as u64, 0, 0]
        {
            return Ok(None);
        }
        #[derive(Deserialize)]
        struct Gate {
            origin: dctx0_core::VerificationRunOrigin,
            required_subjects: BTreeSet<String>,
            open_obligations: Vec<dctx0_core::VerificationIssue>,
            result: dctx0_core::VerificationResult,
        }
        let raw: String = self.connection.query_row(
            "SELECT json_object('origin',json_extract(data,'$.origin'),'required_subjects',json_extract(data,'$.required_subjects'),'open_obligations',json_extract(data,'$.open_obligations'),'result',json_extract(data,'$.result')) FROM verification_runs WHERE project=?1 AND id=?2",
            params![project, run], |row| row.get(0),
        )?;
        let gate: Gate = serde_json::from_str(&raw)?;
        let manual = |issue: &dctx0_core::VerificationIssue| {
            matches!(
                issue.code.as_str(),
                "MANUAL_REQUIREMENT" | "SUITE_INSPECTION_REQUIRED"
            )
        };
        let permitted = match &gate.result {
            dctx0_core::VerificationResult::Pass => gate.open_obligations.is_empty(),
            dctx0_core::VerificationResult::Unknown { issue } => {
                manual(issue)
                    && gate.open_obligations.contains(issue)
                    && gate.open_obligations.iter().all(manual)
            }
            dctx0_core::VerificationResult::Fail { .. } => false,
        };
        Ok(
            (permitted && gate.origin == summary.origin && gate.result == summary.result)
                .then_some(gate.required_subjects),
        )
    }

    pub fn verification_report_runs(
        &self,
        project: &str,
        report_hashes: &BTreeSet<String>,
    ) -> Result<std::collections::BTreeMap<String, String>> {
        let mut found = std::collections::BTreeMap::new();
        if !self.has_verification_history()? || report_hashes.is_empty() {
            return Ok(found);
        }
        let mut statement = self
            .connection
            .prepare("SELECT id FROM verification_runs WHERE project=?1")?;
        for row in statement.query_map([project], |r| r.get::<_, String>(0))? {
            let id = row?;
            let hash = format!("{:x}", Sha256::digest(id.as_bytes()));
            if report_hashes.contains(&hash) {
                found.insert(hash, id);
            }
        }
        Ok(found)
    }

    /// True only for a run linked by guarded task publication with the same requirements.
    pub fn verification_task_linked(
        &self,
        project: &str,
        id: &str,
        task: i64,
        requirements: &str,
    ) -> Result<bool> {
        Ok(self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM verification_task_links l JOIN verification_runs r ON r.id=l.run_id WHERE r.project=?1 AND l.run_id=?2 AND l.task_id=?3 AND l.requirements_hash=?4)",
            params![project, id, task, requirements], |row| row.get(0),
        )?)
    }

    /// A stored report is not authority unless this run was transactionally linked to the task.
    pub fn verification_inspected_task_linked(
        &self,
        project: &str,
        id: &str,
        task: i64,
        requirements: &str,
        report_hash: &str,
    ) -> Result<bool> {
        // Schema 6 has no compact evidence metadata. Without its transaction
        // receipt the inspected publication is unknown, including for readers.
        if crate::storage::version(&self.connection)? < 7 {
            return Ok(false);
        }
        Ok(self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM verification_task_links l JOIN verification_runs r ON r.id=l.run_id WHERE r.project=?1 AND l.run_id=?2 AND l.task_id=?3 AND l.requirements_hash=?4 AND EXISTS(SELECT 1 FROM task_events e JOIN storage_evidence_metadata m ON m.event_seq=e.seq AND m.task_id=e.task_id WHERE e.project=?1 AND e.task_id=?3 AND e.kind='native-check' AND m.ordinal=(SELECT max(x.ordinal) FROM storage_evidence_metadata x WHERE x.event_seq=e.seq AND x.task_id=e.task_id) AND json_extract(m.data,'$.report_hash')=?5 AND json_extract(m.data,'$.verifier')='dctx0-inspected-verifier'))",
            params![project, id, task, requirements, report_hash], |row| row.get(0),
        )?)
    }

    pub fn verification_run_context(
        &self,
        project: &str,
        id: &str,
    ) -> Result<Option<VerificationRunContext>> {
        let summary = self.verification_run_summary(project, id)?;
        let (provenance, artifacts): (Option<String>, Option<String>) = self.connection.query_row(
            "SELECT json_extract(CASE WHEN data='' THEN '{}' ELSE data END,'$.provenance'),json_extract(CASE WHEN data='' THEN '{}' ELSE data END,'$.artifacts') FROM verification_runs WHERE project=?1 AND id=?2",
            params![project, id], |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let (Some(provenance), Some(artifacts)) = (provenance, artifacts) else {
            return if summary.details_available {
                Err(invalid("Stored verification context is missing"))
            } else {
                Ok(None)
            };
        };
        let context = VerificationRunContext {
            provenance: serde_json::from_str(&provenance)?,
            artifacts: serde_json::from_str(&artifacts)?,
        };
        if codec::json_hash(&context.provenance)? != summary.provenance_hash {
            return Err(invalid("Stored verification provenance digest mismatch"));
        }
        Ok(Some(context))
    }

    /// Latest check references at the exact event watermark. Legacy schemas
    /// have no compact index and return unknown rather than expanding reports.
    pub fn task_evidence_references(
        &self,
        project: &str,
        event_seq: i64,
    ) -> Result<Option<Vec<TaskEvidenceReference>>> {
        if crate::storage::version(&self.connection)? < 7 {
            return Ok(None);
        }
        let mut statement = self.connection.prepare(
            "WITH heads AS (SELECT task_id,max(event_seq) AS event_seq FROM storage_evidence_metadata WHERE event_seq<=?2 AND task_id IN (SELECT id FROM tasks WHERE project=?1) GROUP BY task_id) SELECT m.data FROM storage_evidence_metadata m JOIN heads h USING(task_id,event_seq) WHERE m.ordinal=(SELECT max(x.ordinal) FROM storage_evidence_metadata x WHERE x.task_id=m.task_id AND x.event_seq=m.event_seq) ORDER BY m.task_id",
        )?;
        Ok(Some(
            statement
                .query_map(params![project, event_seq], |r| r.get::<_, String>(0))?
                .map(|row| Ok(serde_json::from_str(&row?)?))
                .collect::<Result<Vec<_>>>()?,
        ))
    }
}
