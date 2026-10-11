# dctx0ctl

Rust reimplementation of dctx0 from `../runatron`. The Cargo workspace implements the agreed foundations: deterministic multi-file compilation, typed expression IR, semantic dependencies and impact paths, versioned snapshots and change fingerprints, transactional SQLite history, explicit symbol renames, and target-neutral task planning with versioned work history.

Both complete example workspaces compile: Cinema (2 sources) and runatron-next (18 sources). Tests compare their statement trees and data/operation models against independently generated TypeScript fixtures. Compilation validates declarations and expressions. Target-neutral task planning and administration are available. Rust centrally evaluates finite verification plans, while generated Bun workers execute actual TypeScript applications through external adapters. The verification CLI, immutable run history, replay and guarded task adoption are implemented. Cinema complete acceptance is documented in [the Rust CLI guide](docs/verification-complete.md); the TypeScript tutorial extends this workflow through 16 cumulative applications. Four target crates supply editable profiles; TypeScript/Bun has CLI execution, and the Go crate additionally supplies SDK worker transport conformance. Opaque `calls Boundary.operation(...) exactly n` labels remain explicitly unmodeled: they do not establish resolved dependencies or prove runtime calls.

## Setup and commands

The [MiniBoard tutorial](examples/tutorial/README.md) introduces the language
step by step with real TypeScript applications, independent production tests
and external verification adapters. Its final page creates and completes actual
tasks. Run it with `cd examples/tutorial-step-16-http-replay && bun run start`,
then open <http://127.0.0.1:4173/>. Use
`./examples/tutorial/agent-step.sh 1 -- codex exec --full-auto` to give your
agent a fresh exercise while retaining the complete reference solutions.

Use rustup and place `~/.cargo/bin` before system Cargo in `PATH`. Python 3 and Linux are required for the benchmark runner.

```sh
rustup component add rustfmt clippy llvm-tools-preview
cargo install cargo-llvm-cov --locked
cargo run --bin dctx0 -- check examples/cinema/dctx0
cargo run --bin dctx0 -- check examples/runatron-next/dctx0 --json
python3 scripts/validate.py --coverage
```

Commands accept a spec directory; `parse` also accepts a single file. `.dctx0` discovery is recursive and sorted, skips symlinks, and excludes generated/build directories. Diagnostics use relative files, codes, half-open UTF-8 byte ranges, and Unicode scalar columns. Symbol IDs are fully qualified names, such as `MovieNight.Catalog.Film`.

```sh
dctx0 parse <root>
dctx0 check <root> --json
dctx0 history <root> --json
dctx0 diff <root> <before-snapshot> latest --json
dctx0 impact <root> <qualified-symbol> --snapshot latest --json
dctx0 rename <root> <qualified-symbol> <new-name> --dry-run --json
dctx0 rename <root> <qualified-symbol> <new-name>
```

Use `cargo run --bin dctx0 --` in place of `dctx0` when the binary is not installed. Snapshot selectors accept full hashes, unambiguous hexadecimal prefixes of at least eight characters, or `latest`. Global `--db <path>` selects a different SQLite database; relative database paths resolve against the working directory. Semantic or command errors exit unsuccessfully.

## Architecture and history

| Crate | Responsibility |
| --- | --- |
| `dctx0-core` | Source AST, typed IR, expressions, reference spans, diagnostics |
| `dctx0-language` | Lexer/parser, fragments, scoped resolution, type checking |
| `dctx0-graph` | Typed dependencies, governing facts, cycles, impact explanations |
| `dctx0-changes` | Snapshot projections, fingerprints, diffs, history ports and rename lineage |
| `dctx0-store` | SQLite adapter for history queries and transactional publication |
| `dctx0-engine` | Source discovery, project locks, compilation/publication, rename journals |
| `dctx0-cli` | Human-readable and JSON commands, benchmark drivers |

