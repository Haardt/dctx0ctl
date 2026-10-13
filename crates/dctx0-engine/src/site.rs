//! Versioned browser contract. Stored models stay typed; only this wire format
//! encodes all integer scalars as decimal strings (including IDs and spans).
use crate::*;
use dctx0_store::{SiteRecords, SiteSelection};
use serde::Serializer;
mod evidence;

#[derive(Debug)]
pub struct SiteSnapshot {
    pub schema_version: u32,
    pub project: String,
    pub notice: &'static str,
    pub records: SiteRecords,
    pub tasks: Vec<dctx0_tasks::Overview>,
    pub dependencies: Vec<dctx0_tasks::DependencyView>,
}
fn decimal_integers(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Number(n) if n.is_i64() || n.is_u64() => {
            *value = serde_json::Value::String(n.to_string());
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(decimal_integers),
        serde_json::Value::Object(items) => items.values_mut().for_each(decimal_integers),
        _ => {}
    }
}
impl Serialize for SiteSnapshot {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        self.browser_value()
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
}
impl SiteSnapshot {
    fn browser_value(&self) -> Result<serde_json::Value> {
        let mut records = evidence::records_value(&self.records)?;
        let mut tasks = serde_json::to_value(&self.tasks)?;
        let mut dependencies = serde_json::to_value(&self.dependencies)?;
        decimal_integers(&mut records);
        decimal_integers(&mut tasks);
        decimal_integers(&mut dependencies);
        Ok(
            serde_json::json!({"schema_version":self.schema_version,"project":self.project,"notice":self.notice,"records":records,"tasks":tasks,"dependencies":dependencies}),
        )
    }
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
    Ok(site_html_json(&serde_json::to_string(snapshot)?, live))
}
/// Build the same embedded page from a serialized snapshot, escaping raw-text delimiters.
pub fn site_html_json(json: &str, live: bool) -> String {
    let json = json
        .replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e");
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>dctx0 · Spec companion</title><style>{}</style></head><body><div id=\"root\"></div><script id=\"site-data\" type=\"application/json\">{json}</script><script>window.__SITE_LIVE__={live};</script><script>{}</script></body></html>",
        include_str!("../../../ui/site/dist/site.css"),
        include_str!("../../../ui/site/dist/site.js")
    )
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
        let snapshot = self.site_snapshot(selection)?;
        let json = serde_json::to_string(&snapshot)?;
        let html = site_html_json(&json, false);
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
