# Repository Guidelines

## Project Direction

The Cargo workspace implements parsing, multi-file composition, name resolution, typed expression IR, semantic dependency graphs, change fingerprints, SQLite history, and explicit renames. A language-neutral verification foundation is being introduced; four target crates provide editable implementation profiles. Interpreter work is resumed within the external verification stages; native execution and automatic adoption remain planned. Port the core and reference fixtures first, complete TypeScript next, stabilize the SDK through conformance tests, then add Go and Java. Document intentional compatibility changes.

## Current Capabilities & Processing Flow

Eleven crates implement compiler, change-tracking and pure verification foundations, and four target crates supply editable TypeScript, Java, Go and Rust profiles. Both complete examples compile: Cinema (2 sources) and runatron-next (18 sources). The CLI provides `parse`, `check`, `history`, `diff`, `impact`, explicit `rename` with `--dry-run`, phase benchmarks, target-neutral `tasks` planning, queries, guarded work transitions, slices and external check/review reports, and read-only `verify plan` with published subject/task requirements. Task commands default to Markdown; `--json` supplies machine-readable reports.

Processing follows sorted source discovery → lexer/parser and AST → workspace/fragment composition, scoped name resolution and type checking → validated IR → semantic graph → fingerprints and snapshot/diff publication. Diagnostics retain source files, codes, and positions. Specs remain reviewed `.dctx0` files; SQLite stores derived history. `check` accepts a spec directory and preserves the last valid head when compilation fails. `parse` also accepts one file.

Diff facets distinguish contracts (declarations/signatures), behavior (modeled expressions/clauses), facts (intent/constraints/decisions), and location. Impact uses old and new dependency graphs and reports deterministic explanation paths. Semantic cycles are supported; tasks use a separate acyclic prerequisites graph.

Compilation validates modeled rules; it does not execute examples, invariants, properties, or lifecycles. The pure expression interpreter, deterministic input generation, validated-IR plan projection and explicit finite-plan assessment are implemented foundations. A separate target SDK and generated Bun transport are implemented. Managed native verification, automatic adoption, agent execution and UI remain future work. Opaque trace labels retain an unmodeled status; never present them as resolved dependencies or execution evidence. Explicit rename currently covers declarations and scoped descendants; fields, parameters, and enum alternatives need dedicated operations.

The Cinema scenario increment (2026-10-10) passes 202 Rust tests and 8 Python tests, plus 4 Bun conformance tests, with 100% merged source-line (14,526 lines) and LLVM function coverage (1,232 functions). Preserve raw LLVM metrics separately; this is not a claim of complete branch coverage. Current task validation is recorded in `docs/site/src/reference/tasks-validation.md` and `docs/site/src/reference/coverage.md`; earlier refactoring evidence remains in `docs/site/src/reference/refactoring.md` and `benchmarks/refactoring-comparison.json`. Update this dated status when capabilities change.

## Project Structure & Architecture

Use one Cargo workspace with shared dependencies, metadata, lint configuration, and a committed `Cargo.lock`:

- `crates/dctx0-core/`: AST, validated IR, values, diagnostics, and language-neutral binding models.
- `crates/dctx0-language/`: lexer, parser, name resolution, and type checking.
- `crates/dctx0-interpreter/`: pure budgeted expression evaluation and suspended pure-operation calls; state execution remains planned.
- `crates/dctx0-verification/`: deterministic corpora, published IR projections, ordered scenarios, lifecycle paths and central finite-plan assessment; native orchestration, shrinking and replay remain planned.
- `crates/dctx0-target-sdk/`: separate versioned native transport/artifact contracts; generated Bun conformance is implemented, full behavior execution conformance remains planned.
- `crates/dctx0-graph/`: semantic dependencies, cycles, governing facts, and impact paths.
- `crates/dctx0-changes/`: source/semantic hashes, contract/behavior/fact fingerprints, and snapshot diffs.
- `crates/dctx0-tasks/`: pure task planning, typed context, dependency DAG, reconciliation and slices.
- `crates/dctx0-store/`: transactional SQLite snapshots, runs, rename events and versioned task history.
- `crates/dctx0-engine/`: filesystem composition, checks, history, and journaled rename orchestration.
- `crates/dctx0-cli/`: commands and target registration.
- `targets/{typescript,java,go,rust}/`: separate profile-provider crates with embedded editable Markdown defaults; TypeScript also supplies testing-only Bun transport artifacts, while complete native verification and other execution targets remain deferred.
- `fixtures/{specs,conformance}/`: shared compatibility and target fixtures.

Dependencies point toward the core; the core must never import targets. Keep AST and validated IR separate. Targets consume validated IR. The CLI composes the engine and selected targets.

## Task Planning Direction

Task planning and administration are implemented in `dctx0-tasks`, with SQLite schema version 3 and `dctx0 tasks` Markdown/JSON commands. See `docs/site/src/reference/tasks.md` for the implemented API. Task creation, waves and understandable task inspection remain established workflows. Pure interpreter work resumes for the adaptive verification stages documented in `docs/site/src/reference/verification-architecture.md`. See `docs/site/src/reference/tasks-analysis.md` for original-code findings, confirmed requirements, and the proposed increment.

Task creation is target-neutral: derive work from a valid snapshot, intents, constraints, decisions, and examples. Language/framework choice, file projections, native checks, and agent execution belong to later stages. Use a pure core-owned planner, a separate task DAG, versioned SQLite plans, and read-only list/show queries. Every hard prerequisite needs an explainable reason; semantic calls and write-resource conflicts are separate relationships. The first wave has no open prerequisites; tasks within each wave are independent. Release later work after its prerequisite results are delivered. Provide on-demand task context and predecessor results with pinned snapshot provenance.

Work progress is Open, Progress, or Done. Testing evidence is separate: unverified, PASS, or FAIL, scoped to the task requirements and tested implementation. Done must never imply PASS. Readiness and concrete blockers are derived separately.

A relevant specification change creates new change tasks with new IDs; never automatically reopen or rewrite historical Done/PASS tasks. Link new tasks to predecessors with changed subjects/facts and an explicit reason. History links are not scheduling prerequisites. Model many-to-many links for splits/merges and follow explicit rename history; add no universal `stableId` to specs. Keep unchanged tasks and make repeated planning idempotent; a genuine later reversion is a new change, not reuse of an old PASS. Supersession may remove obsolete unfinished work from the active plan, but must not falsely mark it Done/PASS. Preserve records and result history transactionally.

Historical PASS tasks are implementation context for their old requirements, not proof of the current spec. Produce delta-oriented tasks explaining existing results, changed requirements, and requested adaptations. Generate context on demand; listing saved tasks must not claim working-tree freshness.

The implemented task guide supersedes the earlier API drafts in `docs/site/src/reference/tasks-api.md`. Use one filtered list query for wave/state/kind/subject combinations. Keep read-only `next` separate from atomic `claim_next`; parallel workers must not receive the same task. Read-only `show` and `next` include the actual assignment, contract-specific acceptance and required paginated context. Include complete original declarations for entities/messages/events and required types; avoid duplicate own-type excerpts and initial change noise. Derive presentation from pinned snapshots without rewriting saved definitions. Show/context/history queries expose pinned requirements, predecessor results, changes, and blocker reasons. Work transitions use explicit start/release/complete actions rather than a generic status setter. Review may be a normal task when explicitly required, but LLM review is separate from testing PASS and review execution is deferred.

Context delivery and atomic slice reservation are implemented; actual agent execution remains deferred. Historical sizing evidence remains in `docs/site/src/reference/tasks-context.md`. Default context should contain the current task's essential requirements and compact prerequisite/history overviews; full predecessor tasks and historical bodies are loaded explicitly by ID/section. Do not silently truncate mandatory constraints, examples, or type contracts. Report incomplete sections and pinned cursors; token counts must name their tokenizer and serialization. `docs/task-context-sizing.json` measures historical artifacts with a reference tokenizer, not new Rust tasks or whole agent-run consumption.

A later-wave task may anchor a dependency-closed work slice. Traverse its actual prerequisites, not every task in earlier waves; historical links stay separate. Show all ancestor overviews, delivered boundary results, open work, external blockers, and tasks owned by other workers. A slice is independently actionable only when every prerequisite is included or supplied by an appropriate existing result. Bundle reservation must be atomic; preserve individual task IDs, work states, wave order, history, and testing evidence. Reserve later tasks without claiming their prerequisites have already been delivered. Viewing context alone never authorizes or reserves execution.

## Target Extension Contract

Each target supplies a versioned manifest (ID, SDK compatibility, capabilities, profiles), binding analysis, artifact generation, native implementation verification, and behavior execution. Define focused traits such as `BindingAnalyzer`, `ArtifactGenerator`, `ImplementationVerifier`, and `BehaviorExecutor`; a target bundles these components. Adding a target requires a crate, registration, and passing the shared conformance suite without changing language semantics.

Initially link targets statically, with Cargo features selecting shipped targets. If independently installed targets become necessary, add a versioned JSON/stdio process protocol implementing the same contract; avoid native Rust dylibs as the initial plugin mechanism.

Separate languages from framework profiles: `java` with `spring-web`, `typescript` with `hono`, and `go` with `net-http`. Keep binding normalization and coverage checks language-neutral; resolve symbols and validate profile payloads inside targets. Preserve strict YAML validation: reject duplicate keys, aliases, anchors, merge keys, custom tags, unknown fields, and escaping project paths; retain source locations.

Centralize interpretation, test corpus generation, seeds, and replay semantics. Targets execute common test plans against actual implementations. Retain `PASS`, `FAIL`, and `UNKNOWN`, explicit unsupported capabilities, and versioned evidence. Never report success for unexecuted checks. Keep generation and reports deterministic.

## Mandatory Example Acceptance (2026-10-12)

Cinema is the mandatory complete specification, shared unchanged between
`examples/cinema/dctx0` and `examples/cinema-go/dctx0`. Both actual applications
must pass the central finite plan, their real persistence/HTTP checks, meaningful
mutations and replay. The user explicitly removed runatron-next and requested
removal of its compiler tests, goldens and measurement workloads; do not restore
it as a fixture or native application. Historical evidence remains historical.

## Libraries & Tooling

Use `petgraph` for semantic graph algorithms, `rusqlite` with bundled SQLite for local history, `sha2` for deterministic fingerprints, and `fs2` for project locks. Use `serde`/`serde_json` for data contracts, `thiserror` for library errors, `clap` for CLI parsing, and `tokio` for bounded subprocess orchestration, timeouts, and cancellation. Ensure timed-out compiler/harness processes are terminated and reaped.

Start with a direct port of the existing handwritten DSL parser for compatibility; evaluate `chumsky` on a small language slice. Use `ariadne` for source diagnostics. Evaluate `saphyr` against the strict binding rules before selecting YAML tooling.

