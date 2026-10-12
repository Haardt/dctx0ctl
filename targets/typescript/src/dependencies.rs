//! Tree-sitter dependency syntax, with unresolved expressions retained explicitly.
use dctx0_target_sdk::*;
use tree_sitter::{Node, Parser};
pub struct TypeScriptDependencies;
fn literal(node: Node<'_>, source: &str) -> Option<String> {
    if node.kind() != "string" {
        return None;
    }
    let text = &source[node.byte_range()];
    if text.starts_with('"') {
        serde_json::from_str(text).ok()
    } else {
        text.strip_prefix('\'')?
            .strip_suffix('\'')
            .filter(|text| !text.contains('\\'))
            .map(str::to_owned)
    }
}
fn member_loader(node: Node<'_>, source: &str) -> bool {
    let object = node
        .child_by_field_name("object")
        .map(|n| &source[n.byte_range()]);
    let property = match node.kind() {
        "member_expression" => node
            .child_by_field_name("property")
            .map(|n| source[n.byte_range()].to_owned()),
        "subscript_expression" => node
            .child_by_field_name("index")
            .and_then(|n| literal(n, source)),
        _ => None,
    };
    matches!(
        (object, property.as_deref()),
        (Some("module"), Some("require")) | (Some("require"), Some("resolve"))
    )
}
fn uncertain_loader(node: Node<'_>, source: &str) -> bool {
    let property = match node.kind() {
        "member_expression" => node
            .child_by_field_name("property")
            .map(|n| source[n.byte_range()].to_owned()),
        "subscript_expression" => node
            .child_by_field_name("index")
            .and_then(|n| literal(n, source)),
        _ => None,
    };
    matches!(property.as_deref(), Some("require" | "createRequire")) && !member_loader(node, source)
}
impl DependencyAnalyzer for TypeScriptDependencies {
    fn dependencies(&self, sources: &[InspectionSource]) -> Result<DependencyInspection, Error> {
        let mut report = DependencyInspection {
            schema_version: 1,
            analyzer_version: "tree-sitter-typescript-dependencies-4".into(),
            sources: vec![],
            references: vec![],
            syntax_errors: vec![],
        };
        let mut ordered = sources.iter().collect::<Vec<_>>();
        ordered.sort_by(|a, b| a.path.cmp(&b.path));
        for source in ordered {
            report.sources.push(source.path.clone());
            let mut parser = Parser::new();
            let grammar = match source.language {
                InspectionLanguage::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
                InspectionLanguage::Tsx => tree_sitter_typescript::LANGUAGE_TSX,
                InspectionLanguage::Go | InspectionLanguage::Rust => continue,
            };
            parser
                .set_language(&grammar.into())
                .map_err(|error| Error::Invalid(error.to_string()))?;
            let tree = parser.parse(&source.text, None).ok_or_else(|| {
                Error::Invalid("Dependency syntax parser returned no tree".into())
            })?;
            if tree.root_node().has_error() {
                report.syntax_errors.push(source.path.clone());
            }
            let mut stack = vec![tree.root_node()];
            while let Some(node) = stack.pop() {
                let entry =
                    match node.kind() {
                        "member_expression" | "subscript_expression"
                            if (member_loader(node, &source.text)
                                || uncertain_loader(node, &source.text))
                                && !node.parent().is_some_and(|parent| {
                                    parent.kind() == "assignment_expression"
                                        && parent.child_by_field_name("left") == Some(node)
                                })
                                && !node.parent().is_some_and(|parent| {
                                    parent.kind() == "call_expression"
                                        && parent.child_by_field_name("function") == Some(node)
                                }) =>
                        {
                            Some((DependencyKind::DynamicCode, None))
                        }
                        "import_statement" => {
                            Some((DependencyKind::Import, node.child_by_field_name("source")))
                        }
                        "export_statement" if node.child_by_field_name("source").is_some() => {
                            Some((DependencyKind::Reexport, node.child_by_field_name("source")))
                        }
                        "call_expression" => {
                            let loader = node
                                .child_by_field_name("function")
                                .is_some_and(|function| member_loader(function, &source.text));
                            let uncertain = node
                                .child_by_field_name("function")
                                .is_some_and(|function| uncertain_loader(function, &source.text));
                            let function = node
                                .child_by_field_name("function")
                                .map(|function| &source.text[function.byte_range()]);
                            let argument = node
                                .child_by_field_name("arguments")
                                .and_then(|arguments| arguments.named_child(0));
                            match function {
                                _ if loader => Some((DependencyKind::Require, argument)),
                                _ if uncertain => Some((DependencyKind::DynamicCode, None)),
                                Some("import") => Some((DependencyKind::DynamicImport, argument)),
                                Some("require") => Some((DependencyKind::Require, argument)),
                                Some("eval" | "Function" | "createRequire") => {
                                    Some((DependencyKind::DynamicCode, None))
                                }
                                _ => None,
                            }
                        }
                        "new_expression"
                            if node.child_by_field_name("constructor").is_some_and(
                                |constructor| &source.text[constructor.byte_range()] == "Function",
                            ) =>
                        {
                            Some((DependencyKind::DynamicCode, None))
                        }
                        _ => None,
                    };
                if let Some((kind, argument)) = entry {
                    report.references.push(DependencyReference {
                        source: source.path.clone(),
                        kind,
                        specifier: argument.and_then(|argument| literal(argument, &source.text)),
                        start_byte: node.start_byte(),
                        end_byte: node.end_byte(),
                    });
                }
                let mut cursor = node.walk();
                let children = node.named_children(&mut cursor).collect::<Vec<_>>();
                stack.extend(children.into_iter().rev());
            }
        }
        Ok(report)
    }
}

