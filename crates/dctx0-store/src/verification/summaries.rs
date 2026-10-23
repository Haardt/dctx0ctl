use super::*;
use dctx0_core::VerificationRunOrigin;

/// Historical outcome, never a replacement for full execution evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationRunSummary {
    #[serde(default = "summary_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub open_obligation_count: usize,
    #[serde(default)]
    pub open_obligation_count_available: bool,
    #[serde(default)]
    pub missing_case_count: usize,
    pub id: String,
    pub created_at: String,
    pub started_at_unix_ns: String,
    pub origin: VerificationRunOrigin,
    pub target: String,
    pub snapshot: String,
    pub check_occurrence: i64,
    pub selected_subjects: BTreeSet<String>,
    pub required_subject_count: usize,
    pub required_case_count: usize,
    pub case_count: usize,
    pub case_inventory_complete: bool,
    pub case_subjects: BTreeSet<String>,
    /// PASS, FAIL, UNKNOWN, in that order.
    pub outcomes: [u64; 3],
    pub result: VerificationResult,
    pub provenance_hash: String,
    pub details_available: bool,
    pub pruned_at: Option<String>,
}
impl VerificationRunSummary {
    pub fn new(id: &str, run: &VerificationRun) -> Result<Self> {
        let mut outcomes = [0; 3];
        for case in &run.cases {
            outcomes[match case.result {
                VerificationResult::Pass => 0,
                VerificationResult::Fail { .. } => 1,
                VerificationResult::Unknown { .. } => 2,
            }] += 1;
        }
        Ok(Self {
            schema_version: 1,
            open_obligation_count: run.open_obligations.len(),
            open_obligation_count_available: true,
            missing_case_count: run.required_cases.len().saturating_sub(run.cases.len()),
            id: id.into(),
            created_at: String::new(),
            started_at_unix_ns: run.started_at_unix_ns.clone(),
            origin: run.origin.clone(),
            target: run.provenance.target.clone(),
            snapshot: run.provenance.snapshot.clone(),
            check_occurrence: run.provenance.check_occurrence,
            selected_subjects: run.selected_subjects.clone(),
            required_subject_count: run.required_subjects.len(),
            required_case_count: run.required_cases.len(),
            case_count: run.cases.len(),
            case_inventory_complete: run
                .cases
                .iter()
                .map(|c| c.id.clone())
                .collect::<BTreeSet<_>>()
                == run.required_cases,
            case_subjects: run.cases.iter().map(|c| c.subject.clone()).collect(),
            outcomes,
            result: run.result.clone(),
            provenance_hash: codec::json_hash(&run.provenance)?,
            details_available: true,
            pruned_at: None,
        })
    }
}
impl Store {
    pub fn verification_run_summary(
        &self,
        project: &str,
        id: &str,
    ) -> Result<VerificationRunSummary> {
        if !self.has_verification_history()? {
            return Err(Error::Missing(format!("Verification run {id}")));
        }
        summary(&self.connection, project, id)
    }
    /// Limit is bounded, and summaries never decode schema-5 case corpora.
    pub fn verification_run_summaries(
        &self,
        project: &str,
        offset: u64,
        limit: u32,
    ) -> Result<Vec<VerificationRunSummary>> {
        if !self.has_verification_history()? {
            return Ok(vec![]);
        }
        if !(1..=200).contains(&limit) || offset > i64::MAX as u64 {
            return Err(invalid(
                "History page must have limit 1..200 and a valid offset",
            ));
        }
        let mut statement = self.connection.prepare("SELECT id FROM verification_runs WHERE project=?1 ORDER BY rowid DESC LIMIT ?2 OFFSET ?3")?;
        statement
            .query_map(params![project, limit, offset as i64], |r| {
                r.get::<_, String>(0)
            })?
            .map(|id| summary(&self.connection, project, &id?))
            .collect()
    }
}
pub(super) fn summary(
    connection: &Connection,
    project: &str,
    id: &str,
) -> Result<VerificationRunSummary> {
    let version: i64 = connection.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let sql = if version >= 5 {
        "SELECT created_at,summary,pruned_at FROM verification_runs WHERE project=?1 AND id=?2"
    } else {
        "SELECT created_at,NULL,NULL FROM verification_runs WHERE project=?1 AND id=?2"
    };
    let (created, data, pruned): (String, Option<String>, Option<String>) = connection
        .query_row(sql, params![project, id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .optional()?
        .ok_or_else(|| Error::Missing(format!("Verification run {id}")))?;
    let mut summary = match data {
        Some(data) => decode(&data)?,
        None => {
            // Ignore observation/state payloads while deriving a legacy summary.
            // This reads one JSON string, never constructs its full case values.
            let data: String = connection.query_row(
                "SELECT data FROM verification_runs WHERE project=?1 AND id=?2",
                params![project, id],
                |r| r.get(0),
            )?;
            if format!("{:x}", Sha256::digest(data.as_bytes())) != id {
                return Err(invalid("Stored legacy run digest is corrupt"));
            }
            let legacy: LegacySummary = decode(&data)?;
            legacy.finish(id)?
        }
    };
    summary.missing_case_count = summary
        .required_case_count
        .saturating_sub(summary.case_count);
    if !summary.open_obligation_count_available && pruned.is_none() {
        #[derive(Deserialize)]
        struct Obligations {
            open_obligations: Vec<dctx0_core::VerificationIssue>,
        }
        let header: String = connection.query_row(
            "SELECT data FROM verification_runs WHERE project=?1 AND id=?2",
            params![project, id],
            |r| r.get(0),
        )?;
        let header: Obligations = decode(&header)?;
        summary.open_obligation_count = header.open_obligations.len();
        summary.open_obligation_count_available = true;
    }
    summary.created_at = created;
    summary.details_available = pruned.is_none();
    summary.pruned_at = pruned;
    Ok(summary)
}

#[derive(Deserialize)]
struct LegacyCase {
    id: String,
    subject: String,
    result: VerificationResult,
}
#[derive(Deserialize)]
struct LegacySummary {
    started_at_unix_ns: String,
    origin: VerificationRunOrigin,
    provenance: dctx0_core::VerificationProvenance,
    selected_subjects: BTreeSet<String>,
    required_subjects: BTreeSet<String>,
    required_cases: BTreeSet<String>,
    cases: Vec<LegacyCase>,
    result: VerificationResult,
    #[serde(default)]
    open_obligations: Vec<dctx0_core::VerificationIssue>,
}
impl LegacySummary {
    fn finish(self, id: &str) -> Result<VerificationRunSummary> {
        let mut outcomes = [0; 3];
        for case in &self.cases {
            outcomes[match case.result {
                VerificationResult::Pass => 0,
                VerificationResult::Fail { .. } => 1,
                VerificationResult::Unknown { .. } => 2,
            }] += 1;
        }
        Ok(VerificationRunSummary {
            schema_version: 1,
            open_obligation_count: self.open_obligations.len(),
            open_obligation_count_available: true,
            missing_case_count: self.required_cases.len().saturating_sub(self.cases.len()),
            id: id.into(),
            created_at: String::new(),
            started_at_unix_ns: self.started_at_unix_ns,
            origin: self.origin,
            target: self.provenance.target.clone(),
            snapshot: self.provenance.snapshot.clone(),
            check_occurrence: self.provenance.check_occurrence,
            selected_subjects: self.selected_subjects,
            required_subject_count: self.required_subjects.len(),
            required_case_count: self.required_cases.len(),
            case_count: self.cases.len(),
            case_inventory_complete: self
                .cases
                .iter()
                .map(|c| c.id.clone())
                .collect::<BTreeSet<_>>()
                == self.required_cases,
            case_subjects: self.cases.iter().map(|c| c.subject.clone()).collect(),
            outcomes,
            result: self.result,
            provenance_hash: codec::json_hash(&self.provenance)?,
            details_available: true,
            pruned_at: None,
        })
    }
}

fn summary_version() -> u32 {
    1
}