Compiler orchestration lives in `compiler.rs`; declaration models, relations, symbol resolution, expression parsing, and clause handlers have separate modules. Resolution indexes qualified-name suffixes, and declaration/clause compilation indexes nodes by owner. Snapshot fingerprints use named serialization projections. Diff analysis constructs borrowed graph indexes once per changed snapshot pair; unchanged diffs skip traversal preparation.

History contracts and rename continuity rules belong to `dctx0-changes`; SQLite implements `HistoryReader` and `HistoryWriter`. The engine separates filesystem access, check publication, pure `plan_rename(&sources, symbol, new_name)`, and journal execution/recovery. Its `SourceReader`/`SourceWriter` ports describe source access. The CLI separates arguments, dispatch, rendering, benchmarks, and shared source/RSS helpers; the parser driver remains isolated from history dispatch. See [refactoring boundaries](docs/refactoring.md) for extension guidance.

Rust callers import the relevant history/source traits to use their methods. Database opening takes `AccessMode::ReadOnly` or `AccessMode::ReadWrite`. Change facets and state phases are enums; their serialized spellings stay unchanged. These Rust API changes do not migrate the SQLite schema or CLI/IR JSON formats.

Specs remain source files. Default history is `<root>/.dctx0ctl/state.sqlite3`, with nodes, typed edges and origin spans, fingerprints, source/IR snapshots, successful/failed runs, and rename events. SQLite uses WAL and full synchronization. A successful check publishes one transaction; repeated snapshots deduplicate while retaining every run. An invalid spec records diagnostics and leaves the previous valid head intact. Unknown schemas and corrupt data cause errors and are never silently reset.

Diffs distinguish contract, behavior, facts, and location changes; raw source changes remain visible even when semantic hashes match. Impact considers the old and new graphs, including removals. Dependency cycles are legal and traversal terminates with deterministic explanation paths. This semantic graph is separate from the implemented task prerequisite DAG.

## Explicit renames and recovery

Existing data-only `stableId` clauses retain their meaning. Ordinary edits produce additions/removals; matching names or similar semantics never infer renames. The explicit rename command resolves declaration/reference spans, previews exact edits, recompiles the proposed workspace, and records continuity across rename chains. It preserves narrative text and unrelated names. The initial implementation renames declared symbols and their scoped descendants; fields, parameters, and enum alternatives require future dedicated operations.

A dry run writes neither specs nor database. A real rename acquires the project lock and writes a durable `.dctx0ctl/rename-journal.json` before replacing files atomically. Its snapshot and rename event commit together. Following interruption, `check` or another real rename completes committed edits or rolls back uncommitted edits. Unexpected intervening file changes stop recovery and retain the journal for inspection. Filesystem-wide atomic visibility is unavailable: editors may observe intermediate file replacements, while tool commands coordinate through the project lock.

## Validation and performance

`python3 scripts/validate.py --coverage` runs Python runner tests, rustfmt, Clippy with warnings denied, Rust tests, and the user-approved 97% line/function coverage gate. Every validation run also benchmarks uninstrumented release binaries, including failed validation runs. Coverage tools and compiler must use the same rustup toolchain. The gate merges LCOV counts by source line and retains the 97% LLVM function threshold. Raw LLVM instance-summary percentages remain visible; see [coverage measurement](docs/coverage.md) for the observed reporting distinction. No source/error paths are excluded. Coverage does not establish complete behavioral correctness or native execution.

For each example, measure parsing, compilation, graph construction, snapshots, unchanged/changed diffs, persistence, and unchanged/changed checks. The changed scenario adds a small domain/type in memory and alternates snapshots for checks; original examples are untouched. Persistence uses temporary databases. One warm-up precedes five fresh-process samples; parsing uses 20 iterations and other phases up to three. Timings exclude phase preparation and source loading; parser timing includes lexing, syntax validation, AST construction, and disposal. Check timing includes compilation, graph/snapshot construction, and SQLite publication, using already loaded sources.

