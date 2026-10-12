//! Structural inspection receives source text; targets perform no filesystem I/O.
use crate::Error;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InspectionLanguage {
    TypeScript,
    Tsx,
    Go,
    Rust,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InspectionSource {
    pub path: String,
    pub language: InspectionLanguage,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StructuralDeclaration {
    pub name: String,
    pub kind: String,
    pub exported: bool,
    pub start_byte: usize,
    pub end_byte: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StructuralStatement {
    pub text: String,
    pub start_byte: usize,
    pub end_byte: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceInspection {
    pub path: String,
    pub language: InspectionLanguage,
    pub syntax_errors: bool,
    pub declarations: Vec<StructuralDeclaration>,
    pub imports: Vec<StructuralStatement>,
    pub exports: Vec<StructuralStatement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InspectionReport {
    pub schema_version: u32,
    pub target: String,
    pub inspector_version: String,
    pub sources: Vec<SourceInspection>,
}

pub trait ImplementationInspector {
    fn inspect(&self, sources: &[InspectionSource]) -> Result<InspectionReport, Error>;
}
