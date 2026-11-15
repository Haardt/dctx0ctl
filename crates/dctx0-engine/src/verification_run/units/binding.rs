//! Stable source/recipe identities; captured paths remain only in original provenance.
use super::*;
use dctx0_target_sdk::{InspectionLanguage, InspectionSource};

fn hash_file(path: &Path) -> Result<String> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() || metadata.len() > 268_435_456 {
        return Err(Error::Invalid(
            "Unit dependency exceeds its file budget".into(),
        ));
    }
    Ok(digest(&fs::read(path)?))
}
fn discovery(path: &Path) -> Result<String> {
    build::discovery_hash(&path.to_string_lossy())
}

/// Refresh a retained compiled recipe for read-only freshness. Recipe source is
/// bound to the verifier image; publication separately pins the configuration
/// and fixture documents, so this path never regenerates or launches a harness.
pub(in crate::verification_run) fn refresh_captured(
    root: &Path,
    config: &VerificationConfiguration,
    original: &UnitEvidenceBindingV1,
) -> Result<UnitEvidenceBindingV1> {
    let mut binding = original.clone();
    for (name, hash) in &mut binding.dependencies {
        if let Some(path) = name.strip_prefix("file:") {
            *hash = hash_file(Path::new(path))?;
        }
    }
    if let UnitDependencyBoundary::Complete {
        discovery: entries, ..
    } = &mut binding.unit.boundary
    {
        for (path, hash) in entries {
            *hash = discovery(Path::new(path))?;
        }
    }
    let environment = materialize::controlled_environment(root, config)?;
    let entries = environment
        .entries()
        .map_err(|error| Error::Invalid(error.to_string()))?;
    let go = crate::native::identity::resolve_program_with_path(
        &config.runtime,
        root,
        crate::native::environment_value(&entries, "PATH"),
    )
    .map_err(|error| Error::Invalid(error.to_string()))?;
    binding.environment = entries
        .into_iter()
        .map(|(name, value)| {
            Ok((
                name.into_string()
                    .map_err(|_| Error::Invalid("Non-UTF8 unit environment".into()))?,
                Some(digest(value.as_encoded_bytes())),
            ))
        })
        .collect::<Result<_>>()?;
    let verifier = crate::native::VerifierIdentity::capture()
        .map_err(|error| Error::Invalid(error.to_string()))?;
    binding
        .semantics
        .insert("verifier".into(), unit_planner::evidence_digest(&verifier)?);
    binding
        .semantics
        .insert("toolchain".into(), hash_file(&go)?);
    binding.semantics.insert(
        "evaluator".into(),
        unit_planner::evidence_digest(&dctx0_verification::VERIFICATION_VERSION)?,
    );
    Ok(binding)
}

pub(super) fn allowed_sources(
    store: &Store,
    project: &str,
    source: Option<&str>,
) -> Result<Option<BTreeSet<String>>> {
    let Some(source) = source else {
        return Ok(None);
    };
    match store.evidence_composition(project, source) {
        Ok(composition) => Ok(Some(
            composition
                .units
                .iter()
                .filter_map(|unit| unit.proof.as_ref().map(|p| p.native_run.clone()))
                .collect(),
        )),
        Err(_) => {
            store.verification_run_summary(project, source)?;
            Ok(Some([source.into()].into()))
        }
    }
}

