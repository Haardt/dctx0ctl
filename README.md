# dctx0ctl

Rust reimplementation of dctx0 from `../runatron`. The Cargo workspace implements the agreed foundations: deterministic multi-file compilation, typed expression IR, semantic dependencies and impact paths, versioned snapshots and change fingerprints, transactional SQLite history, explicit symbol renames, and target-neutral task planning with versioned work history.

Both complete example workspaces compile: Cinema (2 sources) and runatron-next (18 sources). Tests compare their statement trees and data/operation models against independently generated TypeScript fixtures. Compilation validates declarations and expressions. Target-neutral task planning and administration are available. Pure expression evaluation and explicit finite-plan assessment are available as library foundations. Library drivers execute pure cases, prepared examples and observed lifecycles through a generated Bun harness. The complete verification CLI, immutable run history and automatic task adoption remain under implementation. Four target crates provide editable profiles; TypeScript/Bun additionally supplies the harness. Opaque `calls Boundary.operation(...) exactly n` trace labels accepted by the reference language remain explicitly unmodeled in IR; they do not establish a resolved dependency or prove native execution.

## Setup and commands

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

`python3 scripts/validate.py --coverage` runs Python runner tests, rustfmt, Clippy with warnings denied, Rust tests, and the 100% line/function coverage gate. Every validation run also benchmarks uninstrumented release binaries, including failed validation runs. Coverage tools and compiler must use the same rustup toolchain. The gate merges LCOV counts by source line and retains the 100% LLVM function threshold. Raw LLVM instance-summary percentages remain visible; see [coverage measurement](docs/coverage.md) for the observed reporting distinction. No source/error paths are excluded. Coverage does not establish complete behavioral correctness or native execution.

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

`python3 scripts/validate.py --coverage` now measures 84 workloads, including
profile creation, loading, rendering and claim binding for all four targets and
both examples, plus core-only plan projection, input generation, unbound expression evaluation and assessment. New workload baselines remain unassessed until explicitly accepted.

The adaptive external-harness direction and remaining stages are recorded in
[verification-architecture.md](docs/verification-architecture.md). The initial
[verification foundation](docs/verification-foundation.md) introduced pure library
assessment. Published planning is now available; native harnesses and automatic
task adoption remain outstanding.
[Validation evidence](docs/verification-validation.md) records 142 Rust tests,
100% merged source lines/functions and the isolated release measurement.

[Deterministic input generation](docs/verification-generation.md) supplies valid
corpora and parameter assignments for upcoming spec-driven execution. This does
not yet implement the native verification workflow.

Published verification planning is now available with `dctx0 verify plan <spec-root>
--subject <qualified-name>` or `--task T31`. It retains typed requirements, examples,
property cases and lifecycle paths, reports missing scenario inputs, and reads the
published check without modifying tasks. Use `--json` for the complete plan;
`--historical-check <id>` explicitly selects historical requirements. See
[verification planning](docs/verification-planning.md). Native harness execution,
run history and automatic adoption remain outstanding.


The TypeScript target now generates a testing-only Bun worker through the separate
[dctx0 target SDK](docs/bun-harness.md). Native conformance exercises sessions,
collaborator requests, events/state and an actual Cinema tie call. These transport
tests do not establish complete Cinema verification or automatic task adoption.
Bun 1.4.2 is required for the target conformance tests and installed in CI; native
costs are recorded separately. `--focus native-transport` selects partial development
validation while still running release measurements; full acceptance continues to
use `python3 scripts/validate.py --coverage`.


The engine now supplies a [managed Linux native executor](docs/native-executor.md)
with bounded protocol/log streams, cancellation, timeouts and child cleanup. Its
pure-case continuation checks the complete Cinema winner invariant against actual
Bun calls and detects an isolated application mutation. The public CLI still
exposes planning; complete Cinema scenario execution, run history and automatic
adoption remain outstanding. `--focus native-executor` selects partial development
checks with paired release measurements; full coverage requires workspace focus.


[Central Cinema scenario execution](docs/cinema-scenarios.md) now checks all twelve
examples against actual consumer services through generated Bun sessions, central
port mocks and observed events/failures. Isolated production mutations are detected.
These scenario results alone do not prove production persistence, state,
lifecycles or automatic task adoption. Managed `verify run` now combines the
available execution layers; the complete
remaining scope stays in [the Cinema checklist](docs/cinema-verification-progress.md).

Cinema now supplies actual SQLite production ports and explicit Hono/Awilix
composition. Its external test adapter can observe real calls and persisted state;
Rust evaluates connected lifecycle paths against those observations. See
[real-state scope and open points](docs/cinema-state-lifecycles.md). This remains a
library/native continuation: the complete verification run/history/adoption CLI
is still being implemented. Native application coverage is measured separately
from Rust and generated helper coverage.

For a narrower development check, run
`PATH=/home/held/.cargo/bin:$PATH python3 scripts/validate.py --focus verification --samples 1 --iterations 1`.
It installs the Cinema dependencies from the frozen Bun lockfile, runs the selected
verification/native tests, and records release speed/RSS measurements. Full
acceptance still requires `python3 scripts/validate.py --coverage`; reduced
measurement settings do not assess the accepted default-sample baseline.


`dctx0 verify init <spec-root> --project <app-root> --target typescript` now
creates preserved testing artifacts and an explicitly unavailable adapter scaffold.
Its default scope is empty. `verify plan` can load the saved application
configuration, scope and reviewed inputs; explicit selectors override configured
selectors. See [setup and configured planning](docs/verification-setup.md). Neither
setup nor planning executes the application or adopts tasks. Release validation
now includes separate fresh-setup workloads for both complete example sources.


Writers now add verification history through SQLite schema 4 while readers accept
schemas 1–4 without migration. `dctx0 verify show <spec-root> <run-id>` reads
immutable saved evidence in Markdown or JSON without asserting working-tree
freshness. Run recording alone never updates task progress or testing status.
See [verification history and evidence limits](docs/verification-history.md).
Managed `verify run` now executes the generated Bun harness and stores actual
observations from published requirements. See [run workflow](docs/verification-run.md).
`verify replay` executes saved case inputs against a new captured implementation.
`dctx0 verify discover <spec-root> --project <app-root>` now provides read-only
Tree-sitter TypeScript/TSX inventory and unconfirmed name candidates from a
published snapshot. It executes neither application nor adapter, adopts nothing,
and creates no testing files. Syntax-error sources are shown but excluded from
suggestions; multiple candidates remain ambiguous. Class members are structural
candidates, never proof of an instance, DI registration or runtime type.
The inventory skips dependencies, testing artifacts and common generated output;
it rejects symlinks and limits source count/bytes, visited entries and depth.
See [run workflow and evidence limits](docs/verification-run.md).
`bench <spec-root> verification-discover 1 --db <temporary-database>` measures
read-only application inventory/parse separately; compiler publication is setup.
Validation includes discovery workloads for both examples, with source hashes
and Linux process RSS. New workloads have no accepted baseline yet.
Guarded adoption and complete operation obligations remain required.
The current whole-Cinema report is UNKNOWN. Functional tests pass;
the new code's complete coverage gate is still being restored.
