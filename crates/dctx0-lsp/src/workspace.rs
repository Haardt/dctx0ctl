use crate::{Analysis, TextIndex};
use dctx0_core::{Source, StatementKind};
use lsp_types::{Position, TextDocumentContentChangeEvent};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
};
use url::Url;

const MAX_FILE: usize = 2 * 1024 * 1024;
const MAX_TOTAL: usize = 16 * 1024 * 1024;
const MAX_FILES: usize = 2048;
#[derive(Clone)]
pub(crate) struct Document {
    pub text: String,
    pub version: i32,
}
#[derive(Clone)]
pub(crate) struct Workspace {
    pub roots: Vec<PathBuf>,
    pub documents: BTreeMap<String, Document>,
}
pub(crate) struct Snapshot {
    pub projects: Vec<Analysis>,
    pub versions: BTreeMap<String, i32>,
    pub warnings: Vec<String>,
}
impl Workspace {
    pub fn change(
        &mut self,
        uri: &str,
        version: i32,
        changes: Vec<TextDocumentContentChangeEvent>,
    ) -> Result<(), String> {
        let document = self.documents.get_mut(uri).ok_or("Document is not open")?;
        if version <= document.version {
            return Err("Ignoring stale document version".into());
        }
        let mut text = document.text.clone();
        for change in changes {
            if let Some(range) = change.range {
                let index = TextIndex::new(&text);
                let start = index
                    .offset(range.start)
                    .ok_or("Invalid UTF-16 edit start")?;
                let end = index.offset(range.end).ok_or("Invalid UTF-16 edit end")?;
                if start > end {
                    return Err("Reversed edit range".into());
                }
                text.replace_range(start..end, &change.text);
            } else {
                text = change.text;
            }
            if text.len() > MAX_FILE {
                return Err("Document exceeds 2 MiB editor limit".into());
            }
        }
        document.text = text;
        document.version = version;
        Ok(())
    }
    pub fn open(&mut self, uri: String, version: i32, text: String) -> Result<(), String> {
        let path = file_path(&uri).ok_or("Only local file URIs are supported")?;
        if path.extension().is_none_or(|e| e != "dctx0")
            || !self.roots.iter().any(|r| path.starts_with(r))
        {
            return Err("Document is outside the selected spec workspace".into());
        }
        if text.len() > MAX_FILE
            || self.documents.len() >= MAX_FILES
            || self.documents.values().map(|d| d.text.len()).sum::<usize>() + text.len() > MAX_TOTAL
        {
            return Err("Open documents exceed editor resource limits".into());
        }
        self.documents.insert(uri, Document { text, version });
        Ok(())
    }
    pub fn analyze(&self) -> Snapshot {
        let mut sources = BTreeMap::new();
        let mut warnings = Vec::new();
        let mut bytes = 0;
        let mut visited = 0;
        for root in &self.roots {
            if let Err(e) = discover(root, 0, &mut sources, &mut bytes, &mut visited) {
                warnings.push(e);
            }
        }
        for (uri, document) in &self.documents {
            // Keep the editor's URI, replacing the same disk file even when
            // macOS symlinks or Windows verbatim paths spell it differently.
            let path = file_path(uri);
            sources.retain(|disk_uri, _| file_path(disk_uri) != path);
            sources.insert(uri.clone(), document.text.clone());
        }
        // Each workspace declaration anchors a distinct project, including nested examples.
        let mut anchors = Vec::new();
        for (uri, text) in &sources {
            if dctx0_language::parse(uri, text)
                .ast
                .statements
                .iter()
                .any(|s| s.kind == StatementKind::Workspace)
            {
                if let Some(path) = file_path(uri).and_then(|p| p.parent().map(Path::to_owned)) {
                    anchors.push(path);
                }
            }
        }
        anchors.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
        let mut groups: BTreeMap<PathBuf, Vec<Source>> = BTreeMap::new();
        for (uri, text) in sources {
            let Some(path) = file_path(&uri) else {
                continue;
            };
            let root = anchors.iter().find(|p| path.starts_with(p)).or_else(|| {
                self.roots
                    .iter()
                    .filter(|p| path.starts_with(p))
                    .max_by_key(|p| p.components().count())
            });
            if let Some(root) = root {
                groups
                    .entry(root.clone())
                    .or_default()
                    .push(Source { file: uri, text });
            }
        }
        Snapshot {
            projects: groups.into_values().map(Analysis::new).collect(),
            versions: self
                .documents
                .iter()
                .map(|(uri, d)| (uri.clone(), d.version))
                .collect(),
            warnings,
        }
    }
}
impl Snapshot {
    pub fn project(&self, uri: &str) -> Option<&Analysis> {
        self.projects.iter().find(|project| {
            project.source(uri).is_some()
                || file_path(uri).is_some_and(|path| {
                    project
                        .files()
                        .any(|source| file_path(source).as_ref() == Some(&path))
                })
        })
    }
    pub fn request(
        &self,
        method: &str,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        use serde_json::{Value, json};
        if method == "workspace/symbol" {
            let query = params["query"].as_str().unwrap_or("");
            return Ok(json!(
                self.projects
                    .iter()
                    .flat_map(|p| p
                        .workspace_symbols(query)
                        .as_array()
                        .cloned()
                        .unwrap_or_default())
                    .collect::<Vec<_>>()
            ));
        }
        let uri = params["textDocument"]["uri"]
            .as_str()
            .ok_or("Missing textDocument URI")?;
        let Some(project) = self.project(uri) else {
            return Ok(match method {
                "textDocument/semanticTokens/full" => json!({"data":[]}),
                "textDocument/completion"
                | "textDocument/references"
                | "textDocument/documentSymbol" => json!([]),
                _ => Value::Null,
            });
        };
        // Compiler spans retain the open document's URI. Resolve a canonical alias
        // to that source before looking up symbols/tokens in the pinned analysis.
        let source_uri = if project.source(uri).is_some() {
            uri
        } else {
            let path = file_path(uri).ok_or("Invalid local file URI")?;
            project
                .files()
                .find(|source| file_path(source).as_ref() == Some(&path))
                .map(String::as_str)
                .ok_or("Document alias has no analyzed source")?
        };
        let position = || {
            serde_json::from_value::<Position>(params["position"].clone())
                .map_err(|e| e.to_string())
        };
        Ok(match method {
            "textDocument/documentSymbol" => project.document_symbols(source_uri),
            "textDocument/semanticTokens/full" => project.semantic_tokens(source_uri),
            "textDocument/completion" => project.completions(source_uri, position()?),
            "textDocument/hover" => project.hover(source_uri, position()?),
            "textDocument/definition" => project.definition(source_uri, position()?),
            "textDocument/references" => project.references(
                source_uri,
                position()?,
                params["context"]["includeDeclaration"]
                    .as_bool()
                    .unwrap_or(false),
            ),
            _ => return Err("Unsupported method".into()),
        })
    }
}
pub(crate) fn file_path(uri: &str) -> Option<PathBuf> {
    let path = Url::parse(uri).ok()?.to_file_path().ok()?;
    if path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return None;
    }
    // Open editor documents may not exist on disk yet.
    Some(fs::canonicalize(&path).unwrap_or(path))
}
pub(crate) fn file_uri(path: &Path) -> Result<String, String> {
    Url::from_file_path(path)
        .map(String::from)
        .map_err(|_| "Cannot encode file URI".into())
}
fn discover(
    path: &Path,
    depth: usize,
    sources: &mut BTreeMap<String, String>,
    bytes: &mut usize,
    visited: &mut usize,
) -> Result<(), String> {
    *visited += 1;
    if *visited > 20_000 {
        return Err("Source discovery exceeds 20,000 entries".into());
    }
    if depth > 32 {
        return Err("Source discovery exceeds depth 32".into());
    }
    let meta = fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if meta.file_type().is_symlink() {
        return Ok(());
    }
    if meta.is_dir() {
        let mut children = fs::read_dir(path)
            .map_err(|e| e.to_string())?
            .take(20_001)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        if children.len() > 20_000 {
            return Err("Source directory exceeds 20,000 entries".into());
        }
        children.sort_by_key(|e| e.file_name());
        for child in children {
            if [
                ".git",
                ".dctx0ctl",
                ".dctx0",
                ".agents",
                ".claude",
                ".runatron",
                "node_modules",
                "target",
                "dist",
                "vendor",
            ]
            .contains(&child.file_name().to_string_lossy().as_ref())
            {
                continue;
            }
            discover(&child.path(), depth + 1, sources, bytes, visited)?;
        }
    } else if meta.is_file() && path.extension().is_some_and(|e| e == "dctx0") {
        if meta.len() > MAX_FILE as u64
            || *bytes + meta.len() as usize > MAX_TOTAL
            || sources.len() >= MAX_FILES
        {
            return Err(
                "Source discovery exceeds editor limits (2 MiB/file, 16 MiB/project, 2048 files)"
                    .into(),
            );
        }
        let mut text = String::new();
        fs::File::open(path)
            .map_err(|e| e.to_string())?
            .take((MAX_FILE + 1) as u64)
            .read_to_string(&mut text)
            .map_err(|e| e.to_string())?;
        if text.len() > MAX_FILE || *bytes + text.len() > MAX_TOTAL {
            return Err("Source changed beyond editor size limits".into());
        }
        *bytes += text.len();
        sources.insert(file_uri(path)?, text);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn editor_alias_replaces_canonical_disk_source() {
        let directory =
            std::env::temp_dir().join(format!("dctx0-lsp-alias-{}", std::process::id()));
        let root = directory.join("spec");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("a.dctx0"),
            "workspace Platform { type Year = Int }",
        )
        .unwrap();
        let alias = directory.join("alias");
        std::os::unix::fs::symlink(&root, &alias).unwrap();
        let uri = file_uri(&alias.join("a.dctx0")).unwrap();
        let mut workspace = Workspace {
            roots: vec![fs::canonicalize(root).unwrap()],
            documents: BTreeMap::new(),
        };
        workspace
            .open(
                uri.clone(),
                1,
                "workspace Platform { type Year = Missing }".into(),
            )
            .unwrap();
        let snapshot = workspace.analyze();
        assert_eq!(snapshot.projects.len(), 1);
        assert_eq!(snapshot.versions.get(&uri), Some(&1));
        let project = snapshot.project(&uri).unwrap();
        assert_eq!(project.files().count(), 1);
        assert!(!project.diagnostics(&uri).as_array().unwrap().is_empty());
        let canonical_uri = file_uri(&workspace.roots[0].join("a.dctx0")).unwrap();
        assert_ne!(canonical_uri, uri);
        let aliased_tokens = snapshot
            .request(
                "textDocument/semanticTokens/full",
                &serde_json::json!({"textDocument":{"uri":canonical_uri}}),
            )
            .unwrap();
        assert_eq!(aliased_tokens, project.semantic_tokens(&uri));
        assert!(!aliased_tokens["data"].as_array().unwrap().is_empty());
        fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn edits_are_atomic_versioned_and_use_utf16() {
        let mut workspace = Workspace {
            roots: vec![PathBuf::from("/tmp")],
            documents: BTreeMap::new(),
        };
        let uri = "file:///tmp/edits.dctx0";
        workspace.open(uri.into(), 1, "😀 hello".into()).unwrap();
        let change = |start, end, text: &str| TextDocumentContentChangeEvent {
            range: Some(lsp_types::Range::new(
                Position::new(0, start),
                Position::new(0, end),
            )),
            range_length: None,
            text: text.into(),
        };
        assert!(workspace.change(uri, 2, vec![change(1, 2, "bad")]).is_err());
        assert_eq!(workspace.documents[uri].version, 1);
        workspace
            .change(uri, 2, vec![change(3, 8, "world")])
            .unwrap();
        assert_eq!(workspace.documents[uri].text, "😀 world");
        assert!(workspace.change(uri, 2, vec![]).is_err());
        assert!(
            workspace
                .open("file:///elsewhere/a.dctx0".into(), 1, "".into())
                .is_err()
        );
        assert!(file_path("https://example.com/x.dctx0").is_none());
    }
    #[test]
    fn nested_workspaces_are_isolated_and_closed_documents_return_to_disk() {
        let root = std::env::temp_dir().join(format!("dctx0-lsp-projects-{}", std::process::id()));
        for name in ["a", "b"] {
            fs::create_dir_all(root.join(name)).unwrap();
            fs::write(
                root.join(name).join("spec.dctx0"),
                format!("workspace {} {{ domain D {{}} }}", name.to_uppercase()),
            )
            .unwrap();
        }
        let mut workspace = Workspace {
            roots: vec![root.clone()],
            documents: BTreeMap::new(),
        };
        let uri = file_uri(&root.join("a/spec.dctx0")).unwrap();
        assert_eq!(workspace.analyze().projects.len(), 2);
        workspace
            .open(uri.clone(), 1, "workspace A { domain Edited {} }".into())
            .unwrap();
        assert!(
            workspace
                .analyze()
                .project(&uri)
                .unwrap()
                .source(&uri)
                .unwrap()
                .contains("Edited")
        );
        workspace.documents.clear();
        assert!(
            !workspace
                .analyze()
                .project(&uri)
                .unwrap()
                .source(&uri)
                .unwrap()
                .contains("Edited")
        );
        fs::remove_dir_all(root).unwrap();
    }
}
#[cfg(test)]
mod boundary_tests {
    use super::*;
    #[test]
    fn orphan_sources_and_io_errors_remain_observable_without_a_published_workspace() {
        let root =
            std::env::temp_dir().join(format!("dctx0-lsp-boundaries-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let uri = file_uri(&root.join("orphan.dctx0")).unwrap();
        fs::write(root.join("orphan.dctx0"), "domain D {}").unwrap();
        let mut workspace = Workspace {
            roots: vec![root.clone()],
            documents: BTreeMap::new(),
        };
        let snapshot = workspace.analyze();
        assert!(snapshot.project(&uri).is_some());
        assert!(
            !snapshot
                .project(&uri)
                .unwrap()
                .diagnostics(&uri)
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(
            snapshot
                .request(
                    "textDocument/hover",
                    &serde_json::json!({"textDocument":{"uri":uri}})
                )
                .is_err()
        );
        workspace
            .open(uri.clone(), 1, "domain D {}".into())
            .unwrap();
        workspace
            .open(
                file_uri(&root.join("second.dctx0")).unwrap(),
                1,
                "domain E {}".into(),
            )
            .unwrap();
        assert_eq!(workspace.documents.len(), 2);
        fs::write(root.join("invalid-utf8.dctx0"), [255]).unwrap();
        assert!(!workspace.analyze().warnings.is_empty());
        let missing = Workspace {
            roots: vec![root.join("absent")],
            documents: BTreeMap::new(),
        };
        assert!(!missing.analyze().warnings.is_empty());
        assert!(file_uri(Path::new("relative/file.dctx0")).is_err());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let private = root.join("private");
            fs::create_dir(&private).unwrap();
            fs::set_permissions(&private, fs::Permissions::from_mode(0o0)).unwrap();
            if fs::read_dir(&private).is_err() {
                assert!(discover(&private, 0, &mut BTreeMap::new(), &mut 0, &mut 0).is_err());
            }
            fs::set_permissions(&private, fs::Permissions::from_mode(0o700)).unwrap();
            let file = root.join("restricted.dctx0");
            fs::write(&file, "domain D {}").unwrap();
            fs::set_permissions(&file, fs::Permissions::from_mode(0o0)).unwrap();
            if fs::File::open(&file).is_err() {
                assert!(discover(&file, 0, &mut BTreeMap::new(), &mut 0, &mut 0).is_err());
            }
            fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
        }
        fs::remove_dir_all(root).unwrap();
    }
}
