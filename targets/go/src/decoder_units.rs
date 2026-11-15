//! Direct decoder adapters isolate complete compiler-resolved native cohorts.
use dctx0_core::{ContractDecoderBinding, UnitDependencyBoundary};
use dctx0_target_sdk::*;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn adapter(bindings: &[ContractDecoderBinding]) -> Result<Artifact, Error> {
    let mut imports = BTreeMap::new();
    let mut subjects = String::new();
    let mut calls = String::new();
    let mut seen = BTreeSet::new();
    for binding in bindings {
        if binding.function.is_empty()
            || !binding
                .function
                .bytes()
                .enumerate()
                .all(|(i, b)| b.is_ascii_alphabetic() || b == b'_' || (i > 0 && b.is_ascii_digit()))
            || binding.import_path.is_empty()
            || !binding
                .import_path
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"/.-_".contains(&b))
            || binding
                .import_path
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
            || !seen.insert(&binding.subject)
        {
            return Err(Error::Invalid(
                "Invalid or duplicate direct decoder binding".into(),
            ));
        }
        let next = imports.len();
        let index = *imports.entry(binding.import_path.clone()).or_insert(next);
        let subject =
            serde_json::to_string(&binding.subject).map_err(|e| Error::Invalid(e.to_string()))?;
        subjects.push_str(&format!("{{Subject:{subject},Role:\"contract\",Artifact:\".dctx0/testing/go/adapter.go\",EntryPoint:\"decoderSession.Validate\"}},\n"));
        calls.push_str(&format!(
            "case {subject}: return callDecoder(subject,value,decoder{index}.{})\n",
            binding.function
        ));
    }
    if bindings.is_empty() {
        return Err(Error::Invalid("Direct decoder unit is empty".into()));
    }
    let imports = imports
        .into_iter()
        .map(|(package, index)| {
            format!(
                "decoder{index} {}",
                serde_json::to_string(&package).expect("string")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok(Artifact {
        path: "adapter.go".into(),
        contents: include_str!("../harness/decoder-unit.go")
            .replace("// __DECODER_IMPORTS__", &imports)
            .replace("// __DECODER_SUBJECTS__", &subjects)
            .replace("// __DECODER_CALLS__", &calls),
    })
}

pub(super) fn boundary(
    group: &str,
    roots: &BTreeMap<String, BTreeSet<String>>,
    sources: &[InspectionSource],
) -> Result<UnitDependencyBoundary, Error> {
    let report = crate::GoInspector.dependencies(sources)?;
    let mut reasons = BTreeSet::new();
    if !report.syntax_errors.is_empty()
        || !roots.contains_key("compiled-process")
        || sources.is_empty()
    {
        reasons.insert("COMPILED_PROCESS_SOURCE_CLOSURE_MISSING".into());
    }
    for reference in &report.references {
        if reference.specifier.as_deref().is_none_or(|name| {
            // Standard packages are closed explicitly: an unlisted package
            // may expose process state, filesystem, network, clocks or loaders.
            // Nonstandard packages are inspected from their compiler-selected
            // source closure, rather than trusting their import declarations.
            let external = name
                .split('/')
                .next()
                .is_some_and(|head| head.contains('.'));
            !external
                && !matches!(
                    name,
                    "bytes"
                        | "cmp"
                        | "encoding/base64"
                        | "encoding/binary"
                        | "encoding/hex"
                        | "encoding/json"
                        | "errors"
                        | "fmt"
                        | "io"
                        | "iter"
                        | "maps"
                        | "math"
                        | "math/big"
                        | "math/bits"
                        | "reflect"
                        | "regexp"
                        | "regexp/syntax"
                        | "slices"
                        | "sort"
                        | "strconv"
                        | "strings"
                        | "sync"
                        | "sync/atomic"
                        | "unicode"
                        | "unicode/utf8"
                        | "unicode/utf16"
                )
        }) {
            reasons.insert(format!(
                "EXTERNAL_OR_DYNAMIC_PROCESS_INPUT:{}",
                reference.source
            ));
        }
    }
    for source in sources {
        if crate::standard_purity::external_format_access(source)? {
            reasons.insert(format!(
                "AMBIENT_STANDARD_IO_OR_OPAQUE_ALIAS:{}",
                source.path
            ));
        }
    }
    if !reasons.is_empty() {
        return Ok(UnitDependencyBoundary::Unknown {
            group: group.into(),
            reasons,
        });
    }
    Ok(UnitDependencyBoundary::Complete {
        analyzer: "go-isolated-compiled-process".into(),
        version: "2".into(),
        roots: roots.clone(),
        discovery: BTreeMap::new(),
    })
}