RSS is Linux post-exec `VmHWM` in bytes for the standalone Rust process. Phase RSS includes prepared inputs, retained comparison snapshots, and SQLite caches; it is not incremental phase allocation. The isolated parser driver avoids linking history/CLI dispatch into the parser microbenchmark. No subprocess/compiler memory is included in these foundation workloads. Unsupported measurement is an explicit failure.

Raw samples, medians, peak RAM, wall time, input/source hashes, toolchain, platform, and hardware are retained in the latest 20 JSON runs under `benchmarks/history/`. Locked atomic writes protect history updates. The separate accepted `benchmarks/baseline.json` survives pruning.

```sh
python3 scripts/validate.py --coverage
python3 scripts/validate.py --accept-baseline latest
python3 scripts/validate.py --accept-baseline <recorded-filename.json>
```

Baseline promotion is an explicit operation and requires a complete measurement with passing validation. No tests automatically promote it. Comparable regressions above speed 20% or RAM 10% fail the gate; configure independent tolerances with `--speed-tolerance` and `--ram-tolerance`. New workloads without accepted values are listed as unassessed; comparable existing workloads still enforce their gates. Hardware/toolchain/workload mismatches are reported explicitly. Establish CI baselines on a consistent runner.

## Reference fixtures

`fixtures/parser/` and `fixtures/semantic/` originate from the TypeScript implementation. Regeneration requires sibling `../runatron` and Bun:

```sh
bun scripts/reference-parser.ts
bun scripts/reference-semantic.ts
```

Review fixture changes as language compatibility changes. `fixtures/semantic/snapshot-hashes.json` captures the pre-refactoring snapshot IDs and full serialized snapshot hashes for both complete examples. Its regression test checks all IR fields, graph spans, and fingerprints; review changes to this fixture as persisted-format changes.

Independent Rust tests also assert expression precedence, typing, source positions, fragments, ambiguity/errors, graph cycles, fingerprints, persistence, rename chains, races, and interruption recovery. Copied historical `.runatron` artifacts are reference material only.

## Task planning and administration

Tasks now use the valid stored spec snapshot and SQLite history. Start with
`dctx0 check <root>` followed by `dctx0 tasks preview <root>` and
`dctx0 tasks plan <root>`. `tasks list`, `waves`, `next`, `show`, `context`,
`history`, `dependencies`, `diff` and `slice` read saved plans. Markdown is the
new task-command default; `--json` remains available. `show` and `next` include the executable assignment, specific acceptance criteria and complete spec excerpts with required types. Large assignments expose missing required sections and pinned continuation cursors.

Use `claim-next` or `claim-slice --actor <name>` for atomic work allocation;
complete/release with the returned claim token and state version. Work completion,
review and executed-check evidence stay separate. Spec changes create new work
with predecessor links rather than reopening historical tasks. See
[the task guide](docs/tasks.md) for full commands, report formats, migrations,
pagination, domain slices and explicit follow-up.

## Editable implementation profiles

Four statically registered targets supply editable architecture guidance without
changing language semantics or executing implementations:

```sh
dctx0 profile rust create --project /path/to/app --default --agents
dctx0 tasks show /path/to/app/dctx0 T42 --target rust
dctx0 tasks context /path/to/app/dctx0 T42 --section implementation
dctx0 tasks profile-changes /path/to/app/dctx0 --task T42 --format json > update.json
dctx0 tasks profile-update /path/to/app/dctx0 --file update.json --reason "Adopt revised architecture"
```

TypeScript, Java, Go and Rust are enabled by default. Select shipped targets with
`cargo build -p dctx0-cli --no-default-features --features rust`. Profiles need no
native compiler setup; production generation and complete native verification remain deferred. See
[the full profile workflow](docs/profiles.md) for roles, discovery, frozen claims,
missing-file handling, migration 3, report schema 2 and selective adaptation.

`python3 scripts/validate.py --coverage` supports 92 release workloads, including
profile creation, loading, rendering and claim binding for all four targets and
both examples, plus core-only plan projection, input generation, unbound expression evaluation and assessment. New workload baselines remain unassessed until explicitly accepted.

