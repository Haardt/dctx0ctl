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
}

impl Store {
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