Use `tree-sitter` for target syntax and architecture analysis, not semantic type proofs. Use Tree-sitter for structural inventory and the external harness for behavioral verification. An additional TypeScript Compiler API/LSP analysis layer is outside this increment. Normal native build and execution tools remain permitted. Native-language helper programs may live with their Rust target crates.

Use `insta` for IR, diagnostic, and generated-artifact snapshots; use `proptest` for Rust core properties. Do not silently replace the language-neutral deterministic test generator with `proptest`. Pin compatible dependencies centrally and add libraries only when needed.

## Build, Test, and Development Commands

Use `python3 scripts/validate.py --coverage` for validation paired with release speed/RAM measurements. It runs the relevant checks below; raw Cargo tests alone do not complete validation:

```sh
cargo check --workspace --all-targets --all-features
cargo build --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
python3 scripts/coverage.py # 97% merged source lines and LLVM functions
```

The coverage stage preserves raw LLVM metrics and strictly enforces every merged LCOV source line; see `docs/site/src/reference/coverage.md` for the instrumented-instance reporting limitation. It requires `cargo-llvm-cov` and the toolchain's `llvm-tools-preview` component. Document CLI usage and native target toolchain setup in `README.md`. Test supported feature combinations separately when all-features testing cannot represent them.

## Coding Style & Naming Conventions

Use rustfmt's default four-space indentation, `snake_case` for modules/functions, `PascalCase` for types/traits, and `SCREAMING_SNAKE_CASE` for constants. Prefer typed enums and structs over unstructured JSON inside the core. Use explicit errors rather than panics for invalid input; justify any unsafe code.

## Architecture Boundaries & Compatibility

- Keep domain rules pure and filesystem/process effects in adapters. Preserve separate compiler modules for declarations, relations, resolution, expression rules, and clause handlers. Keep dispatch functions short and CLI arguments, commands, rendering, and benchmarks separate.
- Use `ChangeFacet`, `StatePhase`, and structured `RenameError` variants. Name example invocation/outcome components internally; preserve their established serialized IR order.
- Keep `plan_rename(&sources, symbol, new_name)` free of I/O. Put locking, journal execution, source verification, and recovery in the engine. Coordinate multi-file writes through the project lock.
- History ports and rename lineage rules belong to `dctx0-changes`; SQLite implements `HistoryReader`/`HistoryWriter`. Publication must read the previous head inside the same write transaction. Use `SourceReader`/`SourceWriter` at source I/O boundaries; avoid interfaces for every domain helper. Rust callers import the relevant traits and choose `AccessMode::ReadOnly` or `AccessMode::ReadWrite`.
- Reuse snapshot-local symbol, owner-node, and graph indexes without changing deterministic ordering. Build graph traversal indexes only when a diff contains changes. Keep the standalone parser benchmark independent of history/command dispatch.
- Named fingerprint projections must preserve canonical JSON key order. Both full-example snapshot hashes in `fixtures/semantic/snapshot-hashes.json` are compatibility gates covering IR, spans, graph, and fingerprints. Review fixture, CLI/IR JSON, or SQLite schema changes as explicit compatibility changes; never refresh goldens merely to silence a regression.

## Testing & Coverage Requirements

Target **97% line and function coverage of first-party Rust production code**, enforced in CI with `cargo-llvm-cov`. Also pursue full branch/region coverage where Rust/LLVM tooling supports reliable measurement. Line coverage alone does not prove every behavior is tested.

Cover success, rejection, boundary, timeout, cancellation, and unavailable-toolchain paths. Use unit tests near implementation, integration tests under each crate's `tests/`, and descriptive behavior-based names. Run doctests separately where coverage instrumentation does not support them. Compare parser, IR, interpreter, and target behavior against shared reference fixtures; document intentional deviations. Exercise real native compilers and harnesses in integration CI, not only mocks.

Merge coverage across required platform/feature jobs where necessary. Any technically unreachable or unmeasurable code needs a narrowly scoped, documented exclusion explaining the limitation and alternative validation. Record exclusions and tooling limitations in `docs/site/src/reference/coverage.md` when introduced. Never exclude ordinary error paths, lower the user-approved 97% thresholds, or add meaningless tests just to obtain 100%. Report coverage of non-Rust target helpers separately; Rust instrumentation does not measure them.

Coverage tooling reference: https://github.com/taiki-e/cargo-llvm-cov

## Mandatory Performance Measurements

Every test/validation run must also include performance measurements of **speed and RAM consumption**. Provide a project-owned script or CLI that runs the tests and the performance suite together; wire it into CI and document its exact commands in `README.md` once implemented. A test result without the associated measurements is incomplete.

Benchmark representative operations separately, including parsing, compilation, generation, and available verification/behavior execution for both mandatory example workspaces. Record elapsed time in milliseconds and peak resident memory (RSS) in bytes. Define how subprocess memory is measured and report native compiler/harness costs separately from Rust core costs; unsupported memory measurement must be explicit, never reported as zero.

Use release builds without coverage instrumentation for benchmarks. Keep inputs, seeds, feature sets, warm-up, repetition counts, and cache conditions reproducible. Retain raw samples and summary statistics; record timestamp, source revision or content hash, dirty-worktree status, toolchain, platform, hardware, and workload identity. Compare only compatible measurement environments and workloads.

Persist the **latest 20 measurement runs** in the project under `benchmarks/history/`, using structured JSON. Each run contains its workload results and metadata. Append the new run and prune only the oldest history entries beyond 20; use atomic writes and protect concurrent updates. Failed or incomplete measurements remain explicitly marked and cannot become a baseline.

Keep the accepted benchmark baseline separately at `benchmarks/baseline.json`, outside history pruning. Provide an explicit command to promote the latest successful, complete measurement (or a selected recorded run) to the new baseline. Save its full results and provenance so it survives history rotation. Never update the baseline automatically during tests or in response to a regression; baseline changes must be intentional and reviewable.

Report absolute and percentage changes in runtime and peak RAM against the accepted baseline for each workload, plus recent historical trends. Configure and document independent regression tolerances for speed and RAM, accounting for measurement noise; fail the performance gate when a comparable result exceeds either tolerance. Flag a missing or incompatible baseline explicitly instead of claiming no regression. Include performance results and intentional baseline changes in PR validation evidence.

The runner currently measures 84 workloads on Linux: ten planning/generation/unbound verification and harness-artifact phases, 40 profile phases plus the 34 established workloads: parsing, compilation, graph, snapshot, unchanged/changed diff, persistence, unchanged/changed check, and eight task phases (plan, reconcile, publish, list, Markdown/JSON context, slice and claim) for each example. RSS is post-exec process `VmHWM`, including phase preparation; it is not incremental allocation. Default regression tolerances are speed 20% and RAM 10%. Promote intentionally with `python3 scripts/validate.py --accept-baseline latest` or a recorded filename. On 2026-10-09 the user explicitly accepted the complete 34-workload run `20261009T181143.296099Z-266218.json` as the baseline after reviewing the timing deviation. All 34 established workloads now have accepted values; the original measurement retains its failed comparison to the previous baseline. Future promotions still require explicit authorization.

## Identity, History & Renames

Keep specs as reviewed source files and derived history in `.dctx0ctl/state.sqlite3`. A successful `check` publishes atomically; failed compilation records diagnostics without replacing the valid head. Source hashes preserve raw changes separately from semantic fingerprints. Semantic dependency cycles are valid; the implemented task DAG is a separate structure.

`stableId` retains its existing data-declaration meaning and must not become universal symbol identity. Qualified names identify declarations within a snapshot. Only explicit `rename <root> <qualified-symbol> <new-name>` records continuity; ordinary remove/add edits remain remove/add, without similarity heuristics. `--dry-run` performs no writes. Resolve references before editing, preserve narratives and unrelated identifiers, recompile, and journal multi-file edits for crash recovery. Follow recorded rename chains across snapshots; deletion ends continuity.

## Commit, Pull Request & Security Guidelines

No Git history currently establishes a convention. Use concise imperative commit subjects and focused changes. PRs explain behavior changes, link relevant issues, and include checks, coverage results/exclusions, compatibility implications, and screenshots for UI changes. Never commit secrets; document configuration with safe examples.

## Task Implementation Invariants

Normal work is one contract or operation; recursive contract components and explicit integration are justified exceptions. Include the complete needed type closure and refinements in context, and retain typed models in JSON. Markdown may omit internal metadata, never binding requirements. Before/after deltas are explicit; cursor pages pin plan, snapshot and event sequence. Token counts without a named tokenizer are unmeasured.

Task definitions are immutable. Progress and external check/review reports use append-only events and state caches. Plan publication checks its expected head, check occurrence and event sequence within the write transaction. Read-only access never migrates or recovers files. Preserve version-1 snapshot readability and additively migrate writers to version 2. Record new renames against the exact check occurrence; snapshot hashes can repeat.

Start/complete/release enforce actor, claim and version guards. Slice reservation is atomic and does not start later tasks. Changed plans release invalidated unstarted reservations; ongoing superseded work must quiesce before conflicting replacement work starts. Started removed requirements create retirement work after adapting former consumers. Integration/review reuse checks prerequisite task identities as well as requirements, including deletion/reintroduction with identical contracts. Explicit FAIL/review follow-up creates new IDs and integration/review revisions; repeated planning must retain that work. Review approval and manual completion never set testing PASS. Pure interpreter evaluation is introduced separately; native execution remains deferred.

## Editable Target Profiles (2026-10-09)

Core owns `TargetProfileProvider`, manifests, roles, documents and execution
bundles. Four statically registered target crates supply defaults; they perform
no filesystem access or task decisions. Engine treats application roots
separately from spec roots and publishes editable `.dctx0/profiles` files without
overwriting existing Markdown. Tasks select profiles purely from pinned semantics.
Store freezes used bundles in claim/reservation transactions and preserves
event history. Schema-1/2 readers remain read-only; writers migrate to schema 3.

Profile edits never alter work/check status or create tasks automatically.
Explicit previews and selections create new Open/Unverified adaptation IDs for
completed work, retain old results and revise affected integration/review tasks.
Other implementation tasks are not automatically recreated. Typed adaptations
survive replanning; legacy follow-up reasons retain read fallback. PASS never
asserts automated verification of free-form profile notes. See `docs/site/src/reference/profiles.md`.

Release measurement provenance includes target crates and embedded defaults.
The runner includes 40 profile workloads in addition to the accepted 34; new
workloads are unassessed until an explicitly authorized baseline promotion.

