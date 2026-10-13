//! Testing-only calls into the actual read-only site API and export adapter.
use super::*;
use dctx0_engine::SourceReader;
use dctx0_store::SiteSelection;
impl Actual {
    pub(super) fn site(&mut self, subject: &str) -> Result<Value, String> {
        let project = self.project()?;
        if subject.ends_with(".emptyHistory") {
            return Ok(json!({"rejected":project.site_snapshot(&SiteSelection::default()).is_err(),"databaseCreated":project.database.exists()}));
        }
        let check = project.check().map_err(error)?;
        if !check.ok { return Err("Site fixture compilation failed".into()); }
        project.task_plan("latest").map_err(error)?;
        let snapshot = project.site_snapshot(&SiteSelection::default()).map_err(error)?;
        if subject.ends_with(".snapshot") {
            let wire = serde_json::to_value(&snapshot).map_err(error)?;
            return Ok(json!({"sourceFiles":snapshot.records.snapshot.sources.len(),"tasks":snapshot.tasks.len(),"integerPins":wire["records"]["check"]["run_id"].is_string()&&wire["records"]["tasks"]["event_seq"].is_string(),"sourcePinned":snapshot.records.snapshot.sources==project.sources().map_err(error)?,"unverified":snapshot.tasks.iter().all(|task|task.check==CheckStatus::Unverified)}));
        }
        if subject.ends_with(".exportSnapshot") {
            let before = fs::read(&project.database).map_err(error)?;
            let output = self.root.join("site-export");
            fs::create_dir_all(&output).map_err(error)?;
            fs::write(output.join("keep.txt"),"keep").map_err(error)?;
            project.site_export(&SiteSelection::default(),&output).map_err(error)?;
            let first = fs::read(output.join("index.html")).map_err(error)?;
            project.site_export(&SiteSelection::default(),&output).map_err(error)?;
            let html = String::from_utf8(first.clone()).map_err(error)?;
            return Ok(json!({"deterministic":first==fs::read(output.join("index.html")).map_err(error)?,"selfContained":html.contains("window.__SITE_LIVE__=false")&&!html.contains("<script src=")&&!html.contains("<link rel=\"stylesheet\""),"preservesUnmanaged":fs::read_to_string(output.join("keep.txt")).map_err(error)?=="keep","databaseUnchanged":before==fs::read(&project.database).map_err(error)?}));
        }
        Err("Unknown site operation".into())
    }
}
