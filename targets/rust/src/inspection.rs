//! Rust syntax inventory; macros and names are candidates, never semantic proofs.
use dctx0_target_sdk::*;
use tree_sitter::{Node, Parser};

pub struct RustInspector;

fn walk(
    node: Node<'_>,
    source: &InspectionSource,
    item: &mut SourceInspection,
    references: &mut Vec<DependencyReference>,
) {
    let text = &source.text;
    if matches!(
        node.kind(),
        "use_declaration" | "extern_crate_declaration" | "mod_item"
    ) {
        item.imports.push(StructuralStatement {
            text: text[node.byte_range()].into(),
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
        });
        // Preserve syntax instead of claiming compiler-resolved module paths.
        let specifier = node
            .child_by_field_name("argument")
            .or_else(|| node.child_by_field_name("name"))
            .map(|n| text[n.byte_range()].to_owned());
        references.push(DependencyReference {
            source: source.path.clone(),
            kind: DependencyKind::Import,
            specifier,
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
        });
    }
    if matches!(
        node.kind(),
        "function_item"
            | "struct_item"
            | "enum_item"
            | "trait_item"
            | "type_item"
            | "const_item"
            | "static_item"
            | "mod_item"
            | "macro_definition"
    ) {
        if let Some(name) = node.child_by_field_name("name") {
            let mut cursor = node.walk();
            let exported = node
                .named_children(&mut cursor)
                .any(|n| n.kind() == "visibility_modifier" && text[n.byte_range()] == *"pub");
            item.declarations.push(StructuralDeclaration {
                name: text[name.byte_range()].into(),
                kind: node.kind().into(),
                exported,
                start_byte: node.start_byte(),
                end_byte: node.end_byte(),
            });
        }
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        walk(child, source, item, references);
    }
}

impl RustInspector {
    fn analyze(
        &self,
        sources: &[InspectionSource],
    ) -> Result<(InspectionReport, DependencyInspection), Error> {
        let mut inspection = InspectionReport {
            schema_version: 1,
            target: "rust".into(),
            inspector_version: "dctx0-rust-inspector-1".into(),
            sources: vec![],
        };
        let mut dependencies = DependencyInspection {
            schema_version: 1,
            analyzer_version: "dctx0-rust-dependencies-1".into(),
            sources: vec![],
            references: vec![],
            syntax_errors: vec![],
        };
        let mut selected = sources
            .iter()
            .filter(|s| s.language == InspectionLanguage::Rust)
            .collect::<Vec<_>>();
        selected.sort_by(|a, b| a.path.cmp(&b.path));
        for source in selected {
            let mut parser = Parser::new();
            parser
                .set_language(&tree_sitter_rust::LANGUAGE.into())
                .map_err(|e| Error::Invalid(e.to_string()))?;
            let tree = parser
                .parse(&source.text, None)
                .ok_or_else(|| Error::Invalid("Rust structural parser returned no tree".into()))?;
            let mut item = SourceInspection {
                path: source.path.clone(),
                language: InspectionLanguage::Rust,
                syntax_errors: tree.root_node().has_error(),
                declarations: vec![],
                imports: vec![],
                exports: vec![],
            };
            walk(
                tree.root_node(),
                source,
                &mut item,
                &mut dependencies.references,
            );
            dependencies.sources.push(source.path.clone());
            if item.syntax_errors {
                dependencies.syntax_errors.push(source.path.clone());
            }
            inspection.sources.push(item);
        }
        Ok((inspection, dependencies))
    }
}
impl ImplementationInspector for RustInspector {
    fn inspect(&self, sources: &[InspectionSource]) -> Result<InspectionReport, Error> {
        self.analyze(sources).map(|(report, _)| report)
    }
}
impl DependencyAnalyzer for RustInspector {
    fn dependencies(&self, sources: &[InspectionSource]) -> Result<DependencyInspection, Error> {
        self.analyze(sources).map(|(_, report)| report)
    }
}