Cinema-only LLM consumer acceptance is reproducible with
`PATH=/home/held/.cargo/bin:$PATH python3 scripts/cinema_acceptance.py`.
It passes 27 workflow assertions and 18 native TypeScript/Bun tests for the
ReleaseYear contract, including meaningful mutation checks, and records 37
Cinema-only release workloads. It uses isolated profile/task history and leaves
the reference spec untouched; it does not execute runatron-next. See
`docs/site/src/reference/cinema-acceptance.md` and its retained evidence for scope and memory limits.

A bounded three-task Cinema follow-up is available through
`PATH=/home/held/.cargo/bin:$PATH python3 scripts/cinema_chain_acceptance.py`.
It passes 15 workflow assertions and 12 focused native tests for FilmId,
the VotingRules contract and candidateOrderWinner, with strict TypeScript
checking and three representative release phases. All other tasks stay Open.
`docs/site/src/reference/cinema-chain.md` records the result handoff, bounded invariant checks and
the unmodeled empty-vote decision; this does not build the complete application.

## Adaptive verification decision (2026-10-10)

See `docs/site/src/reference/verification-architecture.md` for the accepted external-harness and
adaptive-adapter direction, stage boundaries and evidence invariants. Production
code remains independent of dctx0. Default generation creates testing artifacts
only; production generation is optional future work. Interpreter work resumes
for the staged language-neutral verification core. TypeScript/Bun is the first
planned executable target; other targets retain profiles without execution claims.
Automatic adoption requires complete native provenance and transactional guards;
partial checks, imported legacy PASS, reserved/running work, review and retirement
tasks cannot authorize it. Native harness, schema-4 history, CLI orchestration,
mocks and lifecycle execution remain planned until separately implemented and
accepted. Existing compiler snapshot goldens and task definitions stay unchanged.

Verification foundation validation (2026-10-10): 142 all-feature Rust tests and
8 Python tests pass, with 100% merged source-line coverage (11,329 lines) and
100% LLVM function coverage (955 functions). This is not branch completeness or
native verification. Four new release workloads measure unbound evaluation and
assessment for both complete example IRs; they do not execute applications and
remain unassessed against the accepted baseline. See
`docs/site/src/reference/verification-foundation.md` for implemented scope and remaining stages, and
`docs/site/src/reference/verification-validation.md` for retained validation/performance evidence.

Cinema goal continuation (2026-10-10): runtime contract checking now covers aliases,
records, variants, lists/options, UUID, RFC3339 Timestamp, finite Float wire values,
modeled ranges and executable where clauses. Pure returns must satisfy their IR
result contract. The checkpoint passes 148 Rust/8 Python tests, 11,590 merged source
lines and 977 LLVM functions at 100%. Development speed/RSS evidence uses reduced
samples and is explicitly unassessed against the accepted baseline. The complete
Cinema objective remains active; see `docs/site/src/reference/cinema-verification-progress.md` and
`docs/cinema-contract-validation.json`. No harness/adoption completion is claimed.

Cinema generation checkpoint (2026-10-10): deterministic target-neutral corpora
and parameter tuples are implemented. All 45 Cinema data declarations generate
contract-valid values; finite spaces and bounded selection remain distinct.
Validation passes 155 Rust/8 Python tests and 100% of 12,065 merged source lines
and 1,016 LLVM functions. Two generation workloads bring the runner to 80. Reduced
development measurements are incompatible/unassessed against the accepted
baseline. See `docs/site/src/reference/verification-generation.md`; full native Cinema verification,
plan projection, CLI/history/adoption, mocks and lifecycle execution remain open.

Cinema projection continuation (2026-10-10): target-neutral plans now retain all
Cinema contracts, operations, examples, assertions, lifecycle transitions and
manual/suite/policy clauses. Example action order and connected lifecycle paths
are explicit; missing reviewed inputs, unreachable work and unsupported expressions
remain open. `verify plan` supports explicit subjects/tasks and historical check
occurrences, reading published snapshots without task changes or implicit checks.
Current spec source drift and mismatched task/check occurrences are rejected.
The runner adds two projection phases (82 workloads); accepted baseline remains
unchanged. See `docs/site/src/reference/verification-planning.md`. Native harness execution, run
history, adoption, mocks and actual lifecycle execution remain outstanding.

Cinema projection validation (2026-10-10): 173 Rust/8 Python tests pass; merged
source lines 13,143/13,143 and LLVM functions 1,099/1,099. All 82 release speed/RSS
workloads completed in `20261010T004130.382281Z-529285.json`; development settings
remain unassessed and the baseline is unchanged. `docs/cinema-projection-validation.json`
preserves complete samples/provenance and earlier incomplete runs. All 38 concrete
Cinema symbolic inputs for 12 examples are supplied and type-checked; no native
execution or automatic-adoption claim follows.


Bun transport checkpoint (2026-10-10): the separate target SDK and TypeScript
harness generator produce testing-only artifacts and preserve profile manifests.
Native conformance exercises real counter calls, sessions, state/events, external
mock replies and one existing Cinema tie case; Rust decodes the actual packets.
177 Rust/8 Python/4 Bun tests pass, with 13,244 merged source lines and 1,108 LLVM
functions at 100%. The coupled runner records 84 Rust release workloads and
separate native transport costs; development settings remain unassessed. Embedded
TS helpers participate in release provenance. CI installs Bun 1.4.2 and retains
native reports. `--focus native-transport` is partial development validation only;
coverage acceptance still requires workspace focus. No full Cinema verification
or adoption is claimed. See `docs/site/src/reference/bun-harness.md` for missing managed limits,
exception classification, central scenario evaluation and helper coverage.


Managed native executor checkpoint (2026-10-10): engine-owned Linux process groups,
bounded protocol/log streams, cancellation, timeouts and explicit cleanup/reaping
are implemented. Pure expression cases drive actual adapter calls and central
Rust result-contract/assertion assessment. Both Cinema winner assertions run
through real Bun in 101 fresh cases with 201 invocations; an isolated incorrect
implementation is detected. This does not execute complete Cinema examples, mocks
or lifecycles and supplies no authenticated adoption authority. The coupled
workspace check passes 192 Rust/8 Python/4 Bun tests, 13,979 merged lines and 1,183
LLVM functions at 100%, with 84 release speed/RSS workloads and separate native
process samples. No coverage exclusion, threshold or baseline change was made.
See `docs/site/src/reference/native-executor.md` and retained `docs/native-executor-validation.json`.
Complete native scenario orchestration, application implementation, verification
CLI/history/provenance/adoption and helper line coverage remain required.


Cinema scenario checkpoint (2026-10-10): all 12 unchanged Cinema examples execute
against real application-owned Catalog/Community/ScreeningPlanning services in
`examples/cinema/src/application/movie-night.ts`. The external adapter injects
ports; Rust prepares/evaluates mocks, delivered outcomes and actual events. The
examples record 12 surface invocations, 16 collaborators, seven events and 19
expectations. Isolated event-payload, removed-emission and port-wiring mutations
are detected. Production imports no spec/testing runtime. Whole-case deadlines
span requests and cleanup; adapter failures are distinct from explicitly captured
application exceptions. Missing/duplicate actions and unused prepared collaborators
cannot produce PASS. Given does not imply an invented exactly-one call requirement.
Modeled numeric-to-Float representation is allowed only when exact; native values
are never repaired by it. The regular Cinema TypeScript check passes with separate
compiler speed/RSS evidence. The coupled workspace check passes 202 Rust/8 Python/
4 Bun tests and 14,526 merged lines/1,232 LLVM functions at 100%, with 84 release
workloads. Baseline, coverage thresholds and exclusions remain unchanged.
See `docs/site/src/reference/cinema-scenarios.md` and `docs/cinema-scenarios-validation.json`.
Actual production ports, Hono/Awilix/HTTP wiring, operation requires/ensures/call
counts, state/lifecycles, whole-plan execution, verification CLI/history/provenance/
adoption and native helper line coverage remain required. Transport errors need
partial scenario retention in the future whole-run report.

Cinema real-state continuation (2026-10-10): production now includes all thirteen
SQLite ports, explicit Hono routes and a scoped Awilix composition. The external
adapter supports explicitly configured real sessions, actual port call recording
and SQLite entity observations. Rust evaluates keyed state/trace predicates and
connected lifecycle evidence; both Cinema paths execute with fresh databases and
state/wiring mutations are detected. Transition guards use before-state even when
the transition consumes a prerequisite. Native compiler/test commands have bounded
ordinary log streams distinct from the verification protocol. Independent native
application tests pass 36 cases; Bun reports 210/210 application lines and 131/131
functions, separately from Rust and generic worker/adapter coverage. The complete coupled checkpoint passes 217 Rust/8 Python/4 Bun transport tests
and 36 native Cinema tests, with 15,332 merged Rust lines and 1,298 LLVM functions
at 100%. All 84 paired release workloads complete in
`20261010T042938.796798Z-712215.json`; development settings remain unassessed and
the accepted baseline is unchanged. Intermediate failures are retained. See `docs/site/src/reference/cinema-state-lifecycles.md`
and the checkpoint evidence in `docs/cinema-real-validation.json`. Whole-plan
operation obligations, CLI run/history/replay, safe adoption and partial transport
evidence remain required. Do not silently resolve the entity-key scope ambiguity
for Rating/FilmVote or claim that native application coverage certifies the worker.


Verification setup checkpoint (2026-10-10): `verify init` creates testing-only
configuration, contract, worker and unavailable adapter scaffold; existing files
and configuration are retained. A saved spec/application association permits
configured `verify plan` scope, budgets and fixtures. Empty scope never selects
the whole application; explicit selectors override configuration. Neither command
executes application work or adopts tasks. Real Bun confirms scaffold startup
fails explicitly. Rejected paths, symlinks, configuration and failed/resumed
publication are covered. 224 Rust/8 Python/4 Bun transport/36 native Cinema tests
pass, with 15,619 merged Rust lines and 1,313 LLVM functions at 100%. The runner
now records 86 release workloads, including two fresh setup phases; development
settings remain unassessed and the baseline is unchanged. See
`docs/site/src/reference/verification-setup.md` and retained `docs/verification-setup-validation.json`.
The next priority remains whole-plan native execution and run/show/replay,
immutable schema-4 history, guarded adoption and structural discovery. Do not
replace that missing workflow with more isolated library acceptance claims.


Verification history checkpoint (2026-10-10): writers migrate additively to SQLite
schema 4; read-only readers accept 1–4 without migration. Core owns schema-1 run,
case/state/provenance models; task report schema 2 and snapshot IR remain unchanged.
Immutable run/artifact publication checks exact successful check occurrence, case
coverage/outcomes and content digests, and rolls back failures transactionally.
`verify show` reads saved evidence without working-tree freshness claims. Recording
a report never changes task progress/testing/adoption; self-declared native origin
is not authority. 229 Rust/8 Python/4 Bun transport/36 native Cinema tests pass,
with 15,894 merged lines and 1,343 LLVM functions at 100%. All 90 paired release
workloads complete; development sampling remains unassessed and the baseline is
unchanged. See `docs/site/src/reference/verification-history.md` and retained
`docs/verification-history-validation.json`. Native run publication, complete
operation/contract/lifecycle orchestration, guarded adoption, replay/shrink and
structural discovery remain required; do not mark the full Cinema goal complete.