The adaptive external-harness architecture is recorded in
[verification-architecture.md](docs/verification-architecture.md). Rust owns
interpretation, input generation, assessment, shrinking and replay semantics.
The TypeScript target generates testing-only Bun helpers; an external project
adapter calls actual application code. Production imports no dctx0 runtime or
harness. Execution uses bounded Linux processes with cleanup, rather than a
security sandbox. See [the SDK and harness](docs/bun-harness.md) and
[managed execution](docs/native-executor.md).

The implemented workflow is:

```sh
dctx0 check examples/cinema/dctx0
dctx0 verify discover examples/cinema/dctx0 --project examples/cinema
dctx0 verify init examples/cinema/dctx0 --project examples/cinema --target typescript
dctx0 verify plan examples/cinema/dctx0 --project examples/cinema
dctx0 verify run examples/cinema/dctx0 --project examples/cinema --no-adopt
dctx0 verify show examples/cinema/dctx0 <run-id>
dctx0 verify replay examples/cinema/dctx0 <run-id> --case <case-id>
```

`discover` produces unconfirmed structural candidates without executing code.
`init` preserves existing testing files and creates an unavailable scaffold.
A reviewed adapter must supply actual entry points before native checks can pass.
Configuration defaults to an empty scope; explicit subjects or tasks override the
configured selection. Planning preserves mandatory examples and reports missing
lifecycle inputs without executing the adapter. Changed spec sources require a
successful `check`; verification never publishes a compiler head implicitly.
See [setup](docs/verification-setup.md) and
[planning](docs/verification-planning.md).

The central application configuration is `.dctx0/testing/verification.json`.
Its `budgets.generated_cases` defaults to 100 per obligation; set it to 1 for a
small development corpus. Explicit examples remain mandatory, and the saved
plan reports the chosen corpus and budgets. Cinema's adapter configuration is
separate at `.dctx0/testing/typescript/config.json`.

Schema-4 writers preserve immutable runs, provenance and artifacts; schema-1–4
readers do not migrate. `show` reports saved evidence without asserting current
working-tree freshness. `replay` executes saved inputs against a new capture,
reports changed inputs and never adopts tasks. Prepared port replies, actual
calls/events/resources and before/after states are assessed centrally. Native
operation and lifecycle setup must satisfy its declared preconditions. Missing
observers, missing model projections and unsupported requirements remain UNKNOWN.
See [run/replay and evidence limits](docs/verification-run.md) and
[history](docs/verification-history.md).

Without `--no-adopt`, complete task-specific native evidence may atomically deliver eligible Open,
unclaimed and unreserved tasks using the ordinary transition/profile guards.
Imported PASS reports cannot authorize this. Incomplete evidence, changed sources,
stale pins and unmet prerequisites prevent adoption. Historical Done/PASS work
remains unchanged. SQLite rollback concerns task events, results and bindings;
it never reverts code, adapters or specs.
An overall UNKNOWN for unrelated manual requirements or named suites does not
prevent fully proved tasks from being delivered. Every mandatory native case
must still pass; each delivered task needs complete requirement mapping and
provenance. Reviews, integrations and tasks with incomplete requirements remain
open. To enable adoption, initialize the implementation profiles and publish a
current task plan explicitly:

```sh
dctx0 profile typescript create --project examples/cinema --default
dctx0 tasks plan examples/cinema/dctx0
dctx0 verify run examples/cinema/dctx0 --project examples/cinema
```

The frozen direct-function and Awilix/Hono fixtures pass the same central plan,
and mutations of actual business code and HTTP wiring are detected. The complete
Cinema scope runs through the generated harness: **5,674 mandatory cases, all
PASS**, including 14 actual HTTP cases. The reference project has **104 tasks
Done/PASS, zero open**. Native evidence remains UNKNOWN for manual requirements;
explicit source-bound inspections produce a separate effective PASS without
rewriting native observations. See [completion evidence](docs/cinema-complete-validation.json).
Release measurements can be restricted to Cinema with
`python3 scripts/validate.py --coverage --example cinema`; the two-example
benchmark conformance check is deferred in this scope.

