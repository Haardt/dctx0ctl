//! Tree-sitter inventory is structural evidence, never a semantic type proof.
use dctx0_target_sdk::*;
use tree_sitter::{Node, Parser};
pub struct TypeScriptInspector;

fn statement(node: Node<'_>, source: &str) -> StructuralStatement {
    StructuralStatement {
        text: source[node.byte_range()].into(),
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
    }
}
fn declaration(
    node: Node<'_>,
    exported: bool,
    source: &str,
    output: &mut Vec<StructuralDeclaration>,
) {
    if matches!(node.kind(), "lexical_declaration" | "variable_declaration") {
        let mut cursor = node.walk();
        for child in node
            .named_children(&mut cursor)
            .filter(|n| n.kind() == "variable_declarator")
        {
            declaration(child, exported, source, output);
        }
        return;
    }
    if !matches!(
        node.kind(),
        "function_declaration"
            | "generator_function_declaration"
            | "class_declaration"
            | "abstract_class_declaration"
            | "interface_declaration"
            | "type_alias_declaration"
            | "enum_declaration"
            | "variable_declarator"
    ) {
        return;
    }
    let Some(name) = node.child_by_field_name("name") else {
        return;
    };
    // Destructuring is retained in syntax, but does not invent individual names.
    if !matches!(name.kind(), "identifier" | "type_identifier") {
        return;
    }
    output.push(StructuralDeclaration {
        name: source[name.byte_range()].into(),
        kind: node.kind().into(),
        exported,
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
    });
    if matches!(
        node.kind(),
        "class_declaration" | "abstract_class_declaration"
    ) {
        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            for member in body
                .named_children(&mut cursor)
                .filter(|m| matches!(m.kind(), "method_definition" | "public_field_definition"))
            {
                let mut modifiers = member.walk();
                if member.named_children(&mut modifiers).any(|m| {
                    m.kind() == "accessibility_modifier"
                        && matches!(&source[m.byte_range()], "private" | "protected")
                }) {
                    continue;
                }
                let Some(member_name) = member.child_by_field_name("name") else {
                    continue;
                };
                if !matches!(member_name.kind(), "property_identifier" | "identifier")
                    || &source[member_name.byte_range()] == "constructor"
                {
                    continue;
                }
                output.push(StructuralDeclaration {
                    name: format!(
                        "{}.{}",
                        &source[name.byte_range()],
                        &source[member_name.byte_range()]
                    ),
                    kind: member.kind().into(),
                    exported,
                    start_byte: member.start_byte(),
                    end_byte: member.end_byte(),
                });
            }
        }
    }
}
impl ImplementationInspector for TypeScriptInspector {
    fn inspect(&self, sources: &[InspectionSource]) -> Result<InspectionReport, Error> {
        let mut report = InspectionReport {
            schema_version: 1,
            target: "typescript".into(),
            inspector_version: "tree-sitter-typescript-0.23.2/inspection-1".into(),
            sources: vec![],
        };
        let mut ordered = sources.iter().collect::<Vec<_>>();
        ordered.sort_by(|a, b| a.path.cmp(&b.path));
        for source in ordered {
            let mut parser = Parser::new();
            let grammar = match source.language {
                InspectionLanguage::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
                InspectionLanguage::Tsx => tree_sitter_typescript::LANGUAGE_TSX,
            };
            parser
                .set_language(&grammar.into())
                .map_err(|e| Error::Invalid(e.to_string()))?;
            let tree = parser
                .parse(&source.text, None)
                .ok_or_else(|| Error::Invalid("Structural parser returned no tree".into()))?;
            let root = tree.root_node();
            let mut inspected = SourceInspection {
                path: source.path.clone(),
                language: source.language,
                syntax_errors: root.has_error(),
                declarations: vec![],
                imports: vec![],
                exports: vec![],
            };
            let mut cursor = root.walk();
            for node in root.named_children(&mut cursor) {
                match node.kind() {
                    "import_statement" => inspected.imports.push(statement(node, &source.text)),
                    "export_statement" => {
                        inspected.exports.push(statement(node, &source.text));
                        let mut children = node.walk();
                        for child in node.named_children(&mut children) {
                            declaration(child, true, &source.text, &mut inspected.declarations);
                        }
                    }
                    _ => declaration(node, false, &source.text, &mut inspected.declarations),
                }
            }
            report.sources.push(inspected);
        }
        Ok(report)
    }
}
