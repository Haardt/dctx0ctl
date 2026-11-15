//! Metadata-only contradiction checks, repeated in the publication transaction.
use super::*;

pub(super) fn later_nonpass(
    c: &Connection,
    project: &str,
    parent: &str,
) -> Result<BTreeSet<String>> {
    // Compare the execution cohort and its exact external-input receipt, not
    // timestamps or IDs alone. Unclassified group failures contradict the whole
    // original cohort, including cases absent from an aborted later observation. Older schemas have no authoritative case index.
    if crate::storage::version(c)? < 6 {
        return Ok(BTreeSet::new());
    }
    let sql = r#"
SELECT DISTINCT original.case_id
FROM verification_runs p
JOIN verification_runs n ON n.project=p.project AND n.rowid>p.rowid
JOIN verification_case_index original ON original.run_id=p.id
LEFT JOIN verification_case_index i ON i.run_id=n.id AND i.case_id=original.case_id
WHERE p.project=?1 AND p.id=?2 AND n.origin=p.origin
AND json_valid(n.data) AND json_valid(p.data)
AND (
    i.outcome IN ('FAIL','UNKNOWN')
    OR EXISTS (
        SELECT 1 FROM json_each(n.data,'$.open_obligations') obligation
        WHERE json_extract(obligation.value,'$.code') NOT IN ('MANUAL_REQUIREMENT','SUITE_INSPECTION_REQUIRED')
    )
    OR EXISTS (
        SELECT 1 FROM json_each(n.data,'$.provenance.incomplete') gap
        WHERE json_extract(gap.value,'$.code')!='ADOPTION_HISTORICAL_SCOPE'
    )
    OR (
        json_extract(n.result,'$.status') IN ('FAIL','UNKNOWN')
        AND json_extract(n.result,'$.issue.code') NOT IN ('MANUAL_REQUIREMENT','SUITE_INSPECTION_REQUIRED','ADOPTION_HISTORICAL_SCOPE')
        AND NOT EXISTS (
            SELECT 1 FROM verification_case_index observed
            WHERE observed.run_id=n.id AND observed.outcome IN ('FAIL','UNKNOWN')
        )
    )
)
AND json_extract(n.data,'$.provenance.implementation')=json_extract(p.data,'$.provenance.implementation')
AND json_extract(n.data,'$.provenance.adapter_hash')=json_extract(p.data,'$.provenance.adapter_hash')
AND json_extract(n.data,'$.provenance.configuration_hash')=json_extract(p.data,'$.provenance.configuration_hash')
AND json_extract(n.data,'$.provenance.harness_hash')=json_extract(p.data,'$.provenance.harness_hash')
AND json_extract(n.data,'$.provenance.runtime_version')=json_extract(p.data,'$.provenance.runtime_version')
AND EXISTS (
    SELECT 1 FROM verification_run_artifacts old_input
    JOIN verification_artifacts a ON a.hash=old_input.artifact_hash
    JOIN verification_run_artifacts new_input ON new_input.artifact_hash=a.hash
    WHERE old_input.run_id=p.id AND new_input.run_id=n.id
    AND a.media_type='application/vnd.dctx0.incremental-inputs+json'
)"#;
    Ok(c.prepare(sql)?
        .query_map(params![project, parent], |row| row.get(0))?
        .collect::<std::result::Result<_, _>>()?)
}

impl Store {
    /// Ordering and same-project ancestry for diagnostic observation ledgers.
    /// This metadata is not a native reuse or task-publication authority.
    pub fn verification_observation_order(&self, project: &str, id: &str) -> Result<i64> {
        Ok(self.connection.query_row(
            "SELECT rowid FROM verification_runs WHERE project=?1 AND id=?2",
            params![project, id],
            |row| row.get(0),
        )?)
    }

    pub fn verification_direct_parent(
        &self,
        project: &str,
        child: &str,
        parent: &str,
    ) -> Result<bool> {
        if crate::storage::version(&self.connection)? < 7 {
            return Ok(false);
        }
        Ok(self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM verification_run_parents p JOIN verification_runs r ON r.id=p.child WHERE r.project=?1 AND p.child=?2 AND p.parent=?3)",
            params![project, child, parent], |row| row.get(0),
        )?)
    }

    /// A later failing observation, or unassigned group failure selecting this
    /// case, blocks an older green ledger entry even without an input receipt.
    pub fn verification_later_case_problem(
        &self,
        project: &str,
        source: &str,
        case: &str,
    ) -> Result<bool> {
        if crate::storage::version(&self.connection)? < 6 {
            return Ok(true);
        }
        Ok(self.connection.query_row(r#"
SELECT EXISTS(
 SELECT 1 FROM verification_runs p JOIN verification_runs n ON n.project=p.project AND n.rowid>p.rowid
 JOIN verification_requirements r ON r.run_id=n.id AND r.case_id=?3
 LEFT JOIN verification_case_index i ON i.run_id=n.id AND i.case_id=?3
 WHERE p.project=?1 AND p.id=?2 AND n.origin=p.origin
 AND json_extract(n.data,'$.provenance.implementation')=json_extract(p.data,'$.provenance.implementation')
 AND json_extract(n.data,'$.provenance.configuration_hash')=json_extract(p.data,'$.provenance.configuration_hash')
 AND (i.outcome IN ('FAIL','UNKNOWN') OR EXISTS (
   SELECT 1 FROM json_each(n.data,'$.provenance.incomplete') gap
   WHERE json_extract(gap.value,'$.code') NOT IN ('CONTINUATION_COMPOSITION_REQUIRED','HYBRID_COMPLETE_PROOF_REQUIRED','ADOPTION_HISTORICAL_SCOPE')
 ) OR (i.case_id IS NULL AND EXISTS (
   SELECT 1 FROM json_each(n.data,'$.open_obligations') gap
   WHERE json_extract(gap.value,'$.code') NOT IN ('MANUAL_REQUIREMENT','SUITE_INSPECTION_REQUIRED','CONTINUATION_COMPOSITION_REQUIRED')
 )))
)"#, params![project, source, case], |row| row.get(0))?)
    }

    pub fn verification_later_nonpass(
        &self,
        project: &str,
        parent: &str,
    ) -> Result<BTreeSet<String>> {
        later_nonpass(&self.connection, project, parent)
    }
}