Native session evidence (2026-10-10): the managed worker retains the latest
case's ordered actual invocation/state/mock prefix after transport failure or
cancellation. Begin/End markers require matching acknowledgments; written mock
replies do not prove consumption. Every execution attempt resets previous-case
evidence. This is not whole-run publication or adoption authority. The coupled
validation passes 229 Rust tests and retains 100% merged lines (15,931) and LLVM
functions (1,344), with 90 complete release workloads. Reduced sampling remains
unassessed and the baseline is unchanged. See
`docs/verification-session-validation.json`. Continue with usable managed native
run publication and guarded adoption; the full Cinema objective is still active.

Managed run development (2026-10-10): `verify run` executes a freshly generated
Bun harness from an owned bounded application/dependency capture and stores
immutable schema-1 evidence. It never publishes a compiler head. The complete
Cinema projection now executes FilmId/ReleaseYear factories, 100 generated pure
operation calls, 101 assertions, all 12 examples and both real lifecycle paths.
Overall result remains UNKNOWN for unsupported contract/operation/manual
obligations. No task adoption occurs: default reports ADOPTION_UNAVAILABLE;
`--no-adopt` suppresses adoption attempts. Task/profile provenance remains
explicitly incomplete. Both full spec compiler gates remain unchanged.
The first full workspace validation passed functional tests but failed 100%
coverage (99.41% merged lines, 99.28% functions). Additional resource/protocol
tests are in development; do not claim this preserves current 100% coverage.
See `docs/site/src/reference/verification-run.md`. Finish the missing obligations, restore full
coverage, implement guarded adoption/discovery/replay and complete the full
Cinema objective; this is not final acceptance.

Native contract rejection planning (2026-10-10): projected contract plans now
include a separately versioned `dctx0-rejections-1` corpus alongside valid values.
Rust selects canonical values it rejects against the pinned contract, including
UUID text, integer/decimal boundaries, structural records/options/lists and enum
alternatives. Exact boundary offsets reuse interpreter arithmetic. Native
acceptance of a required rejection case is FAIL, rejection PASS, unavailability
UNKNOWN. Legacy schema-1 plans remain readable but missing rejection obligations
are UNKNOWN and need reprojection. These verification JSON additions do not
modify snapshot IR/goldens or immutable task definitions. FilmId/ReleaseYear now
execute 30 actual negative cases successfully; removal of a production range
guard is detected even while valid inputs still pass. Complete operations,
guarded adoption/discovery/replay and the 100% coverage restoration remain required.

Operation-clause development (2026-10-10): core verification now evaluates typed
requires/ensures and declared result/failure contracts against actual observations.
Reviewed operation fixtures are additive optional verification JSON collections;
they pin exact setup/invocation cases without changing IR or task definitions.
Native sessions observe before/after state and gate calls/events on declared
observer capabilities. Cinema executes four success/closed-poll cases for castVote
and closeVoting; missing effect handlers remain UNKNOWN. Reviewed cases do not
replace generated operation obligations. Finish effect/state planning, full
generation/context closure, guarded adoption/discovery/replay and 100% coverage;
the full Cinema goal is still active.

Replay development (2026-10-10): `verify replay <root> <run-id> --case <case-id>`
loads saved plan/inputs and executes a new captured implementation against pinned
historical requirements. It stores new evidence, compares provenance changes and
never adopts tasks. Original reports remain immutable; current spec/example edits
do not replace old replay inputs. Selected-case outcomes are distinct from broader
run completeness. Generated indices can be reindexed; original/new IDs are both
reported. Factory counterexample replay and changed-production detection pass
focused tests. All-kind replay conformance, shrinking, replay workloads, complete
effects/context generation, guarded adoption/discovery and 100% coverage still
need completion. Do not mark the full Cinema goal achieved.

State-effect development (2026-10-10): Rust assesses actual before/after entity
observations for lists, keyed observations, complete-source inserts/upserts and
keyed removals. It preserves unrelated rows and checks direct entity, Option and
List results against modeled state. Removal results refer to the removed before
state. Duplicate declared identities and absent observers remain UNKNOWN;
replacement sources and distinct nominal result projections require explicit
modeled support. Failure outcomes do not invent successful state effects.
Cinema now executes six reviewed real operation cases, including FilmCatalogStore
insert/update; these do not replace generated obligations or authorize adoption.
Focused verification tests and 90 release workloads pass on a stable source tree
in `benchmarks/history/20261010T081157.183186Z-871771.json`. Reduced sampling is
unassessed against the baseline; this is not full workspace/coverage acceptance.
Full effects/context generation, discovery, guarded adoption, shrinking and 100%
coverage remain required. The full Cinema objective remains active.

Call/event development (2026-10-10): Rust assesses modeled emission payloads
against actual event observations and validates observed collaborator signatures.
Call declarations grant permission; exact counts require explicit trace clauses.
Result aliases bind only one valid actual collaborator outcome; ambiguous/missing
observations stay UNKNOWN. No undeclared-effect absence or resource-access proof
is implied. Focused verification tests and 90 paired release workloads pass on a
stable tree in `benchmarks/history/20261010T082313.972831Z-882559.json`; reduced
sampling remains unassessed. Full boundary/resource evidence, generated nonpure
cases, guarded adoption, discovery, shrinking and 100% coverage remain pending.
The complete Cinema objective is still active.

Structural discovery development (2026-10-10): `verify discover <spec-root>
--project <app-root>` performs read-only bounded TypeScript/TSX inventory through
the new SDK `ImplementationInspector` contract. The statically registered target
uses pinned Tree-sitter 0.25.10 and TypeScript/TSX grammars 0.23.2. Imports,
exports, declarations, public class members, syntax errors and exact-name
candidates are retained; no candidate is automatically accepted, no adapter or
application is executed, and no files/tasks/history are written. Syntax-error
sources cannot suggest matches. This is not semantic type analysis, DI/HTTP
resolution or runtime effect proof. Release validation adds two actual-application
discovery workloads (92 total), with application-source hashes and process RSS;
new workloads have no accepted baseline. The first development run is incomplete
because sources changed during measurement. A fresh full workspace/coverage run
is pending; do not claim current 100% coverage or full Cinema acceptance. See
`docs/site/src/reference/verification-run.md`. Guarded adoption, complete operations/effect boundaries,
resource observations, shrinking and final acceptance remain required.

Discovery validation (2026-10-10): full workspace/feature functional tests pass,
but coverage fails at 18,014/18,324 merged lines (98.31%) and 1,462/1,494 LLVM
functions (97.86%). All 92 paired release workloads complete on stable source
hashes; reduced sampling is unassessed and the baseline is unchanged. Failed
coverage and distinct raw LLVM metrics are retained in
`docs/verification-discovery-validation.json`. Restore the gate; do not exclude
ordinary errors or claim current 100% coverage.

Adaptive fixture validation (2026-10-10): frozen independent direct-function and
Hono/Awilix service applications use the same spec and exact pinned plan through
external adapters. Direct service and real HTTP paths pass; mutations of actual
business logic and DI wiring, plus a fake constant adapter, fail central Rust
postconditions. Native pure execution now assesses modeled pre/postconditions;
unmet/unobservable prerequisites remain UNKNOWN. Production remains free of
testing/spec/runtime imports. Strict native TypeScript checks and focused Rust
verification tests pass with 92 complete paired release workloads on stable
provenance; reduced sampling remains unassessed. Native worker VmHWM and debug
run timing are retained; compiler/Rust-process RSS and separate release execution
phases remain incomplete. See `docs/verification-adaptive-validation.json` and
`fixtures/conformance/adaptive-typescript/production-frozen.json`. Full Cinema
operations/types, guarded adoption, resource/effect boundaries, shrinking,
all-kind replay/helper coverage and 100% Rust coverage remain required.

## Verification task pins and delivery groundwork (2026-10-10)

Task-selected native runs capture requirement, plan/event and available profile
bundle pins before execution, plus captured build configuration hashes. Missing
profiles remain explicit adoption provenance gaps. The pure task adoption query
never mutates history or authenticates supplied evidence. An explicit store result
batch reuses start/complete/check rules in one guarded transaction; it is not
native execution authority. Automatic adoption remains disabled pending native
provenance/freshness validation, run/task links and idempotent receipts. See
`docs/site/src/reference/verification-run.md`. The task-pin checkpoint passed focused validation
and 92 paired release workloads; full coverage and full Cinema acceptance remain
incomplete. Do not infer automatic adoption from an imported PASS or Native flag.

## Native verification checkpoint (2026-10-10, historical)

This checkpoint supersedes earlier notes describing automatic adoption as disabled.
Complete native contract evidence now adopts eligible Open tasks atomically,
with runtime executable/environment identity, pinned requirements/profiles,
post-execution source checks and idempotent receipts. Changed code retains old
Done/PASS history. Unknown dependencies, incomplete requirements, claims and
reservations prevent adoption. This is demonstrated on the bounded native
contract fixture; whole-Cinema adoption is not yet proven.

Generated pure and nonpure Cinema operation cases, reviewed real ports, mocks,
examples and both lifecycle paths run through central Rust assessment. Lifecycle
shrinking and minimized replay now have a real production-mutation acceptance
test; the focused paired validation is recorded in
`benchmarks/history/20261010T140835.466152Z-1295153.json`.
Full Cinema contract observers, transitive dependency closure, native helper
coverage and the required 100% Rust coverage remain incomplete. The last full
coverage run measured 97.13% merged source lines and 97.28% LLVM functions.
Do not infer complete acceptance from these focused checkpoints.

## Complete Cinema native checkpoint (2026-10-11)

The full current Cinema plan executes 5,655 mandatory cases, all PASS, including
46 real production contract validators, all 12 mocked-consumer examples,
101 assertion cases, generated/reviewed actual operations and both real SQLite
lifecycle paths. Captured dependency provenance is complete for this execution.
Bounded native ESM observation discharges only loaded, unexecuted dynamic sites;
executed unresolved loaders remain UNKNOWN. Production imports no testing runtime.

