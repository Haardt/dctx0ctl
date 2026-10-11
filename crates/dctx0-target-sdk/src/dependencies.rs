//! Structural dependency references; resolution and capture belong to Engine.
use crate::{Error, InspectionSource};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DependencyKind {
    Import,
    Reexport,
    DynamicImport,
    Require,
    DynamicCode,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyReference {
    pub source: String,
    pub kind: DependencyKind,
    /// None is an unresolved expression, never an empty dependency closure.
    pub specifier: Option<String>,
    pub start_byte: usize,
    pub end_byte: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyInspection {
    pub schema_version: u32,
    pub analyzer_version: String,
    pub sources: Vec<String>,
    pub references: Vec<DependencyReference>,
    pub syntax_errors: Vec<String>,
}
pub trait DependencyAnalyzer {
    fn dependencies(&self, sources: &[InspectionSource]) -> Result<DependencyInspection, Error>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DependencyResolution {
    Import,
    Require,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleObserverSite {
    pub start_byte: usize,
    pub end_byte: usize,
    pub argument_start: usize,
    pub argument_end: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleObserverSource {
    pub source: String,
    pub sha256: String,
    pub sites: Vec<ModuleObserverSite>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleObserverManifest {
    pub schema_version: u32,
    pub observer_version: String,
    pub project_root_relative: String,
    pub sources: Vec<ModuleObserverSource>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleLoadAttempt {
    pub source: String,
    pub start_byte: usize,
    pub specifier: String,
    /// Candidate resolved by Bun's native import resolver. This is an attempt,
    /// not a claim that application evaluation completed successfully.
    pub resolved: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleObserverEvidence {
    pub schema_version: u32,
    pub observer_version: String,
    pub loaded_sources: Vec<String>,
    pub attempts: Vec<ModuleLoadAttempt>,
    pub complete: bool,
}
