//! Compile operation state selectors.
use super::*;

pub(super) fn compile_state_selector<'a>(
    node: &'a SemanticNode,
    context: &mut ExpressionContext<'a>,
    expressions: &mut Vec<Expression>,
) {
    let terms = &node.terms;
    for marker in ["by", "where", "from"] {
        if let Some(i) = top_level(terms, 2, marker) {
            let end = ["by", "where", "from"]
                .iter()
                .filter_map(|m| top_level(terms, i + 1, m))
                .min()
                .unwrap_or(terms.len());
            let expected = if marker == "where" {
                Some(TypeRef::builtin("Boolean"))
            } else {
                None
            };
            let mut selected = &terms[i + 1..end];
            if marker == "by"
                && selected.first().is_some_and(|t| t.text == "(")
                && selected.last().is_some_and(|t| t.text == ")")
            {
                selected = &selected[1..selected.len() - 1];
            }
            let mut parts = Vec::new();
            let mut start = 0;
            let mut depth = 0usize;
            for (index, token) in selected.iter().enumerate() {
                if matches!(token.text.as_str(), "(" | "[" | "{") {
                    depth += 1;
                }
                if matches!(token.text.as_str(), ")" | "]" | "}") {
                    depth = depth.saturating_sub(1);
                }
                if marker == "by" && token.text == "," && depth == 0 {
                    parts.push(&selected[start..index]);
                    start = index + 1;
                }
            }
            parts.push(&selected[start..]);
            for part in parts {
                if let Some(value) = parse_expression(part, context, expected.as_ref()) {
                    expressions.push(value);
                }
            }
        }
    }
}
