//! `fmt` also exposes ambient stdin/stdout. An import allowlist alone cannot
//! establish a closed decoder: permit only its value-returning formatting API.
use dctx0_target_sdk::{Error, InspectionSource};
use tree_sitter::{Node, Parser};

fn literal(text: &str) -> Option<String> {
    if text.starts_with('`') {
        text.strip_prefix('`')
            .and_then(|value| value.strip_suffix('`'))
            .map(str::to_owned)
    } else {
        serde_json::from_str(text).ok()
    }
}

fn nodes(root: Node<'_>) -> Vec<Node<'_>> {
    let mut result = Vec::new();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        result.push(node);
        let mut cursor = node.walk();
        pending.extend(node.named_children(&mut cursor));
    }
    result
}

pub(super) fn external_format_access(source: &InspectionSource) -> Result<bool, Error> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_go::LANGUAGE.into())
        .map_err(|error| Error::Invalid(error.to_string()))?;
    let tree = parser
        .parse(&source.text, None)
        .ok_or_else(|| Error::Invalid("Go purity parser returned no tree".into()))?;
    if tree.root_node().has_error() {
        return Ok(true);
    }
    let all = nodes(tree.root_node());
    let mut aliases = Vec::new();
    for node in all.iter().filter(|node| node.kind() == "import_spec") {
        let Some(path) = node.child_by_field_name("path") else {
            return Ok(true);
        };
        if literal(&source.text[path.byte_range()]).as_deref() != Some("fmt") {
            continue;
        }
        let name = node
            .child_by_field_name("name")
            .map(|node| &source.text[node.byte_range()])
            .unwrap_or("fmt");
        if matches!(name, "." | "_") {
            return Ok(true);
        }
        aliases.push(name);
    }
    for node in all
        .iter()
        .filter(|node| node.kind() == "selector_expression")
    {
        let Some(operand) = node.child_by_field_name("operand") else {
            return Ok(true);
        };
        if !aliases.contains(&&source.text[operand.byte_range()]) {
            continue;
        }
        let Some(field) = node.child_by_field_name("field") else {
            return Ok(true);
        };
        if !matches!(
            &source.text[field.byte_range()],
            "Errorf"
                | "Sprint"
                | "Sprintf"
                | "Sprintln"
                | "Append"
                | "Appendf"
                | "Appendln"
                | "FormatString"
                | "State"
                | "Formatter"
                | "Stringer"
                | "GoStringer"
        ) {
            return Ok(true);
        }
    }
    Ok(false)
}
