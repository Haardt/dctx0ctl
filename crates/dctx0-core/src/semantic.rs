use crate::{Diagnostic, Span, StatementKind, Syntax, Token};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const IR_VERSION: u32 = 1;
pub const COMPILER_VERSION: &str = "dctx0-rust-1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub file: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Symbol {
    pub id: String,
    pub name: String,
    pub kind: StatementKind,
    pub parent: Option<String>,
    pub span: Span,
    pub name_span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeRef {
    pub name: String,
    pub symbol_id: Option<String>,
    pub arguments: Vec<TypeRef>,
}
impl TypeRef {
    pub fn builtin(name: &str) -> Self {
        Self {
            name: name.into(),
            symbol_id: None,
            arguments: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldIr {
    pub name: String,
    pub ty: TypeRef,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariantIr {
    pub name: String,
    pub payload: Option<TypeRef>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataIr {
    pub id: String,
    pub stable_id: Option<String>,
    pub alias: Option<TypeRef>,
    pub fields: Vec<FieldIr>,
    pub variants: Vec<VariantIr>,
    pub keys: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationIr {
    pub id: String,
    pub parameters: Vec<FieldIr>,
    pub result: TypeRef,
    pub failures: Vec<TypeRef>,
    pub pure: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReferenceKind {
    UsesType,
    Accepts,
    Returns,
    FailsWith,
    Calls,
    Reads,
    Writes,
    Emits,
    Observes,
    Owns,
    Requires,
    Specifies,
    Verifies,
    State,
    Fragment,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference {
    pub from: String,
    pub target: String,
    pub kind: ReferenceKind,
    pub spelling: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Expression {
    pub ty: TypeRef,
    pub span: Span,
    pub value: ExpressionKind,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ExpressionKind {
    Literal {
        text: String,
        literal_kind: String,
    },
    Variable {
        name: String,
    },
    Variant {
        name: String,
        declaration: String,
    },
    Field {
        target: Box<Expression>,
        name: String,
    },
    VariantRecord {
        declaration: String,
        variant: String,
        payload: Box<Expression>,
    },
    Trace {
        trace_kind: String,
        target: String,
        modeled_target: Option<String>,
        arguments: Vec<Expression>,
        count: Box<Expression>,
    },
    Record {
        declaration: String,
        fields: Vec<(String, Expression)>,
    },
    List {
        values: Vec<Expression>,
    },
    Unary {
        operator: String,
        operand: Box<Expression>,
    },
    Binary {
        operator: String,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Call {
        name: String,
        target: Option<String>,
        arguments: Vec<Expression>,
    },
    Collection {
        operator: String,
        binder: Option<String>,
        collection: Box<Expression>,
        predicate: Option<Box<Expression>>,
        projection: Option<Box<Expression>>,
    },
    State {
        phase: StatePhase,
        entity: String,
        keys: Vec<Expression>,
    },
}

/// State views have a closed set of phases; names retain their JSON spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StatePhase {
    Before,
    After,
}

/// Every clause is retained; typed expressions supplement its syntax. `parent`
/// is the actual syntax parent and `owner` the nearest named declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticNode {
    pub id: String,
    pub owner: String,
    pub parent: Option<String>,
    pub kind: StatementKind,
    pub name: Option<String>,
    pub terms: Vec<Token>,
    pub syntax: Option<Syntax>,
    pub span: Span,
    pub expressions: Vec<Expression>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceIr {
    pub schema_version: u32,
    pub compiler_version: String,
    pub workspace: String,
    pub symbols: BTreeMap<String, Symbol>,
    pub data: BTreeMap<String, DataIr>,
    pub operations: BTreeMap<String, OperationIr>,
    pub nodes: Vec<SemanticNode>,
    pub references: Vec<Reference>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Compilation {
    pub ir: Option<WorkspaceIr>,
    pub diagnostics: Vec<Diagnostic>,
}