A whole-plan integration run with isolated specification/application history
automatically delivers 41 of 104 tasks Done/PASS, preserves all other tasks
Open/Unverified, and verifies that actual replay leaves task events unchanged.
Task-specific adoption may coexist with unrelated manual/suite UNKNOWN only when
every mandatory native case passes, provenance is complete and each delivered
task has complete requirement-to-case mapping. Store repeats these guards in its
write transaction and independently re-evaluates discharged state preconditions
from the immutable plan, pinned snapshot and actual before-state observations.
Root subject selection pins task subjects below the exact qualified root; it
does not adopt unrelated dependency work. Imported flags remain insufficient.

The whole report remains UNKNOWN for 18 manual requirements and nine named
suites. Do not invent executable assertions from these names or prose, mark
reviews/integrations complete or claim a Release-policy PASS. The final aggregate
and mandatory Rust coverage gate remain outstanding; release performance remains
deferred and the accepted baseline is unchanged. See
`docs/cinema-field-mapping-adoption-validation.json`, `docs/site/src/reference/cinema-verification-progress.md`
and `docs/site/src/reference/verification-run.md` for scope and evidence.

### Validation scheduling decision (2026-10-10)

The user explicitly requested moving performance tests to the end for separate
review. During this implementation, use `python3 scripts/validate.py
--focus verification --skip-performance` for functional checks; its separate
report marks performance deferred and does not update benchmark history or the
accepted baseline. Coverage remains required. Functional conformance tests for
benchmark commands remain ordinary tests; release performance measurements and
regression assessment are deferred to final acceptance. See
`docs/site/src/reference/verification-run.md` for the commands and evidence limits.

## Memory and validation decision (2026-10-11)

The user approved a 97% line/function coverage gate and requested a memory cap of
two thirds of available RAM. Validation/coverage entrypoints now run inside a
Linux systemd user scope with MemoryMax based on startup MemAvailable and no
additional swap. The cap includes descendants collectively. Use
`python3 scripts/memory_guard.py <command> ...` for direct native verification.
Direct invocation without this wrapper has no hard RAM cap. Cargo build jobs and
Rust test threads default to two in guarded runs. Functional tests and coverage
run before benchmark conformance and release performance; `--skip-performance`
defers those final stages without claiming performance acceptance.

## Cinema entity-key continuation (2026-10-11)

Assessment 12 supplies five mandatory native entity-identity cases with actual
recorded operation before/after states, independent Store reconstruction and
pinned-input replay. The complete Cinema corpus has 5,660 PASS cases and 46 of
104 adopted tasks; 58 remain open. The 18 manual and nine suite obligations still
keep the whole result UNKNOWN. No production/spec architecture rewrite, compiler
golden change or task-definition rewrite accompanies this evidence increment.
Actual SQLite replacement mutation exposes duplicated persisted identities and
replay retains the failure without overwriting history. Missing/empty or incomplete
identity observations remain UNKNOWN. See docs/cinema-entity-identity-validation.json.
The requested 97% coverage gate and guarded memory policy remain applicable;
performance measurement runs after functional checks and coverage.

## Complete Cinema acceptance continuation (2026-10-11)

The user now requests complete Cinema validation and zero open tasks, including
explicit agent inspection PASS where deterministic verification is unavailable.
Prefer deterministic evidence; preserve automatic results separately from agent
judgements and never conceal an executed failure. The user also requests bundling
long validation work at the end: use focused checks while implementing, then run
complete Cinema/workspace/coverage acceptance together and performance last.

Assessment 13 implements pure reference proof-category aggregation, exact native
Examples/Properties suite membership and the existing Release rules. Native
suite/policy artifacts are retained and discharged obligations are independently
reconstructed in Store. Missing or stale review results never default to zero
findings or satisfy negative requirements. `verify attest` explicitly publishes
bound task inspections through guarded start/complete/check transactions, with
structured review events for review tasks. Original native runs remain immutable;
agent evidence is visibly separate from automatic adoption. Current production
Cinema completion and fresh whole-workspace acceptance are still pending. See
`docs/site/src/reference/verification-suite-attestation-progress.md`.

Stored inspection continuation (2026-10-11): `verify inspect` retains an effective
assessment as imported inspection evidence referencing unchanged native cases;
this cannot authorize automatic adoption or claim new execution. Bound original
inventory/spec/plan/profile checks and idempotent storage are implemented and
focused real-native tests pass. The validation runner includes native Cinema
TypeScript checking, HTTP and durable SQLite conformance before final coverage
and performance. A source review found and repaired timestamp calendar validation;
production shares an ordinary RFC3339 validator without spec/runtime dependencies.
Whole Cinema inspections, actual task-plan completion and fresh aggregate
acceptance remain pending.


## Cinema final acceptance (2026-10-11)

Cinema has 104 active tasks Done/PASS and zero unfinished tasks. All 5,674
mandatory native cases pass; explicit bound agent inspections resolve the manual
requirements in a separate effective PASS report. Functional validation and the
authorized coverage gate pass (97.8657% source lines, 97.0005% LLVM functions).
The user deferred runatron-next; the original aggregate was stopped only during
its release measurements. Separate Cinema-only measurements complete all 46
workloads with unchanged source hashes and the collective 2/3 available-RAM cap.
Baseline comparison is unassessed and the accepted baseline is unchanged. The
Python measurement selector was checked by all 15 script tests after the
functional run; no Rust or application inputs changed. Earlier pending status
paragraphs are historical checkpoints. See `docs/cinema-complete-validation.json`
and `benchmarks/history/20261011T125429.502923Z-175814.json`.


## Rust complete-verification CLI (2026-10-11)

`dctx0 verify complete <spec-root> --project <app-root>` implements the complete
TypeScript/Bun acceptance workflow in Rust, including authored source/spec hash
validation, explicit check/planning, native execution, effective inspections and
guarded inspected task delivery. Default notes live in the adapter directory;
`--inspections` selects another authored file. The Linux CLI uses one systemd
user scope capped at two thirds of available RAM, without a Python runtime.
Bun remains the native TypeScript runtime. A fresh unchanged check now publishes
a new immutable plan occurrence while preserving task IDs, definitions, states
and history; repeated planning for the same occurrence stays idempotent.

Actual release Cinema acceptance passes 5,674 mandatory cases with 104 tasks
Done/PASS and zero unfinished tasks. The aggregate validation passes at the
user-authorized 97% gate (97.7200% merged lines, 97.0071% LLVM functions), with
46 Cinema-only release workloads. Baseline comparison is unassessed and the
accepted baseline is unchanged. See `docs/site/src/reference/verification-complete.md` and
`docs/verification-complete-validation.json`.

## Progressive tutorial checkpoint (2026-10-11)

The user-requested TypeScript MiniBoard tutorial is in progress under
`examples/tutorial-step-{1..6}-*`, with the complete 16-step feature inventory
in `examples/tutorial/features.json`. These first six cumulative checkpoints
compile and pass their selected native plans with no open task results, and
independent production tests measure 100% source lines/functions. Step 5 has
2,793 mandatory cases and step 6 has 3,126. This is not acceptance of the full
series: later steps, final feature coverage, mutation acceptance, aggregate
validation and commit/push remain outstanding. Production uses its own types;
external adapters translate representations and invoke actual application code.

Interpreter version 5 now executes exact `round`/`divide` with bounded scales
and all four rounding modes; exact finite quotients preserve reference behavior.
Text refinements and Duration/Bytes/compatibility-option/result builtins have
central semantics and generator version 3. See `docs/site/src/reference/tutorial-language-semantics.md`.
The fresh-exercise Bash script retains reference solutions and does not copy
historical approvals. Four independent script tests verify this behavior.

Tutorial HTTP/UI checkpoint (2026-10-11): step 16 implements an explicit
administrative HTTP surface alongside unchanged Catalog summary boundaries.
A local MiniBoard page now creates actual tasks and marks them Done/Open through
real POST/PATCH requests. The Rust complete command executes 3,912 mandatory
cases; source-bound authored inspections yield effective PASS with 157 active
tasks Done/PASS. The independent TypeScript suite passes 37 tests/383 assertions
with 100% measured src line/function coverage. Embedded browser JavaScript and
the server.ts startup are outside that coverage percentage; 15 real Chromium
checks separately exercise rendering, mutations, errors, races and mobile fit.
All 16 steps have individual checkpoints, not final current-source series
acceptance. Feature inventory, mutations/replay, aggregate validation, final
performance and authorized commit/push remain pending. See examples/tutorial/
validation/step-16.json and the step 16 README for evidence and local startup.

Tutorial reset and architecture variants (2026-10-11):
`bun examples/tutorial/reset-agent.ts all` creates/resets marked exercises under
`target/tutorial-exercises/step-N`; individual steps and explicit exercise paths
are supported. Default production starts at the previous step; `--empty` removes
all production solutions. Reset removes both local history/database locations and
old inspections, restores current specs/tests/adapters, and preserves custom
AGENTS.md and profiles. Reference examples are never reset. Ten helper tests cover
all sixteen exercises, empty mode, preserved guidance and rejected unsafe paths.
The English tutorial guide explains agent instructions, editable implement/use
profiles and their separation from behavioral PASS.

`examples/tutorial-variants/` adds direct Bun and Hono/Awilix class-based HTTP
implementations of the same finite step-1 specification. Their shared runner
checks identical semantic plans (independent check occurrences stay distinct),
strict types, 100% production line/function coverage, 28 native cases per variant,
real-production mutation rejection through HTTP and final speed/RSS measurements.
Classes-only production uses methods and bound callbacks, with no standalone
functions or arrow functions; request scopes and the root container are disposed.
See `examples/tutorial-variants/validation.json`. These are architecture examples,
not a second complete sixteen-step series or a whole-workspace acceptance claim.

Complete progressive tutorial acceptance (2026-10-11): all sixteen current
TypeScript examples pass 43,490 mandatory native cases and 100% measured production
line/function coverage per file. All 158 audited language features occur in actual
AST/type/execution nodes. Twenty-one negative checks include genuine production
failures, source/review protection and lifecycle/HTTP replay; fifteen Chromium
checks separately cover all seven UI functions and 58 executable line starts.
The actual step-16 server startup is included in production coverage. Ten exercise/
reset helper tests and both additional Bun/Hono-Awilix variants pass. Reference
solutions and English feature/spec/test guides are committed.

The final coupled workspace validation passes every recorded command and 451
instrumented Rust test executions: merged lines 26,149/26,784 (97.63%), LLVM
functions 2,091/2,150 (97.26%). This uses the explicit user-approved 97% Rust gate;
100% tutorial production coverage is separate and no full branch proof is claimed.
All validation/release input hashes match. Fifty-eight release workloads and
separate native variant speed/RSS records completed last. Baseline comparison is
unassessed/incompatible; the accepted baseline and compiler goldens are unchanged.
Functional test scratch uses an owned on-disk cache with cleanup, avoiding large
instrumented SQLite writes in capacity-limited RAM-backed /tmp; release benchmark
scratch behavior stays unchanged and the 2/3 available-memory/no-swap guard remains.
See docs/site/src/reference/tutorial-acceptance.md and examples/tutorial/validation/complete.json.

