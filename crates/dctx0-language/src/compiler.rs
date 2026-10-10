mod relations;
mod types;
use crate::composition::compose;
use dctx0_core::{Compilation, Source};
use relations::build_relations;
use types::build_types;

pub fn compile(sources: &[Source]) -> Compilation {
    let mut ir = match compose(sources) {
        Ok(ir) => ir,
        Err(diagnostics) => {
            return Compilation {
                ir: None,
                diagnostics,
            };
        }
    };
    let mut diagnostics = Vec::new();
    build_types(&mut ir, &mut diagnostics);
    if diagnostics.is_empty() {
        build_relations(&mut ir, &mut diagnostics);
    }
    if diagnostics.is_empty() {
        crate::expression::compile_clauses(&mut ir, &mut diagnostics);
    }
    ir.references.sort_by(|a, b| {
        (&a.span.file, a.span.start.offset, &a.target, a.kind).cmp(&(
            &b.span.file,
            b.span.start.offset,
            &b.target,
            b.kind,
        ))
    });
    ir.references.dedup();
    diagnostics.sort_by(|a, b| {
        (&a.span.file, a.span.start.offset, &a.code, &a.message).cmp(&(
            &b.span.file,
            b.span.start.offset,
            &b.code,
            &b.message,
        ))
    });
    Compilation {
        ir: if diagnostics.is_empty() {
            Some(ir)
        } else {
            None
        },
        diagnostics,
    }
}