During implementation, performance measurements are deferred at the user's
request. Run functional checks with:

```sh
PATH=/home/held/.cargo/bin:$PATH python3 scripts/validate.py --focus verification --skip-performance
```

Results are saved separately in `target/validation-without-performance.json`.
The deferred mode also skips the expensive CLI benchmark conformance test and
names it in the report. With `--coverage`, this produces explicitly partial
coverage and cannot pass final acceptance. Default validation retains that test.
This does not pass a performance gate or alter the accepted baseline. Final
acceptance still requires full workspace/feature checks, 97% merged Rust source
lines and LLVM functions, and separately reviewed release speed/RSS measurements.
The retained assessment-11 coverage run has 98.83% lines and 97.57% functions;
newer focused tests do not establish a new coverage percentage. Native Bun helper
coverage is recorded separately from Rust. Earlier foundation coverage and
measurement reports are historical evidence, not proof for the current tree.

Validation and coverage automatically use an available Linux systemd user scope capped at
two thirds of startup `MemAvailable`, collectively including all child processes,
with additional swap disabled. Other platforms and systems without a reachable
systemd user manager run without this cap, with an explicit message. Set
`DCTX0_MEMORY_GUARD=0` to disable it, including for Rust `verify complete`.
Once a scope is selected, creation errors fail execution rather than retrying
the application. Guarded Cargo jobs and Rust test threads default to two.
For direct checks use `python3 scripts/memory_guard.py dctx0 verify run <spec-root>`.
The native runner alone has time/output limits but no hard RAM limit. Functional
checks and coverage run first; benchmark conformance and release speed/RSS run
last. `python3 scripts/validate.py --coverage --skip-performance` defers those
final stages and records their status as deferred. The memory budget is shared
within one guarded invocation; concurrent independent invocations have separate
budgets and should not be started together.

Cinema entity-key checkpoint (2026-10-11): Assessment 12 executes 5,660 mandatory
cases with PASS, including five explicit identity checks over actual persisted
operation states. 46 of 104 tasks are delivered; 58 remain open. The whole result
is still UNKNOWN for the 18 manual requirements and nine undefined proof/test
suites. Actual SQLite duplicate-persistence mutation and identity replay pass.
See [entity identity evidence](docs/cinema-entity-identity-validation.json).

Complete Cinema acceptance is available directly from Rust, without Python:

```sh
dctx0 verify complete examples/cinema/dctx0 --project examples/cinema
```

It executes the configured native corpus, verifies existing source-bound review
notes, publishes effective evidence and requires a nonempty task plan with zero
unfinished/non-PASS tasks. Use `--json` for the summary or `--inspections <file>`
for another authored notes file. When available and enabled, the Linux command caps collective CLI/native
memory at two thirds of available RAM using the systemd user manager. Bun is
still needed to run TypeScript. See [the complete command](docs/verification-complete.md).

Explicit inspected task delivery is available as
`dctx0 verify attest <spec-root> <native-run-id> --project <app-root> --file <reports.json>`.
The file contains a JSON array of `TaskAttestation` objects. Each inspection pins
its task (`T<ID>`), snapshot, native plan hash, complete requirement IDs and
implementation artifact hashes, actor, report, limitations and verdict. Review
tasks also require a structured approved review without unresolved high/action
findings. Delivery requires the complete actual native corpus and unchanged
inputs/profiles. It preserves automatic evidence and records agent evidence in
normal task history; it does not manufacture an automatic native PASS. See
[implementation checkpoint](docs/verification-suite-attestation-progress.md).

`dctx0 verify inspect <spec-root> <native-run-id> --project <app-root> --file <reports.json>`
stores an effective suite/review/Release assessment from a JSON array of
`ProgramInspection` reports. Automatic and agent outcomes remain separate. The
inspection report references the unchanged native source and does not claim a new
execution or authorize automatic task adoption. Current source/check/profile
bindings are checked before publication.