## Go target continuation (2026-10-12)

Go now participates in native CLI selection and managed build execution. The
Go worker is compiled with CGO disabled; production remains independent of the
SDK. Tree-sitter provides structural inventory. A module-aware build receipt
binds selected packages, vendored dependencies, embeds, compiler tools and the
actual compiled executable. Go-Cinema uses net/http, ordinary interfaces and
pinned modernc SQLite. Native finite evidence and authored inspection stay
separate. Full workspace coverage and release measurements are completed at
the end of the implementation, with the user-approved 97% Rust line/function
threshold. No new baseline promotion is authorized.


Go target acceptance (2026-10-12): the frozen-source grouped workspace validation
passes 19 commands, including Go-only feature tests and native Go Cinema. The
unchanged Cinema plan executes 5,674 mandatory cases; complete verification
delivers 104 tasks (46 native adoption, 58 source-bound inspection). Five actual
production mutations and broken/repaired replay are accepted. Native manual/suite
obligations retain UNKNOWN and require authored review. Rust merged source-line
coverage is 98.13%, LLVM functions 97.39%, meeting the user-approved 97% gate.
The final release suite records 56 Cinema workloads with performance at the end;
new workloads/reduced sampling remain unassessed and the baseline is unchanged.
See docs/site/src/reference/go-target.md and docs/go-target-validation.json for evidence and limits.


Rust target continuation (2026-10-12): Tree-sitter Rust, testing-only native
transport/adapter, managed Cargo compilation and input/executable receipts are
implemented. Independent Rust Cinema production contains contracts, services,
SQLite transactions and real HTTP. The first native run passed all 5,674 mandatory
cases without incomplete provenance. Manual review remains separate, and final
workspace/feature/coverage acceptance is pending. Group checks at the end and
performance last; preserve the approved 97% gate and accepted baseline. See
docs/site/src/reference/rust-target.md and docs/site/src/reference/rust-target-progress.md.


Rust target acceptance (2026-10-12): the final frozen-source grouped workspace run
passes all 22 commands. Rust Cinema passes the unchanged 5,674 mandatory-case plan,
completes 104 tasks (46 native adoption, 58 source-bound inspection), detects five
actual production mutations and reproduces broken/repaired replay. Native manual/
suite UNKNOWN remains distinct from authored approval. Coverage is 97.94% merged
source lines and 97.19% LLVM functions; no full branch/native-helper claim follows.
The final 66 release workloads ran last, source hashes matched and the baseline
was unchanged. Rust finite verification took about 8.8 seconds, worker RSS about
7 MB with existing caches. See docs/site/src/reference/rust-target.md and docs/rust-target-validation.json.

Bundled skill installation (2026-10-12): the Rust CLI embeds dctx0,
dctx0-language and dctx0-implementation with their references. `skills list`,
`skills install` and `install` support agent-specific local/global directories,
preview, unchanged detection and explicit replacement. Ordered locks, bounded
preflight, staged writes and rollback preserve existing customizations on ordinary
errors. Six focused engine/CLI tests, workspace Clippy and formatting pass. A copied
binary installs without repository assets; its repeated six-destination installation
was unchanged (3.526 ms, child peak RSS 16,445,440 bytes). The shipped language
example compiles. These checks supplement the historical target acceptance; they
are not a new full-workspace coverage run. See docs/site/src/reference/skills.md.

Editor integration continuation (2026-10-12): dctx0-lsp provides a read-only
compiler-backed stdio language service via `dctx0 lsp`. The Rust CLI prepares
local VS Code VSIX and Zed development bundles with `editor install`; optional
VS Code activation invokes its own `code` installer. Syntax assets are adapted
to the Rust language, including Cinema refinement values and post-call field
access. Server tests cover unsaved/versioned UTF-16 edits, project isolation,
resolved navigation, semantic tokens, cancellation and shutdown. Packaged VS Code
client/server acceptance and prepared Zed Rust launcher compilation are implemented;
GUI/store publication is not claimed. Grouped final validation is pending; run
performance last and retain the approved 97% coverage gate. See docs/site/src/reference/editors.md.

Editor error-path follow-up (2026-10-12): full functional/application acceptance
passed, but the first coverage run measured 97.33% lines and 95.24% LLVM functions.
The failed gate is retained in benchmarks/history/20261012T085010.100415Z-890509.json;
its completed measurements are not a baseline. Added meaningful protocol/IO,
initialization/cancellation and local-installer failure tests. Zed launcher paths
now use Rust string literals and explicitly reject unrepresentable paths. A focused
editor validation mode keeps full workspace coverage and release measurements last,
without repeating separately accepted application/race suites. Final coverage is
pending; do not relabel the initial failed validation as successful.

Editor acceptance (2026-10-12): the corrected follow-up passes all eight grouped
commands, including full instrumented Cargo workspace coverage. Merged source
lines are 29,720/30,421 (97.70%); LLVM functions 2,396/2,467 (97.12%). Source hashes
match before/after checks and measurements. The 71 release workloads ran last;
new/reduced samples are unassessed and the baseline is unchanged. Cinema LSP peak
RSS was about 23.5 MiB, initial analysis about 61 ms and unsaved changes about
84 ms including debounce. The earlier full run's 22 functional/application checks
passed while its initial coverage gate failed; preserve both records. GUI/store
availability remains outside automated acceptance. See docs/editor-validation.json.

## Agent conformance tasks (2026-10-12)

Planner version 2 assigns manual constraints/decisions, explicit reviews,
conformance suites and policies through optional `review_obligations` in task
candidates. Scoped reviews are reused; missing scopes get a Review task. Fresh
Cinema plans have 105 tasks and four review owners for 25 obligations. Existing
definitions and historical results remain immutable; compiler snapshots do not
change. `verify review-template` provides deliberately incomplete source-bound
reports, `tasks record-review` stores them under normal guards, and `verify
complete` consumes current reports without setting testing PASS on review
completion. `tasks renew-review` creates an idempotent successor bound to changed
sources and preserves historical reviews. Inspection-note imports remain
compatible but cannot overrule current bound task reports. See
`docs/site/src/reference/conformance-reviews.md`; validation evidence is recorded after acceptance.

Agent conformance acceptance (2026-10-12): TypeScript, Go and Rust Cinema each pass
5,674 native cases and the persisted task-review workflow, with 25 obligations,
four Done/Unverified review tasks and 105 total tasks. Full instrumented workspace
coverage passes at 30,350/31,070 merged lines (97.68%) and 2,454/2,529 raw LLVM
functions (97.03%). Stable scoped checks pass; 71 release speed/RSS workloads run
last and remain unassessed, with no baseline promotion. Concurrent unrelated
`install_mdbook.py`/`docs_site.py` edits are explicitly outside the scoped source
stamp, not Rust coverage exclusions. Source-change/earlier coverage failures stay
recorded. See `docs/conformance-review-validation.json` and
`benchmarks/history/20261012T112024.971179Z-1115890.json`.


## Release distribution (2026-10-12)

Matching `v<SemVer>` tags trigger full reusable Linux validation and five native
release jobs: Linux x64/ARM64 (Ubuntu 22.04, glibc 2.35+), macOS Intel/Apple Silicon
(macOS 14 deployment baseline), and Windows x64/MSVC. Shell and PowerShell
installers verify SHA-256, stream only the executable and replace it atomically.
Archives include CLI/LSP/profiles/skills, license and a source/toolchain receipt;
`SHA256SUMS` covers all five archives and both installers.

`python3 scripts/release_acceptance.py --cargo-tests --platform-tests` couples
13 archive/Unix-installer tests, 9 CLI tests and real CLI/LSP/Bun SDK transport/
owned-process cancellation checks with release packaging/startup speed and peak
memory. Windows additionally runs the native PowerShell installer suite. Reports
retain the latest 20 runs under `benchmarks/history/release/`, using portable
locks and explicitly named Linux/macOS/Windows memory methods. Unavailable memory
queries are null; distribution baselines remain unassessed and the core baseline
is unchanged. Optional systemd limits apply only on Linux.

Managed native verification/adoption remains Linux-only. macOS/Windows tests
exercise the unsupported boundary and must never invent native PASS/adoption.
Local x64 Linux acceptance is implemented; actual macOS/Windows execution and
GitHub publication occur only on configured CI runners. PowerShell syntax and
truncated-input rejection can be checked locally without claiming Windows
execution. See `docs/site/src/reference/releases.md` and retained `docs/release-validation.json`.

## Gin and Axum reference integrations (2026-10-12)

Additional Go/Gin 1.12.0 and Rust/Axum 0.8.9 HTTP variants reuse the existing
Cinema services, persistence and unchanged specification. Existing `net/http`
and builtin Rust HTTP paths remain defaults. Selection belongs to the example
application and reviewed project adapter (`CINEMA_HTTP_FRAMEWORK` and optional
`httpFramework`), not a new dctx0 CLI/framework contract. Rust enables Axum with
its optional application feature `axum-http`; the dctx0 Rust core is unchanged.

Optional English framework profile presets live under
`targets/go/framework-profiles/gin/` and `targets/rust/framework-profiles/axum/`.
Users deliberately merge them into local editable profiles. They are repository
presets, not automatically installed embedded target profiles. Preserve existing
Markdown, frozen bundles, task definitions and historical evidence. Common
framework rules appear once; keyword roles distinguish implementation from use.

The contributor runner `python3 scripts/framework_acceptance.py` couples native
checks, all 5,674 Cinema cases per framework, current source-bound task reviews,
eight real routing/status/middleware/business-exception mutations and broken/
repaired replay with release first-build/warm execution and HTTP-check speed/RSS
measurements at the end. `--skip-performance` explicitly leaves the report
incomplete; `--performance-only` requires matching successful functional inputs
and binary. Failed reports are retained; latest-20 framework history has no
accepted baseline and never promotes the core baseline.

Framework variants route generated real Surface operations through HTTP as well
as explicitly configured HTTP cases. Mocked consumer cases retain a distinct
service path. Gin uses release mode and stderr writers to preserve protocol
stdout and the log budget. Axum uses owned bounded sessions, actual Router/State,
blocking work off executor threads, bounded shutdown and a standard testing HTTP
client with chunked-response/status/header checks. Review approval remains
Done/Unverified; it does not convert the 25 native manual/suite UNKNOWN points
into native PASS. Acceptance scratch/build files use the disk-backed project
`target/frameworks/tmp`, avoiding RAM-backed `/tmp` pressure. Optional Linux/
systemd memory guards remain in force. See `docs/site/src/reference/http-frameworks.md`.


