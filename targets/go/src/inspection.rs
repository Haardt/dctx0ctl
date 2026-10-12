//! Structural Go inventory; compiler selection and execution are separate evidence.
use dctx0_target_sdk::*;
use tree_sitter::{Node, Parser};

pub struct GoInspector;

fn walk(
    node: Node<'_>,
    text: &str,
    declarations: &mut Vec<StructuralDeclaration>,
    imports: &mut Vec<StructuralStatement>,
    references: &mut Vec<DependencyReference>,
    path: &str,
) {
    if node.kind() == "import_spec" {
        if let Some(value) = node.child_by_field_name("path") {
            let literal = &text[value.byte_range()];
            let specifier = if literal.starts_with('`') {
                literal
                    .strip_prefix('`')
                    .and_then(|v| v.strip_suffix('`'))
                    .map(str::to_owned)
            } else {
                serde_json::from_str::<String>(literal).ok()
            };
            imports.push(StructuralStatement {
                text: text[node.byte_range()].into(),
                start_byte: node.start_byte(),
                end_byte: node.end_byte(),
            });
            references.push(DependencyReference {
                source: path.into(),
                kind: DependencyKind::Import,
                specifier,
                start_byte: node.start_byte(),
                end_byte: node.end_byte(),
            });
        }
    }
    if matches!(
        node.kind(),
        "function_declaration"
            | "method_declaration"
            | "type_spec"
            | "type_alias"
            | "var_spec"
            | "const_spec"
    ) {
        if let Some(name) = node.child_by_field_name("name") {
            let name = &text[name.byte_range()];
            declarations.push(StructuralDeclaration {
                name: name.into(),
                kind: node.kind().into(),
                exported: name.chars().next().is_some_and(char::is_uppercase),
                start_byte: node.start_byte(),
                end_byte: node.end_byte(),
            });
        }
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        walk(child, text, declarations, imports, references, path);
    }
}

impl ImplementationInspector for GoInspector {
    fn inspect(&self, sources: &[InspectionSource]) -> Result<InspectionReport, Error> {
        let mut report = InspectionReport {
            schema_version: 1,
            target: "go".into(),
            inspector_version: "dctx0-go-inspector-1".into(),
            sources: vec![],
        };
        let mut sources = sources
            .iter()
            .filter(|s| s.language == InspectionLanguage::Go)
            .collect::<Vec<_>>();
        sources.sort_by(|a, b| a.path.cmp(&b.path));
        for source in sources {
            let mut parser = Parser::new();
            parser
                .set_language(&tree_sitter_go::LANGUAGE.into())
                .map_err(|e| Error::Invalid(e.to_string()))?;
            let tree = parser
                .parse(&source.text, None)
                .ok_or_else(|| Error::Invalid("Go structural parser returned no tree".into()))?;
            let mut item = SourceInspection {
                path: source.path.clone(),
                language: InspectionLanguage::Go,
                syntax_errors: tree.root_node().has_error(),
                declarations: vec![],
                imports: vec![],
                exports: vec![],
            };
            walk(
                tree.root_node(),
                &source.text,
                &mut item.declarations,
                &mut item.imports,
                &mut vec![],
                &source.path,
            );
            report.sources.push(item);
        }
        Ok(report)
    }
}
impl DependencyAnalyzer for GoInspector {
    fn dependencies(&self, sources: &[InspectionSource]) -> Result<DependencyInspection, Error> {
        let inspection = self.inspect(sources)?;
        let mut report = DependencyInspection {
            schema_version: 1,
            analyzer_version: "dctx0-go-dependencies-1".into(),
            sources: inspection.sources.iter().map(|s| s.path.clone()).collect(),
            references: vec![],
            syntax_errors: inspection
                .sources
                .iter()
                .filter(|s| s.syntax_errors)
                .map(|s| s.path.clone())
                .collect(),
        };
        for source in sources
            .iter()
            .filter(|s| s.language == InspectionLanguage::Go)
        {
            let mut parser = Parser::new();
            parser
                .set_language(&tree_sitter_go::LANGUAGE.into())
                .map_err(|e| Error::Invalid(e.to_string()))?;
            let tree = parser
                .parse(&source.text, None)
                .ok_or_else(|| Error::Invalid("Go dependency parser returned no tree".into()))?;
            walk(
                tree.root_node(),
                &source.text,
                &mut vec![],
                &mut vec![],
                &mut report.references,
                &source.path,
            );
        }
        report
            .references
            .sort_by(|a, b| (&a.source, a.start_byte).cmp(&(&b.source, b.start_byte)));
        Ok(report)
    }
}
