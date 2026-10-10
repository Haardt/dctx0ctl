//! Pure dctx0 parsing, workspace composition, resolution, and typed IR compilation.
mod lexer;
mod parser;
mod syntax;
pub use lexer::{LexResult, lex};
pub use parser::{ParseResult, parse};
mod compiler;
mod composition;
mod expression;
pub use compiler::compile;

mod kinds;
mod resolution;