## Rust self-verification acceptance (2026-10-13)

`dctx0 verify self .` captures the required source/build inputs, builds its own
receipt-bound Rust CLI and runs the testing-only adapter in isolated projects.
Explicit subjects are partial development runs; development tasks are never
adopted. Per-project execution limits preserve global defaults. Native stdout and
stderr are separate, and nested/detached process ownership is cleaned up.

The complete bounded Linux plan executes 1,115 mandatory cases across eight
product areas with every case PASS and no machine coverage gaps. All 14 original
examples remain among 43 explicit examples. Actual compiler/evidence mutations
fail three/one relevant cases; native fixtures also exercise production mutation,
adoption and broken/repaired replay. Twelve source-bound authored inspections
complete manual requirements separately. Automatic evidence retains UNKNOWN for
manual requirements; inspection evidence is imported and cannot erase native FAIL.

Seed 42, 16 generated cases per obligation and collection size 2 define a bounded
corpus, not exhaustive behavior. No branch-completeness or standalone adapter
coverage claim is made. Preparation, cold build, warm execution and replay retain
separate elapsed time/RSS; self workloads have no accepted baseline and existing
baselines are unchanged. See `docs/site/src/docs/self-verification.md`,
`docs/self-verification-validation.json` and `docs/self-verification-reviews.json`
for final bindings, scope and measurements.

Fresh all-feature workspace coverage passes 31,313/32,087 merged source lines
(97.59%) and 2,538/2,615 raw LLVM functions (97.06%) with unchanged source hashes.
Raw LLVM lines are 32,874/33,918 (96.92%), regions 48,249/50,855 (94.88%); retain
these separately and make no complete branch-coverage claim. No new exclusions
or threshold reductions were introduced.

The final coupled editor checks and 71 release workloads pass on a frozen source
copy with unchanged before/after source hashes. Fresh full-workspace coverage is
recorded separately. Concurrent unrelated calendar-spec changes invalidated two
shared-checkout measurement runs; these retain incomplete status. One-sample,
one-iteration measurements remain unassessed, with no baseline promotion.

## Read-only Site companion (2026-10-13)

`dctx0 site export` publishes a portable self-contained React UI; `site serve`
binds a read-only Axum server to loopback with conditional refresh. Both read one
consistent published-history transaction and never check, migrate, reserve or
adopt. `--db`, independent historical snapshot/plan pins and task-event `--as-of`
are supported. Browser integer fields are exact decimal strings; existing
CLI/IR/storage contracts are unchanged. Complete historical case inventories are
compressed per run and decoded on selection.

The OSS HeroUI/React Flow/Dagre UI includes pinned sources, behavior, semantic
architecture, task waves/DAG, changes, evidence and stats. Historical links stay
separate from scheduling prerequisites; Done never implies PASS. Embedded assets
and installed-license receipts reproduce from the committed Bun lockfile.
`python3 scripts/site_acceptance.py` couples Rust/UI/browser checks with release
projection/export/server/render/refresh measurements and retains the latest 20
Site runs. Twelve browser checks and four model tests pass, including actual
CLI transitions, stale recovery, keyboard/mobile behavior, native mutation/replay,
read-only boundaries and explicitly synthetic stress inputs.

The product spec includes `Dctx0.Site` and real snapshot/export adapter calls. Its
bounded verification passes 128 native cases; seven manual obligations remain
UNKNOWN. This is not complete product verification or adoption. Site workloads
have no accepted baseline; existing baselines, Cinema sources and compiler
goldens are unchanged. See `docs/site/src/docs/site-companion.md` and
`docs/site-companion-validation.json` for final evidence and memory limits.

Site final-source coverage passes 31,886/32,669 merged source lines (97.60%) and
2,599/2,677 raw LLVM functions (97.09%), with identical source hashes. Raw LLVM
lines are 33,480/34,535 (96.95%), regions 49,223/51,905 (94.83%) and
instantiations 3,504/5,653 (61.98%); preserve these separately. No branch
completeness, new exclusion or threshold reduction is claimed. Site error
formatting reuses the standard CLI formatter; all error paths remain present.

The final coupled workspace validation and all 71 release workloads pass in
`benchmarks/history/20261013T195347.552983Z-2148470.json`, with matching source
hashes. Workspace measurements use one sample/iteration and are unassessed;
Site acceptance retains three samples/five projection iterations in
`benchmarks/history/site/20261013T191319.735841Z-2148755.json`. No baseline was
promoted. Final retained evidence is `docs/site-companion-validation.json`.

Site graph readability correction (2026-10-13): source-code CSS no longer
stretches React Flow handles. Horizontal ports, directed relation bundles and
expandable task-wave summaries preserve individual relation details and actual
prerequisite focus. Final Site acceptance stages pass with an interrupted browser
scenario continued without repeating passed Rust/model/build stages; original
failed records remain retained. Speed/RSS and scope are documented in
`docs/site-graph-correction-validation.json`. Previous workspace coverage remains
historical; no Rust logic, specs, compiler goldens or accepted baselines changed.

## Native portability increment (2026-10-15, native matrix accepted)

The executor now shares launch/protocol/streams across hosts. Auto keeps Linux
process groups/subreaper/descendant ownership; macOS and Windows manage the direct
child with real status, kill/wait and bounded draining. NativeLimits can select
Child explicitly for Linux conformance. Disabled optional supervision and null
RSS are capability limitations, outside functional provenance gaps. Portable
runtime identity schema 2 identifies captured-launch evidence, with executable,
arguments, directory and effective environment digest; schema-1 procfs receipts
remain readable. No SQLite migration or change to PASS/adoption/review guards.

`python scripts/native_portability.py --binary <release-cli> --coverage` couples
portable transport, three real targets, mutation/replay, current Cinema review
workflow and release time/RAM. The existing five-host release matrix now requires
these checks on relevant PRs and before new releases, retains failed logs/native
coverage and rejects missing platform reports. All five native jobs and the final
platform gate passed in run 38784717147 for implementation 5d34fbc6f066c5bf251904184493b8a770c95f88.
Each host completed all 25 stages, including real Bun/Go cleanup probes and nine
compiled Rust cleanup probes. Current support covers all five hosts; historical
Linux-only results remain historical. Exact reports, artifact digests, separate
coverage and release speed/RAM samples are in docs/native-portability-native-matrix.json.
Keep four Cinema review tasks Done/Unverified among 105 tasks. The self-testing
CLI path now uses the host executable suffix; existing source reviews are not
silently rebound after that source change.

Current coupled Linux validation passes all checks and tutorials with 85 complete
release workloads in benchmarks/history/20261015T195713.967729Z-330837.json,
matching checked/measured source hashes, 32,262/33,051 merged source lines
(97.613%) and 2,630/2,711 LLVM functions (97.012%). Preserve raw metrics separately
in docs/native-portability-linux-final.json; no branch-completeness claim,
exclusion or threshold reduction is introduced. One-sample/iteration development
measurements remain unassessed; no baseline was promoted. Frozen fixture
dependencies are now prepared before pinning the validation inventory; all
dependency/source inputs and later-change guards remain present. Fresh full CI
for this runner correction is pending, separately from the passed native matrix.


## Native dependency reference budget continuation (2026-10-16)

Native TypeScript resolution now accepts optional
`dependency_resolution.max_references` in the testing configuration: default
10,000, inclusive bounds 1 through 1,000,000. Repeated references still count and
retain individual evidence; a run-local importer/specifier/import-mode cache
reduces native requests. Effective limits and resolver version are immutable
artifacts, participate in adoption identity and are compared during replay.
Budget exhaustion preserves the observed prefix and blocks adoption.

The 73 engine unit tests and all 40 native-verification integration tests pass,
including a real 1,200-file dependency chain/cycle, small-budget rejection,
guarded adoption and legacy/changed-limit replay. An isolated Company Manager
platform-runners comparison captures 1,018 references completely with 62 cache
hits and all 6,246 cases PASS. Budget 1,000 retains exactly 1,000 observations
and the incompleteness diagnosis. Nineteen manual obligations and 45 stale
source-review bindings remain open; whole Company Manager acceptance is not
claimed. The corrected release development binary is installed locally; the
package version remains 0.2.3.

Full all-features workspace tests pass with 540 Rust test executions; CLI
no-default-features, Rust-only and Go-only combinations also pass.
Tutorial steps 1–15 also passed. Remaining acceptance, fresh coverage and final
release measurements were stopped at the maintainer’s explicit request to release
v0.2.4 without further tests. Go Race rejected this host’s 39-bit VMA layout;
Rust debug acceptance exceeded its existing 240-second timeout. The supplemental
release-mode acceptance was stopped before completion. The v0.2.4 exception
skips new CI tests and native acceptance, retaining five builds and packaging;
later tags retain normal gates. Do not infer fresh coverage or performance acceptance from
this checkpoint. Preserve the 97% gates, historical LLVM
metrics, compiler goldens and accepted baseline. See
`docs/dependency-resolution-validation.json` and
`docs/dependency-resolution-company-manager.json` for actual scope and evidence.


## Verification history retention (2026-10-17)

SQLite schema 5 retains three complete published verification runs per project
across scopes and origins. Older runs retain explicit pruned historical summaries,
identity and task/adoption links. A pruned PASS is historical, cannot authorize
new publication and cannot replay. Exact retries may return an existing matching
publication receipt without creating new evidence or events. Schema 1–4 readers
remain read-only; writers migrate additively. Public complete run JSON and content
hash identities remain unchanged.

Canonical case corpora are gzip-compressed and content-addressed. Large artifacts
are compressed when beneficial; digests refer to original bytes. Successful cases
omit duplicate session/individual assessment diagnostics; FAIL/UNKNOWN prefixes
remain. Paginated history avoids full corpus reads, and Site projection holds
compressed corpora and decodes one run at a time. Task detail availability is
separate from immutable historical work/check state.

`verify gc` previews with `--dry-run`; `--compact` rebuilds and verifies a compact
replacement with source-change guards, ordered FK copies, shared/exclusive store
locks and recoverable staging. It creates no backup. `--exports` reduces recognized
embedded JSON run bodies only under an explicitly selected directory and preserves
unrecognized files and symlinks. Existing Company Manager cleanup was explicitly
authorized without backup: 19.74 GB → 1.76 GB database, 3.53 GB → 262 MB exports;
20 other tables/1,139,345 records have unchanged content digests. Three complete
runs and 406 historical summaries remain. See docs/verification-storage-company-manager.json.

`scripts/storage_acceptance.py` couples storage checks and release speed/RSS,
including identical 10,000-case synthetic workloads with 24,000 payload bytes per
case for both Cinema roots. Latest-20 storage reports are separate from accepted
baselines; no baseline promotion is authorized by this increment. See
README.md and docs/site/src/reference/verification-storage.md for commands and
compatibility. Record final workspace/coverage evidence separately from earlier
failed or source-changing development runs.