pub(crate) fn observer_manifest(
    sources: &[InspectionSource],
    project_root_relative: &str,
) -> Result<ModuleObserverManifest, Error> {
    use sha2::{Digest, Sha256};
    let report = TypeScriptDependencies.dependencies(sources)?;
    let mut manifest = ModuleObserverManifest {
        schema_version: 1,
        observer_version: "dctx0-esm-observer-2".into(),
        project_root_relative: project_root_relative.into(),
        sources: vec![],
    };
    for source in sources
        .iter()
        .filter(|source| source.path.ends_with(".mjs"))
    {
        if report.syntax_errors.contains(&source.path) {
            continue;
        }
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
            .map_err(|error| Error::Invalid(error.to_string()))?;
        let tree = parser
            .parse(&source.text, None)
            .ok_or_else(|| Error::Invalid("Observer syntax tree unavailable".into()))?;
        let mut sites = Vec::new();
        for reference in report
            .references
            .iter()
            .filter(|reference| reference.source == source.path && reference.specifier.is_none())
        {
            let Some(node) = tree
                .root_node()
                .descendant_for_byte_range(reference.start_byte, reference.end_byte)
            else {
                continue;
            };
            if node.byte_range() != (reference.start_byte..reference.end_byte) {
                continue;
            }
            if matches!(node.kind(), "member_expression" | "subscript_expression") {
                // Executed opaque loader access cannot resolve a dependency.
                sites.push(ModuleObserverSite {
                    start_byte: reference.start_byte,
                    end_byte: reference.end_byte,
                    argument_start: reference.start_byte,
                    argument_end: reference.end_byte,
                });
                continue;
            }
            if node.kind() != "call_expression" {
                continue;
            }
            let Some(function) = node.child_by_field_name("function") else {
                continue;
            };
            let name = &source.text[function.byte_range()];
            if !matches!(name, "import" | "require")
                && !member_loader(function, &source.text)
                && !uncertain_loader(function, &source.text)
            {
                continue;
            }
            let Some(arguments) = node.child_by_field_name("arguments") else {
                continue;
            };
            if arguments.named_child_count() != 1 {
                continue;
            }
            let Some(argument) = arguments.named_child(0) else {
                continue;
            };
            if argument.kind() == "spread_element" {
                continue;
            }
            sites.push(ModuleObserverSite {
                start_byte: reference.start_byte,
                end_byte: reference.end_byte,
                argument_start: argument.start_byte(),
                argument_end: argument.end_byte(),
            });
        }
        let intervals = sites
            .iter()
            .map(|site| (site.argument_start, site.argument_end))
            .collect::<Vec<_>>();
        sites.retain(|site| {
            !intervals.iter().any(|(start, end)| {
                (*start, *end) != (site.argument_start, site.argument_end)
                    && *start < site.argument_end
                    && site.argument_start < *end
            })
        });
        if !sites.is_empty() {
            sites.sort_by_key(|site| site.start_byte);
            manifest.sources.push(ModuleObserverSource {
                source: source.path.clone(),
                sha256: format!("{:x}", Sha256::digest(source.text.as_bytes())),
                sites,
            });
        }
    }
    manifest
        .sources
        .sort_by(|left, right| left.source.cmp(&right.source));
    Ok(manifest)
}