pub(in crate::verification_run) fn current(
    project: &Project,
    program: &ProgramPlan,
    group: &DecoderUnitGroup,
    recipe: &materialize::UnitRecipe,
    old: Option<&UnitEvidenceBindingV1>,
) -> Result<UnitEvidenceBindingV1> {
    let logical = catalog::logical_unit_catalog(program).map_err(Error::Invalid)?;
    let cases = dctx0_verification::case_manifest(program).required_cases;
    let snapshot = project.load(&program.snapshot)?;
    let roots = logical
        .iter()
        .flat_map(|u| u.requirements.iter().cloned())
        .collect();
    let requirements = snapshot
        .graph
        .index()
        .verification_closure(&roots)
        .into_iter()
        .map(|id| {
            let p = snapshot
                .fingerprints
                .get(&id)
                .ok_or_else(|| Error::Invalid(format!("UNIT_SEMANTIC_INPUT_MISSING:{id}")))?;
            Ok((
                id,
                unit_planner::evidence_digest(&(&p.contract, &p.behavior, &p.facts))?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let mut logical_cases = BTreeMap::new();
    for unit in &logical {
        for case in &unit.cases {
            logical_cases.insert(
                case.clone(),
                unit_planner::evidence_digest(&(&unit.description_hash, case))?,
            );
        }
    }
    let mut dependencies = old.map(|b| b.dependencies.clone()).unwrap_or_default();
    for (name, hash) in &mut dependencies {
        if let Some(path) = name.strip_prefix("file:") {
            *hash = hash_file(Path::new(path))
                .unwrap_or(unit_planner::evidence_digest(&("missing", name))?);
        }
    }
    for (path, bytes) in &recipe.files {
        dependencies.insert(format!("recipe:{path}"), digest(bytes));
    }
    let boundary = match old.map(|b| &b.unit.boundary) {
        Some(UnitDependencyBoundary::Complete {
            analyzer,
            version,
            roots,
            discovery: prior,
        }) => {
            let discovery = prior
                .keys()
                .map(|path| {
                    Ok((
                        path.clone(),
                        discovery(Path::new(path))
                            .unwrap_or(unit_planner::evidence_digest(&("missing", path))?),
                    ))
                })
                .collect::<Result<BTreeMap<_, _>>>()?;
            UnitDependencyBoundary::Complete {
                analyzer: analyzer.clone(),
                version: version.clone(),
                roots: roots.clone(),
                discovery,
            }
        }
        _ => UnitDependencyBoundary::Unknown {
            group: group.id.clone(),
            reasons: ["COMPILED_PROCESS_CLOSURE_NOT_CAPTURED".into()].into(),
        },
    };
    let entries = recipe
        .configuration
        .environment
        .as_ref()
        .expect("controlled recipe")
        .entries()
        .map_err(|e| Error::Invalid(e.to_string()))?;
    let environment = entries
        .into_iter()
        .map(|(key, value)| {
            Ok((
                key.into_string()
                    .map_err(|_| Error::Invalid("Non-UTF8 unit environment".into()))?,
                Some(digest(value.as_encoded_bytes())),
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let verifier =
        crate::native::VerifierIdentity::capture().map_err(|e| Error::Invalid(e.to_string()))?;
    Ok(UnitEvidenceBindingV1 {
        schema_version: 1,
        unit: VerificationUnit {
            id: group.id.clone(),
            cases,
            boundary,
            execute_always: false,
        },
        logical_cases,
        requirements,
        dependencies,
        fixtures: BTreeMap::new(),
        build_inputs: recipe
            .files
            .iter()
            .map(|(path, bytes)| (path.clone(), digest(bytes)))
            .collect(),
        environment,
        semantics: [
            (
                "decoder-roots".into(),
                unit_planner::evidence_digest(&group.decoders)?,
            ),
            (
                "budgets".into(),
                unit_planner::evidence_digest(&program.budgets)?,
            ),
            (
                "configuration".into(),
                unit_planner::evidence_digest(&recipe.configuration)?,
            ),
            ("verifier".into(), unit_planner::evidence_digest(&verifier)?),
            ("toolchain".into(), hash_file(&recipe.go)?),
            (
                "evaluator".into(),
                unit_planner::evidence_digest(&dctx0_verification::VERIFICATION_VERSION)?,
            ),
            (
                "compiler".into(),
                unit_planner::evidence_digest(&snapshot.ir.compiler_version)?,
            ),
            (
                "generator".into(),
                unit_planner::evidence_digest(&program.generator_version)?,
            ),
        ]
        .into(),
    })
}

pub(super) fn compiled(
    context: &UnitNativeContext,
    receipt: &build::GoBuildReceipt,
    generator: &dyn HarnessGenerator,
) -> Result<UnitEvidenceBindingV1> {
    if receipt.schema_version != 2 || receipt.module_root.is_empty() {
        return Err(Error::Invalid(
            "Source-bound unit compilation needs a schema-2 module receipt".into(),
        ));
    }
    let mut binding = context.planned.clone();
    let mut dependencies = BTreeMap::new();
    let mut sources = Vec::new();
    let module_root = Path::new(&receipt.module_root);
    let goroot = context
        .recipe_go
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| Error::Invalid("Go toolchain root missing".into()))?;
    let mut compiled_roots = BTreeSet::new();
    for (path, hash) in &receipt.files {
        let path = Path::new(path);
        let key = if let Ok(relative) = path.strip_prefix(module_root) {
            let name = relative.to_string_lossy().replace('\\', "/");
            if context
                .recipe_files
                .get(&name)
                .is_some_and(|bytes| digest(bytes) == *hash)
            {
                format!("recipe:{name}")
            } else if matches!(
                name.as_str(),
                ".dctx0/testing/go/contract.go" | ".dctx0/testing/go/worker.go"
            ) {
                format!("generated:{name}")
            } else {
                return Err(Error::Invalid(format!(
                    "UNIT_UNDECLARED_BUILD_INPUT:{name}"
                )));
            }
        } else {
            format!("file:{}", path.to_string_lossy())
        };
        dependencies.insert(key.clone(), hash.clone());
        if path.extension().is_some_and(|e| e == "go") && !path.starts_with(goroot) {
            // Generated transport owns stdin/stdout and process lifecycle. Other
            // selected code is inspected as a whole compiled process cohort.
            if !key.starts_with("generated:") {
                sources.push(InspectionSource {
                    path: key.clone(),
                    language: InspectionLanguage::Go,
                    text: fs::read_to_string(path)?,
                });
                compiled_roots.insert(key);
            }
        } else if !path.starts_with(goroot)
            && matches!(
                path.extension().and_then(|v| v.to_str()),
                Some("s" | "syso" | "h")
            )
        {
            return Err(Error::Invalid(
                "UNIT_NATIVE_CODE_BOUNDARY_UNSUPPORTED".into(),
            ));
        }
    }
    for (path, bytes) in &context.recipe_files {
        dependencies.insert(format!("recipe:{path}"), digest(bytes));
    }
    for decoder in &context.group.decoders {
        if !receipt.packages.contains(&decoder.import_path) {
            return Err(Error::Invalid(
                "Compiled decoder package is absent from native receipt".into(),
            ));
        }
    }
    let roots = BTreeMap::from([
        ("compiled-process".into(), compiled_roots.clone()),
        (
            "producer-adapter-observer-setup-cleanup".into(),
            compiled_roots,
        ),
    ]);
    let mut boundary = generator
        .unit_dependency_boundary(&context.group.id, &roots, &sources)
        .map_err(|e| Error::Invalid(e.to_string()))?;
    if let UnitDependencyBoundary::Complete { discovery, .. } = &mut boundary {
        *discovery = receipt.discovery.clone();
    }
    binding.unit.boundary = boundary;
    binding.dependencies = dependencies;
    if !unit_planner::valid_unit_binding(&binding)
        || matches!(
            binding.unit.boundary,
            UnitDependencyBoundary::Unknown { .. }
        )
    {
        return Err(Error::Invalid(format!(
            "UNIT_COMPILED_BOUNDARY_UNPROVEN: {}",
            serde_json::to_string(&binding.unit.boundary)?
        )));
    }
    Ok(binding)
}