Storage validation checkpoint (2026-10-17): final focused storage acceptance
`20261017T085811.566463Z-3987.json` passes four stages and 12 release speed/RSS
workloads against unchanged sources. All-feature workspace Rust tests and 41
native engine tests pass; final formatting/Clippy and 20 Python tests pass.
Complete Go/Rust Cinema release acceptance each passes 5,674 cases, 105 tasks,
five real mutations and replay. Merged full-workspace plus supplemental test
coverage is 97.45% source lines and 97.01% functions; preserve the separate raw
workspace 96.76% function metric and supplemental reports. No exclusions or
threshold changes were added. The aggregate runner remains failed/incomplete
because of Go Race's unsupported 39-bit VMA host and source changes during
correction; its 73 measurements cannot become a baseline. Separate Go/Rust
native performance reports each complete 11 phases. See
`docs/verification-storage-validation.json` and
`docs/verification-storage-coverage.json`; baseline promotion remains unauthorized.

## Large bound review inputs (2026-10-17)

Inspect, Attest and Complete share the engine streaming JSON reader with a 64 MiB
default and explicit `--max-input-bytes`. Complete's Rust API retains its default
method and exposes `complete_verification_with_input_limit`. Program inspections
require one exact native basis; unknown/duplicate subjects and invalid bindings
are rejected before publication. No implicit cumulative review or imported-native
conversion is introduced. Native failures and incomplete corpora/provenance remain
authoritative. `scripts/inspection_acceptance.py` couples real 543-module Bun
fixtures and 19/47 full review groups with individual release CLI speed/RSS;
latest-20 reports live under `benchmarks/history/inspection/` and remain unassessed
against a baseline. See `docs/site/src/reference/inspection-inputs.md`. Local
focused tests and Complete notes above 1 MiB pass; fresh CI/release is pending.
Self-verification acceptance must replay its latest successful warm run after
mutations, because the original run can legitimately expire under retention.

## Native example call observation and QA RAM guard (2026-10-18)

Ordered example assertions now evaluate actual adapter calls from the completed
`when` prefix. Available empty traces prove zero; missing observations remain
UNKNOWN and cannot be repaired by a later step. Declared failures retain calls;
mock callbacks do not double count them. Projection requires `call-observation`
for call assertions and reserves `mock-injection` for actual prepared mocks.
The additive `Scenario::deliver_observed` API preserves the legacy delivery API;
SDK wire contracts, SQLite schemas, specs and goldens are unchanged.

The Linux QA runner now uses an independent 50 ms process-tree RSS watchdog with
one third of startup MemAvailable, serial builds/tests by default, and cleanup
of detached descendants. A reachable systemd manager alone is not enforcement:
this host exposes no memory controller. The watchdog also operates without
systemd; its sampled RSS sum counts shared pages per process and is not a hard
kernel allocation limit. This supersedes earlier QA two-thirds/systemd-only
memory guidance; production native executor policy is unchanged.

The bounded workspace run passed 25 Python tests, 562 regular Rust executions
and 563 instrumented executions (including the deferred benchmark test), plus
CLI feature combinations and complete Go/Rust Cinema acceptance with mutations
and replay. Fresh merged source-line coverage is 33,818/34,675 (97.53%);
LLVM functions are 2,772/2,857 (97.02%). Preserve raw LLVM line coverage of
96.89% separately; no branch completeness, exclusions or supplemental merge is
claimed. The process tree peaked at 1,582,419,968 bytes within its 4,304,674,816-byte
budget. All 73 release workloads completed; baseline comparison is unassessed
because metadata/inputs are incompatible, and no baseline was promoted.
The aggregate gate remains failed solely because Go Race requires a 48-bit VMA
and this host provides 39 bits.

Fresh Company Manager channel-artifacts execution with the locally built CLI
passes all 7,315 current cases, including all 7,314 original cases and the
reported outage example, with COMPLETE provenance. One additional EventKind
case comes from intervening consumer work. Ten manual constraints remain open;
this is not a new full 15-scope gate PASS. The run peaked at 950,763,520 bytes
within a 4,332,691,456-byte outer budget; the consumer's 3,072 MiB guard also stayed
active. See `docs/call-observation-validation.json` and the linked native archive.

v0.2.6 release preparation additionally corrected QA subreaper cleanup: finished
adopted children are reaped while the command runs without consuming its exit
status. Nine focused guard tests pass. The installed local v0.2.6 passes 14
release/installer checks (one local pwsh skip) and all five native platform checks,
including cancellation, actual mutation and repaired replay. Three fresh packaging
and startup samples retain speed/RSS measurements. The failed initial cancellation
attempt remains recorded. See `docs/release-v0.2.6-validation.json`; publication
still requires normal CI and all five native release hosts.

## Shared native values and qualified Go race execution (2026-10-19)

v0.2.7 uses `VerificationValue::String(Arc<str>)` with unchanged canonical JSON,
configurable 1 MiB/4 MiB message/evidence defaults and 64 MiB/256 MiB ceilings.
Generator `dctx0-values-4` bounds productive recursive constructors and retains
explicitly non-exhaustive corpora; missing inputs and budget exhaustion never
establish PASS. Rust callers construct text with `.into()`.

`python3 scripts/go_race.py` qualifies the native detector with a clean probe and
a deliberate detected race before uncached Cinema tests. Exit 2 is unavailable,
making aggregate validation incomplete and unsuccessful. This ARM64 kernel has
39-bit VMA; Go race requires 48 bits. There is no plain-test fallback or release
exemption. Release v0.2.7 passed the real Linux x64 race tests, full fresh CI
validation, Gin/Axum acceptance and all five native distribution jobs. CI merged
source-line coverage is 33,892/34,737 (97.57%) and LLVM functions are
2,778/2,861 (97.10%). Raw LLVM line coverage is 96.93%, retained separately;
these are not branch-completeness claims. Preserve local and CI evidence separately.

The fresh bounded local run also passes 33,879/34,737 merged source lines
(97.53%) and 2,776/2,861 LLVM functions (97.03%), with unchanged inputs matching
CI. All 73 release workloads complete; comparison remains unassessed and the
baseline is unchanged. The process tree peaks at 1,546,338,304 bytes within a
3,674,705,920-byte budget. The aggregate is incomplete solely for local Go race
unavailability; never report it as local PASS. Both Go and Rust applications
pass 5,674 cases, five production mutations and replay. See
`docs/go-race-validation.json` for local/CI provenance and release asset checks.

## Workflow efficiency checkpoint (2026-10-21)

Read-only dependency-closed work packages, content-deduplicated deltas and pinned
endpoint progress are implemented. Native diagnostics support summaries, filtered
pages and exact cases; adoption previews reuse native proof assessment without
publishing events, executing application code or rebasing stale pins. The three
embedded skills describe bounded handoffs and guarded delivery. External usage
and timestamped receipt analysis never become execution evidence or task status.

Validation passes 578 all-feature Rust tests and 38 Python tests. Fresh isolated
coverage reports 35,078/36,004 merged source lines (97.43%) and 2,865/2,952 raw LLVM
functions (97.05%), with unchanged 97% thresholds and no new exclusions. This does
not establish complete branch coverage. All 78 release speed/RSS workloads finish
on stable sources with reduced development samples; baseline assessment remains
unassessed and no baseline is promoted. The earlier coverage failure and the
source-drift measurement remain recorded.

The local ARM64 qualified Go race probe still returns RACE_HOST_VMA_UNSUPPORTED;
aggregate local validation is partial. v0.2.8 publication requires fresh full CI
validation and all five native platform gates. See
`docs/site/src/reference/workflow-efficiency.md` and
`docs/workflow-efficiency-validation.json` for scope, raw metrics and provenance.

## Configurable codec and review workflow checkpoint (2026-10-22)

`verify scaffold-codecs` generates editable TypeScript testing-only codecs from
current published IR. Integer, option and enum defaults plus qualified-type
exceptions are explicit CLI/configuration inputs. Existing files are preserved;
dry-run proposals do not create files. Codecs convert representations, not business
rules or production validation. `verify review-changes` compares the selected
current task's latest review with current sources and supplies a blank renewal
template; spec drift blocks that template. It never renews approval or testing PASS.
`tasks package-apply` validates and reconstructs a complete package offline, with
Markdown by default and direct package JSON under `--json`/`--format json`.
`tasks show`, native proof/adoption guards, compiler goldens and SQLite schemas
remain unchanged. Company Manager is a read-only reference for this increment.

The final coupled local run passes 586 regular all-feature Rust executions plus
one deferred benchmark check, 587 instrumented Rust executions, 38 Python tests
and 77 Bun codec conformance cases. Nine additional isolated release-CLI-to-Bun
representation checks pass. Both Go and Rust Cinema applications pass 5,674 cases,
five actual mutations and replay. Fresh isolated coverage is 35,646/36,587 merged
source lines (97.43%) and 2,896/2,984 LLVM functions (97.05%), with unchanged 97%
thresholds and no new exclusions. Raw LLVM lines are 37,334/38,622 (96.67%);
regions are 94.22%, not branch completeness. Non-Rust generated helper coverage
is retained separately: 217/294 lines and 31/36 functions across two configurations.

QA runs serially with test/dev optimization level 1, no debug symbols or incremental
cache, and explicit debug assertions/overflow checks enabled. These local profile
settings are recorded; normal release CI remains required. The process tree peaks
at 1,065,418,752 bytes under a 2,866,156,885-byte watchdog budget. All 81 uninstrumented
release speed/RSS workloads complete on stable compilation inputs with one sample,
one measured iteration and one warm-up. Baseline comparison remains unassessed;
no baseline was promoted. The latest 20 measurement records are retained.

Local aggregate validation is incomplete solely because qualified Go Race reports
`RACE_HOST_VMA_UNSUPPORTED` on this ARM64 host. Do not report a local full gate PASS.
The two deliberately interrupted intermediate runs remain documented separately.
See `docs/workflow-tools-validation.json`,
`docs/site/src/reference/codec-scaffolds.md` and
`docs/site/src/reference/workflow-efficiency.md` for scope, raw metrics and provenance.

Workflow follow-up (2026-10-22): TypeScript codec generator v2 supplies a separate
`decodeInput` path for contract-invalid shapes, while strict decoding and explicit
representation failures remain. Read-only review comparison follows explicit
ReviewRenewal ancestry and identifies `previous_task`; it never inherits approval.
Focused workflow validation passes, with 81 coupled release measurements and
929,284,096 bytes peak sampled process-tree RSS. Full coverage was not rerun;
see `docs/workflow-tools-followup-validation.json`. The accepted baseline is unchanged.
