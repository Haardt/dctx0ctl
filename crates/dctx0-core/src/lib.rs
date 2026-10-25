//! Language-neutral syntax and diagnostics. Offsets are UTF-8 bytes; columns
//! count Unicode scalar values, starting at one. Spans are half-open.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub offset: usize,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub file: String,
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelatedDiagnostic {
    pub message: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
    pub span: Span,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related: Vec<RelatedDiagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TokenKind {
    Identifier,
    String,
    TripleString,
    Number,
    Symbol,
    Newline,
    Eof,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
    pub span: Span,
}

macro_rules! statement_kinds {
    ($($variant:ident => $keyword:literal),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub enum StatementKind { $(#[serde(rename = $keyword)] $variant),+ }
        impl StatementKind {
            pub fn from_keyword(keyword: &str) -> Option<Self> {
                match keyword { $($keyword => Some(Self::$variant),)+ _ => None }
            }
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $keyword),+ }
            }
        }
    };
}
statement_kinds! {
    Workspace => "workspace", Domain => "domain", Scalar => "scalar",
    Value => "value", Type => "type", Failure => "failure", Entity => "entity",
    Message => "message", Event => "event", Module => "module", Surface => "surface",
    Port => "port", External => "external", Owns => "owns", Requires => "requires",
    Operation => "operation", Effects => "effects", Read => "read", Write => "write",
    Call => "call", Emit => "emit", Observe => "observe", Intent => "intent",
    Decision => "decision", Choose => "choose", Because => "because", Avoid => "avoid",
    Constraint => "constraint", Example => "example", Given => "given", When => "when",
    Expect => "expect", Fails => "fails", Emits => "emits", Invariant => "invariant",
    Forall => "forall", Proof => "proof", Test => "test", Review => "review",
    Inspect => "inspect", Criteria => "criteria", Question => "question", Policy => "policy",
    Require => "require", Range => "range", Length => "length", Format => "format",
    Where => "where", Key => "key", Ensures => "ensures", State => "state",
    Lifecycle => "lifecycle", Transition => "transition", Assert => "assert",
    Fragment => "fragment", StableId => "stableId", Pure => "pure",
    Field => "field", Own => "own", Dependency => "dependency", Artifact => "artifact",
    Criterion => "criterion", Narrative => "narrative", Assertion => "assertion",
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceAst {
    pub file: String,
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Statement {
    pub kind: StatementKind,
    pub name: Option<String>,
    /// Original token kinds and decoded text, including source spans.
    pub terms: Vec<Token>,
    pub children: Vec<Statement>,
    pub span: Span,
    pub syntax: Option<Syntax>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeSyntax {
    pub name: String,
    pub arguments: Vec<TypeSyntax>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub ty: TypeSyntax,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Variant {
    pub name: String,
    pub payload: Option<TypeSyntax>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EffectPayload {
    Scalar { expression: Vec<Token> },
    Record { fields: Vec<(String, Vec<Token>)> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tag", rename_all = "kebab-case")]
pub enum Syntax {
    Operation {
        parameters: Vec<Parameter>,
        result: Option<TypeSyntax>,
        failures: Vec<TypeSyntax>,
    },
    Data {
        alias: Option<TypeSyntax>,
        fields: Vec<Parameter>,
        variants: Vec<Variant>,
    },
    Narrative {
        text: String,
        narrative: bool,
    },
    Fragment {
        target: String,
        target_span: Span,
    },
    StableId {
        value: String,
        value_span: Span,
        quoted: bool,
    },
    Effect {
        target: String,
        result_alias: Option<String>,
        payload: Option<EffectPayload>,
    },
}

mod semantic;
pub use semantic::*;

mod profiles;
pub use profiles::*;

mod verification;
pub use verification::*;

mod codecs;
pub use codecs::*;

mod bindings;
pub use bindings::*;
