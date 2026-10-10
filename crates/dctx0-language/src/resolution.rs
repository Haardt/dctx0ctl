use crate::{composition::problem, kinds::is_data_declaration as data};
use dctx0_core::*;
use std::collections::BTreeMap;

/// Snapshot-local names; no persistent identity is inferred from this index.
pub(crate) struct SymbolResolver<'a> {
    symbols: &'a BTreeMap<String, Symbol>,
    by_suffix: BTreeMap<&'a str, Vec<&'a Symbol>>,
}
impl<'a> SymbolResolver<'a> {
    pub(crate) fn new(symbols: &'a BTreeMap<String, Symbol>) -> Self {
        let mut by_suffix = BTreeMap::<&str, Vec<&Symbol>>::new();
        for symbol in symbols.values() {
            // Match exactly the previous short-name/scoped-suffix lookup order.
            let mut suffixes = vec![symbol.name.as_str()];
            for (offset, _) in symbol.id.match_indices('.') {
                suffixes.push(&symbol.id[offset + 1..]);
            }
            suffixes.sort();
            suffixes.dedup();
            for suffix in suffixes {
                by_suffix.entry(suffix).or_default().push(symbol);
            }
        }
        Self { symbols, by_suffix }
    }
    pub(crate) fn resolve_symbol(
        &self,
        name: &str,
        owner: &str,
        filter: impl Fn(StatementKind) -> bool,
    ) -> Result<String, Vec<String>> {
        if let Some(symbol) = self.symbols.get(name).filter(|s| filter(s.kind)) {
            return Ok(symbol.id.clone());
        }
        let mut candidates = Vec::new();
        let mut best = 0;
        for symbol in self
            .by_suffix
            .get(name)
            .into_iter()
            .flatten()
            .filter(|s| filter(s.kind))
        {
            let score = owner
                .split('.')
                .zip(symbol.parent.as_deref().unwrap_or(&symbol.id).split('.'))
                .take_while(|(a, b)| a == b)
                .count();
            if score > best {
                best = score;
                candidates.clear();
            }
            if score == best {
                candidates.push(symbol.id.clone());
            }
        }
        if candidates.len() == 1 {
            Ok(candidates.remove(0))
        } else {
            Err(candidates)
        }
    }
}

pub(crate) fn builtin(name: &str) -> Option<usize> {
    match name {
        "String" | "Boolean" | "Int" | "Float" | "Decimal" | "UUID" | "Timestamp" | "Duration"
        | "Bytes" | "Unit" | "None" => Some(0),
        "Option" | "List" | "Some" | "Dctx0Option" => Some(1),
        "Dctx0Result" => Some(2),
        _ => None,
    }
}
pub(crate) fn reference(
    name: &str,
    span: &Span,
    owner: &str,
    kind: ReferenceKind,
    resolver: &SymbolResolver<'_>,
    filter: impl Fn(StatementKind) -> bool,
    output: (&mut Vec<Reference>, &mut Vec<Diagnostic>),
) -> Option<String> {
    let (refs, diagnostics) = output;
    match resolver.resolve_symbol(name, owner, filter) {
        Ok(target) => {
            refs.push(Reference {
                from: owner.into(),
                target: target.clone(),
                kind,
                spelling: name.into(),
                span: span.clone(),
            });
            Some(target)
        }
        Err(candidates) => {
            let code = if candidates.is_empty() {
                "DCTX0_UNKNOWN_REFERENCE"
            } else {
                "DCTX0_AMBIGUOUS_REFERENCE"
            };
            problem(
                diagnostics,
                code,
                format!(
                    "Cannot resolve {name} from {owner}{}",
                    if candidates.is_empty() {
                        String::new()
                    } else {
                        format!(": {}", candidates.join(", "))
                    }
                ),
                span,
            );
            diagnostics
                .last_mut()
                .expect("problem appends a diagnostic")
                .related = candidates
                .iter()
                .map(|id| RelatedDiagnostic {
                    message: id.clone(),
                    span: resolver.symbols[id].name_span.clone(),
                })
                .collect();
            None
        }
    }
}
pub(crate) fn type_ref(
    ty: &TypeSyntax,
    owner: &str,
    kind: ReferenceKind,
    resolver: &SymbolResolver<'_>,
    refs: &mut Vec<Reference>,
    diagnostics: &mut Vec<Diagnostic>,
) -> TypeRef {
    let arguments = ty
        .arguments
        .iter()
        .map(|a| type_ref(a, owner, kind, resolver, refs, diagnostics))
        .collect::<Vec<_>>();
    let symbol_id = if let Some(arity) = builtin(&ty.name) {
        if arity != arguments.len() {
            problem(
                diagnostics,
                "DCTX0_TYPE_ARITY",
                format!("{} expects {arity} type arguments", ty.name),
                &ty.span,
            );
        }
        None
    } else {
        if !arguments.is_empty() {
            problem(
                diagnostics,
                "DCTX0_TYPE_ARITY",
                "User declarations do not accept type arguments",
                &ty.span,
            );
        }
        let mut span = ty.span.clone();
        span.end.offset = span.start.offset + ty.name.len();
        span.end.line = span.start.line;
        span.end.column = span.start.column + ty.name.chars().count();
        reference(
            &ty.name,
            &span,
            owner,
            kind,
            resolver,
            data,
            (refs, diagnostics),
        )
    };
    TypeRef {
        name: ty.name.clone(),
        symbol_id,
        arguments,
    }
}
