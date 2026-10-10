//! Pure rename planning: resolution, edits, validation, and proposed snapshots.
use super::*;
// Fragment paths may contain spaces or comments around dots. Use lexer byte
// positions rather than assuming the normalized name occupies contiguous bytes.
pub(super) fn reference_component(raw: &str, index: usize) -> Option<usize> {
    let tokens = dctx0_language::lex("", raw).tokens;
    let mut components = Vec::new();
    for token in tokens
        .iter()
        .filter(|t| t.kind == dctx0_core::TokenKind::Identifier)
    {
        let mut offset = token.span.start.offset;
        for part in token.text.split('.') {
            components.push(offset);
            offset += part.len() + 1;
        }
    }
    components.get(index).copied()
}
pub(super) fn validate_reference(source: &str, start: usize, end: usize, name: &str) -> Result<()> {
    if source.get(start..end) != Some(name) {
        return Err(RenameError::InvalidReference.into());
    }
    Ok(())
}
pub(super) fn validate_edits(edits: &[Edit]) -> Result<()> {
    for pair in edits.windows(2) {
        if pair[0].file == pair[1].file && pair[0].end > pair[1].start {
            return Err(RenameError::OverlappingEdits.into());
        }
    }
    Ok(())
}
impl RenamePlan {
    pub fn report(&self, dry_run: bool) -> RenameReport {
        RenameReport {
            dry_run,
            symbol: self.symbol.clone(),
            new_symbol: self.new_symbol.clone(),
            before: self.before.id.clone(),
            after: self.after.id.clone(),
            edits: self.edits.clone(),
            mappings: self.mappings.clone(),
            changes: diff(Some(&self.before), &self.after, &self.mappings),
        }
    }
}
pub fn plan_rename(
    sources: &[dctx0_core::Source],
    symbol: &str,
    new_name: &str,
) -> Result<RenamePlan> {
    if !valid_name(new_name) {
        return Err(RenameError::InvalidName.into());
    }
    let compilation = dctx0_language::compile(sources);
    let ir = compilation.ir.ok_or(RenameError::InvalidWorkspace {
        diagnostics: compilation.diagnostics,
    })?;
    let target = ir
        .symbols
        .get(symbol)
        .ok_or_else(|| RenameError::MissingSymbol {
            symbol: symbol.into(),
        })?;
    if target.name == new_name {
        return Err(RenameError::UnchangedName.into());
    }
    let new_symbol = target
        .parent
        .as_ref()
        .map_or_else(|| new_name.to_owned(), |p| format!("{p}.{new_name}"));
    if ir.symbols.contains_key(&new_symbol) {
        return Err(RenameError::NameConflict {
            symbol: new_symbol.clone(),
        }
        .into());
    }
    let mappings = ir
        .nodes
        .iter()
        .filter(|n| {
            n.id == symbol
                || n.id.starts_with(&format!("{symbol}."))
                || n.id.starts_with(&format!("{symbol}#"))
        })
        .map(|n| RenameMapping {
            old: n.id.clone(),
            new: format!("{new_symbol}{}", &n.id[symbol.len()..]),
        })
        .collect::<Vec<_>>();
    let rename_map = mappings
        .iter()
        .map(|m| (m.old.as_str(), m.new.as_str()))
        .collect::<BTreeMap<_, _>>();
    let source_map = sources
        .iter()
        .map(|s| (s.file.as_str(), s.text.as_str()))
        .collect::<BTreeMap<_, _>>();
    let raw = &source_map[target.name_span.file.as_str()]
        [target.name_span.start.offset..target.name_span.end.offset];
    let replacement = if raw.starts_with('"') {
        serde_json::to_string(new_name)?
    } else {
        new_name.into()
    };
    let mut edits = vec![Edit {
        file: target.name_span.file.clone(),
        start: target.name_span.start.offset,
        end: target.name_span.end.offset,
        old: raw.into(),
        new: replacement,
    }];
    for reference in &ir.references {
        if !rename_map.contains_key(reference.target.as_str()) {
            continue;
        }
        // Only rename lexical references containing the declaration name.
        // A bare enum variant (Active) points to its enum but is not its name.
        let parts = reference.spelling.split('.').collect::<Vec<_>>();
        let target_parts = reference.target.split('.').collect::<Vec<_>>();
        let symbol_parts = symbol.split('.').collect::<Vec<_>>();
        let base = target_parts.len().saturating_sub(parts.len());
        if target_parts[base..] != parts[..] {
            continue;
        }
        let component = symbol_parts.len() - 1;
        if component < base || component >= target_parts.len() {
            continue;
        }
        let local = component - base;
        let source = source_map[reference.span.file.as_str()];
        let raw = source
            .get(reference.span.start.offset..reference.span.end.offset)
            .expect("compiler reference spans index their source");
        let offset = reference_component(raw, local)
            .expect("resolved path contains its normalized components");
        let start = reference.span.start.offset + offset;
        let end = start + parts[local].len();
        validate_reference(source, start, end, &target.name)?;
        edits.push(Edit {
            file: reference.span.file.clone(),
            start,
            end,
            old: target.name.clone(),
            new: new_name.into(),
        });
    }
    edits.sort_by(|a, b| (&a.file, a.start, a.end).cmp(&(&b.file, b.start, b.end)));
    edits.dedup_by(|a, b| a.file == b.file && a.start == b.start && a.end == b.end);
    validate_edits(&edits)?;
    let mut edited = sources.to_vec();
    for source in &mut edited {
        for edit in edits.iter().rev().filter(|e| e.file == source.file) {
            source.text.replace_range(edit.start..edit.end, &edit.new);
        }
    }
    let result = dctx0_language::compile(&edited);
    let after_ir = result.ir.ok_or(RenameError::InvalidResult {
        diagnostics: result.diagnostics,
    })?;
    for old in ir.symbols.keys() {
        let expected = rename_map.get(old.as_str()).copied().unwrap_or(old);
        assert!(
            after_ir.symbols.contains_key(expected),
            "validated rename preserves every declaration"
        );
    }
    let before_graph = dctx0_graph::build(&ir);
    let after_graph = dctx0_graph::build(&after_ir);
    Ok(RenamePlan {
        before: snapshot(sources, ir, before_graph),
        after: snapshot(&edited, after_ir, after_graph),
        edits,
        mappings,
        symbol: symbol.into(),
        new_symbol,
    })
}
fn valid_name(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
        && StatementKind::from_keyword(name).is_none()
        && !matches!(
            name,
            "String"
                | "Boolean"
                | "Int"
                | "Float"
                | "Decimal"
                | "UUID"
                | "Timestamp"
                | "Duration"
                | "Bytes"
                | "Unit"
                | "Option"
                | "List"
                | "Some"
                | "None"
                | "Dctx0Option"
                | "Dctx0Result"
        )
}
