//! Versioned browser contract. Stored models stay typed; only this wire format
//! encodes all integer scalars as decimal strings (including IDs and spans).
use crate::*;
use dctx0_store::{SiteRecords, SiteSelection};
use serde::Serializer;
mod evidence;
mod json;
mod progress;
pub use progress::SiteProgress;

#[derive(Debug)]
pub struct SiteSnapshot {
    pub schema_version: u32,
    pub project: String,
    pub notice: &'static str,
    pub records: SiteRecords,
    pub tasks: Vec<dctx0_tasks::Overview>,
    pub dependencies: Vec<dctx0_tasks::DependencyView>,
}
impl Serialize for SiteSnapshot {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let json = site_json(self).map_err(serde::ser::Error::custom)?;
        serde_json::value::RawValue::from_string(json)
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
}
#[derive(Serialize)]
struct BrowserSnapshot<'a> {
    schema_version: &'a serde_json::value::RawValue,
    project: &'a str,
    notice: &'a str,
    records: evidence::RecordsView<'a>,
    tasks: &'a [dctx0_tasks::Overview],
    dependencies: &'a [dctx0_tasks::DependencyView],
}
/// Stream typed records directly; only the top-level schema version stays numeric.
pub fn site_json(snapshot: &SiteSnapshot) -> Result<String> {
    site_json_with_progress(snapshot, &mut |_| {})
}
pub fn site_json_with_progress(
    snapshot: &SiteSnapshot,
    progress: &mut dyn FnMut(SiteProgress),
) -> Result<String> {
    let version = serde_json::value::RawValue::from_string(snapshot.schema_version.to_string())?;
    let view = BrowserSnapshot {
        schema_version: &version,
        project: &snapshot.project,
        notice: snapshot.notice,
        records: evidence::records_view(&snapshot.records, progress)?,
        tasks: &snapshot.tasks,
        dependencies: &snapshot.dependencies,
    };
    let mut bytes = Vec::new();
    progress(SiteProgress::EncodingJson);
    json::write(&mut bytes, &view)?;
    String::from_utf8(bytes).map_err(|error| Error::Invalid(error.to_string()))
}
impl Project {
    fn site_store(&self) -> Result<Store> {
        if !self.database.is_file() {
            return Err(Error::Invalid(format!(
                "No published site history at {}; run dctx0 check with the same database selection first",
                self.database.display()
            )));
        }
        Ok(Store::open(&self.database, AccessMode::ReadOnly)?)
    }
    pub fn site_revision(&self) -> Result<dctx0_store::SiteRevision> {
        Ok(self.site_store()?.site_revision(&self.key())?)
    }
    pub fn site_snapshot(&self, selection: &SiteSelection) -> Result<SiteSnapshot> {
        let store = self.site_store()?;
        let records = store.site_records(&self.key(), selection)?;
        let tasks = records
            .tasks
            .as_ref()
            .map_or_else(Vec::new, |s| s.overviews());
        let dependencies = records
            .tasks
            .as_ref()
            .map_or_else(Vec::new, |s| s.dependencies());
        Ok(SiteSnapshot {
            schema_version: 1,
            project: self.key(),
            notice: "Stored evidence describes its captured implementation; working-tree freshness is not assessed.",
            records,
            tasks,
            dependencies,
        })
    }
}
/// Safe even when a source contains a closing script tag or HTML characters.
pub fn site_html(snapshot: &SiteSnapshot, live: bool) -> Result<String> {
    Ok(site_html_json(&site_json(snapshot)?, live))
}
/// Build the same embedded page from a serialized snapshot, escaping raw-text delimiters.
pub fn site_html_json(json: &str, live: bool) -> String {
    let mut html = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>dctx0 · Spec companion</title><style>{}</style></head><body><div id=\"root\"></div><script id=\"site-data\" type=\"application/json\">",
        include_str!("../../../ui/site/dist/site.css")
    );
    let escapes = json
        .bytes()
        .filter(|byte| matches!(*byte, b'&' | b'<' | b'>'))
        .count();
    html.reserve(
        json.len() + escapes * 5 + include_str!("../../../ui/site/dist/site.js").len() + 256,
    );
    let mut start = 0;
    for (offset, token) in json.match_indices(['&', '<', '>']) {
        html.push_str(&json[start..offset]);
        html.push_str(match token {
            "&" => "\\u0026",
            "<" => "\\u003c",
            _ => "\\u003e",
        });
        start = offset + token.len();
    }
    html.push_str(&json[start..]);
    html.push_str(&format!(
        "</script><script>window.__SITE_LIVE__={live};</script><script>{}</script></body></html>",
        include_str!("../../../ui/site/dist/site.js")
    ));
    html
}
#[derive(Serialize, Deserialize)]
struct ExportManifest {
    schema_version: u32,
    files: std::collections::BTreeMap<String, String>,
}
fn content_hash(bytes: &[u8]) -> String {
    use sha2::Digest;
    format!("{:x}", sha2::Sha256::digest(bytes))
}
impl Project {
    pub fn site_export(&self, selection: &SiteSelection, output: &Path) -> Result<()> {
        self.site_export_with_progress(selection, output, &mut |_| {})
    }
    pub fn site_export_with_progress(
        &self,
        selection: &SiteSelection,
        output: &Path,
        progress: &mut dyn FnMut(SiteProgress),
    ) -> Result<()> {
        progress(SiteProgress::LoadingRecords);
        let snapshot = self.site_snapshot(selection)?;
        let json = site_json_with_progress(&snapshot, progress)?;
        drop(snapshot);
        progress(SiteProgress::RenderingHtml);
        let html = site_html_json(&json, false);
        progress(SiteProgress::WritingExport);
        fs::create_dir_all(output)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(output.join(".dctx0-site.lock"))?;
        fs2::FileExt::lock_exclusive(&lock)?;
        let manifest_path = output.join(".dctx0-site.json");
        let previous: Option<ExportManifest> = if manifest_path.exists() {
            Some(serde_json::from_slice(&fs::read(&manifest_path)?)?)
        } else {
            None
        };
        if previous.as_ref().is_some_and(|m| m.schema_version != 1) {
            return Err(Error::Invalid("Unsupported site export manifest".into()));
        }
        let files = [
            ("snapshot.json", json.as_bytes()),
            (
                "LICENSES.txt",
                include_bytes!("../../../ui/site/dist/LICENSES.txt").as_slice(),
            ),
            (
                "provenance.json",
                include_bytes!("../../../ui/site/dist/provenance.json").as_slice(),
            ),
            ("index.html", html.as_bytes()),
        ];
        // Unmanaged collisions and edits to generated files are never overwritten.
        for (name, _) in &files {
            let path = output.join(name);
            if path.exists() {
                let expected = previous.as_ref().and_then(|m| m.files.get(*name));
                if expected != Some(&content_hash(&fs::read(&path)?)) {
                    return Err(Error::Invalid(format!(
                        "Preserving unmanaged or edited export file: {}",
                        path.display()
                    )));
                }
            }
        }
        let manifest = ExportManifest {
            schema_version: 1,
            files: files
                .iter()
                .map(|(name, bytes)| ((*name).into(), content_hash(bytes)))
                .collect(),
        };
        // The self-contained index is its coherent commit point, published last.
        for (name, bytes) in files {
            atomic_write(&output.join(name), bytes)?;
        }
        atomic_write(&manifest_path, &serde_json::to_vec_pretty(&manifest)?)?;
        Ok(())
    }
}
