# DCTX0

**Make intent executable.**

An executable specification language for AI-native software development.

[Get started](docs/site/src/docs/getting-started.md) · [Language reference](docs/site/src/docs/language.md) · [Tutorial](docs/site/src/docs/miniboard.md) · [Build the documentation site](docs/site/src/reference/documentation-site.md)

DCTX0 turns product requirements into a typed, machine-readable contract. That contract drives a dependency-aware task DAG, parallel implementation waves, and verification plans. Once an application exists, DCTX0 checks its actual behavior against the same specification.

People and coding agents develop the specification together. People own the product decisions; agents can write and refine the contracts, then implement them. An independent verification core assesses the observations.

> [!WARNING]
> **Experimental Project**
>
> This project is still experimental and has not been optimized for token efficiency.
> **Use at your own risk!**

## Why DCTX0?

Coding agents can write an implementation and its tests while misunderstanding the same requirement in both. Reviewing more generated code does not necessarily reveal that mismatch.

DCTX0 makes the expected behavior explicit before implementation begins. Data contracts define valid values. Examples describe concrete outcomes. Invariants express general rules. Lifecycles describe state transitions. Architecture intent and review criteria retain the requirements that need human or agent judgment.

The specification becomes a shared language for product owners, developers, and coding agents—and an executable reference for checking what gets built.

**Two central advantages: deterministic validation and coordinated parallel work.** The same contracts that define expected behavior also let DCTX0 derive focused implementation tasks, explain their dependencies, and identify which work can proceed independently.

## From specification to parallel work

DCTX0 derives a **directed acyclic graph (DAG) of implementation tasks** from the validated specification. Each task carries its contract, acceptance criteria, and pinned specification context. Every hard prerequisite explains which result is needed and why, giving people and agents a concrete work plan.

The planner groups tasks into **dependency waves**:

- **The first wave has no open prerequisites** and can start immediately.
- **Tasks within a wave are independent in the prerequisite graph**, so multiple agents or developers can work on them in parallel, with shared-file changes coordinated separately.
- **Later tasks become ready when their own prerequisite results are delivered.** Waves describe dependency depth; unrelated unfinished work in an earlier wave does not impose a global barrier.

Atomic task claims prevent two workers from claiming the same task. A worker can also reserve a dependency-closed slice leading to a later task, then complete its prerequisites in order. DCTX0 supplies the required contracts and predecessor results on demand, so each worker can focus on its assignment while retaining the surrounding requirements.

This makes the specification useful for organizing implementation as well as checking it: it exposes parallel work, preserves dependency order, and gives every worker a shared contract. An external coding agent or team executes the tasks; DCTX0 provides planning, context, and guarded coordination.

```sh
dctx0 tasks plan <spec-root>
dctx0 tasks waves <spec-root>
dctx0 tasks claim-next <spec-root> --actor agent-1
dctx0 tasks claim-next <spec-root> --actor agent-2
```

Relevant specification changes create new adaptation tasks with explicit links to previous work. Historical results remain tied to their original requirements, and work completion stays separate from testing evidence: **Done does not imply PASS.** See the [task guide](docs/site/src/reference/tasks.md) for claims, slices, and change handling.

## Who writes the specification?

**Your coding agent can write it with you.** Describe the product, discuss its rules and edge cases, and let the agent turn those decisions into DCTX0 contracts. The bundled language skill guides specification authoring and validation. You review the intended behavior and resolve open product questions before implementation begins.

At this stage, the agent can focus on the contract: data, operations, dependencies, valid states, and expected outcomes. The specification contains no application implementation. It can express behavior precisely without carrying algorithm bodies, database plumbing, or framework wiring into every discussion.

**The agent can devote its cognitive capacity to the specification.** Its reasoning effort goes into understanding the domain, connecting requirements, and identifying contradictions or missing decisions. Implementation details can wait until the contract is clear. This gives specification work a focused reasoning phase of its own.

That separation makes room for more of the application's behavior in a limited context window. The agent can reason about relationships across the product before loading the implementation details of a particular task. A clear contract gives it fewer product decisions to guess and a concrete reference when it later writes code.

The same benefit applies to people. You can build an understanding of what the application does and review its rules without first reading all its implementation details. When a technical question matters, you can follow it into the relevant code. The specification provides the starting point.

Compilation catches invalid specifications; execution checks whether the application fulfills the modeled requirements. Examples, generated cases, and lifecycle checks help expose mistakes that plausible code alone can hide. Testing the agreed contract against the real implementation helps reduce errors, while keeping the tested scope and remaining gaps visible.

The workflow is **understand the product → specify the contract → review the meaning → implement focused tasks → verify actual behavior**. Language and implementation skills support the agent through both specification and application work.

## A small example

These excerpts come from the [Cinema specification](examples/cinema/dctx0/movie-night.dctx0):

```dctx0
value ReleaseYear = Int {
  range 1888..2100
}
```

The value contract gives the verifier an explicit validity rule, including boundaries and invalid inputs.

Inside the `FilmCatalog` module, a behavioral example states what happens when a film is missing:

```dctx0
example "unknown film details" {
  given FilmCatalogStore.find(filmId) returns None
  when result = FilmCatalogApi.getFilm(filmId)
  expect result fails FilmNotFound
}
```

The surrounding specification declares the port, surface, and failure. A typed fixture supplies `filmId`. During verification, an external test adapter supplies the declared collaborator reply and invokes the real application operation. The central verifier assesses the observed result.

DCTX0 describes **what must hold**. Your application implements the algorithm, persistence, HTTP wiring, and framework integration in its own language.

## What you can do today

- **Compile specifications:** parse multiple files, compose fragments, resolve names, and check typed expressions and clauses.
- **Plan implementation work:** derive tasks, prerequisite waves, acceptance criteria, and pinned context with the required type closure.
- **Coordinate workers:** atomically claim individual tasks or dependency-closed work slices.
- **Verify real applications:** execute examples, generated contract cases, properties, operation checks, and lifecycle paths through native adapters.
- **Investigate failures:** retain observations and counterexamples, shrink supported failing inputs, and replay saved cases.
- **Track change:** compare contract, behavior, fact, and location changes; explain impact paths; record explicit renames.
- **Retain evidence:** store immutable verification runs and task history, with separate automatic results and source-bound inspections.
- **Work in your editor:** use the native language server, local VS Code/Zed extensions, and bundled coding-agent skills.

The CLI is written in Rust and runs independently. It does not invoke an LLM; an external coding agent uses its commands and skills.

### Native targets

| Target | Editable implementation profiles | Native verification | Complete Cinema example |
| --- | --- | --- | --- |
| TypeScript / Bun | Yes | Yes | [Cinema](examples/cinema/dctx0) |
| Go | Yes | Yes | [Cinema Go](examples/cinema-go) |
| Rust | Yes | Yes | [Cinema Rust](examples/cinema-rust) |
| Java | Yes | Not implemented | — |

The Rust core owns input generation, expectations, assessment, shrinking, and replay semantics. Target workers call actual application code and report observations; they do not assign their own PASS verdicts. Production applications remain independent of the DCTX0 runtime, with adapters kept in a separate testing area.

Managed native execution currently targets Linux. Target-specific limits and supported representations are documented in the [TypeScript harness](docs/site/src/reference/bun-harness.md), [Go target](docs/site/src/reference/go-target.md), and [Rust target](docs/site/src/reference/rust-target.md) guides.

## Quick start

### Install a released binary

```sh
curl -fsSL https://github.com/Haardt/dctx0ctl/releases/latest/download/install.sh | bash
export PATH="$HOME/.local/bin:$PATH"
dctx0 --version
```

Supports **Linux x86_64/ARM64 (glibc 2.35+) and macOS Intel/Apple Silicon (14+)**. The installer verifies SHA-256 and the binary version, then installs atomically without sudo. It leaves shell profiles untouched. Run the script with **Bash**.

Pin a published release or choose another destination with `bash -s -- --version v0.1.0 --bin-dir /path/to/bin`. See [releases and installation](docs/site/src/reference/releases.md) for exact pinned URLs and manual downloads.

On **Windows x64**, run in PowerShell:

```powershell
irm https://github.com/Haardt/dctx0ctl/releases/latest/download/install.ps1 | iex
$env:PATH = "$env:LOCALAPPDATA\DCTX0\bin;$env:PATH"
dctx0 --version
```

The Windows installer checks the ZIP checksum and executable version before atomic replacement. The default destination is `%LOCALAPPDATA%\DCTX0\bin`; `-Version` and `-BinDir` provide pinned/custom installation. CLI, specification checks, task planning and LSP are available on all release platforms. **Managed native verification and automatic adoption currently require Linux**. macOS/Windows CI exercises the testing SDK harness and process cancellation separately and checks that unsupported managed execution never reports PASS.

### Install your application toolchains

Install the toolchains for your selected target yourself and make them available on `PATH` before native verification:

- **TypeScript:** Bun.
- **Go:** Go; the Cinema Go example requires Go 1.25+.
- **Rust:** Rust/Cargo and a C compiler for bundled SQLite.

Prepare your application's dependencies with its package manager, such as `bun install`, `go mod download`, or `cargo fetch`. The release installer installs DCTX0 only. Parsing specifications and planning tasks require only the DCTX0 binary.

### Build the CLI from source

Use a current stable Rust toolchain through rustup, Cargo, and a C toolchain for bundled SQLite. All four target features are enabled by default.

From the repository root:

```sh
cargo build --release -p dctx0-cli --locked
export PATH="$PWD/target/release:$PATH"
dctx0 --help
```

Alternatively, install the CLI from this checkout:

```sh
cargo install --path crates/dctx0-cli --locked
```

To build only a selected target:

```sh
cargo build --release -p dctx0-cli --no-default-features --features rust --locked
```

Parsing, compilation, and task planning do not require a native application toolchain. Verification additionally needs Bun for TypeScript, Go 1.25+ for Cinema Go, or Cargo and a C toolchain for Cinema Rust. Prepare native dependencies explicitly before managed execution. Python is used for repository validation scripts, not CLI product workflows.

### Explore the specification and tasks

To try the example applications, clone the repository and run the following commands from its root:

```sh
git clone https://github.com/Haardt/dctx0ctl.git
cd dctx0ctl
```

```sh
dctx0 check examples/cinema/dctx0
dctx0 tasks preview examples/cinema/dctx0
dctx0 tasks plan examples/cinema/dctx0
dctx0 tasks waves examples/cinema/dctx0
dctx0 tasks next examples/cinema/dctx0
```

`check` compiles and publishes a valid snapshot. `tasks preview` shows proposed work; `tasks plan` publishes it. Read queries describe saved, pinned requirements rather than asserting working-tree freshness.

### Run the TypeScript application checks

The reference Cinema project already contains an adapter, fixtures, configured verification scope, and authored inspection notes.

```sh
bun install --frozen-lockfile --cwd examples/cinema

dctx0 verify plan examples/cinema/dctx0 --project examples/cinema
dctx0 verify run examples/cinema/dctx0 --project examples/cinema --no-adopt
```

`plan` projects requirements without executing the application. `run` executes the configured finite plan and stores evidence. `--no-adopt` keeps task progress unchanged.

For complete configured acceptance:

```sh
dctx0 verify complete examples/cinema/dctx0 --project examples/cinema --json
```

This checks the specification, publishes the current task plan, executes the configured native scope, binds existing authored inspections, and requires a nonempty delivered active plan with appropriate testing or source-bound review evidence. Approved Review tasks may remain Done/Unverified and are reported separately; approval never sets testing PASS. It does not generate review approvals. See [complete acceptance](docs/site/src/reference/verification-complete.md).

### Try Go or Rust

```sh
go -C examples/cinema-go mod download
dctx0 verify complete examples/cinema-go/dctx0 --project examples/cinema-go --json
```

```sh
cargo fetch --manifest-path examples/cinema-rust/.dctx0/testing/rust/Cargo.toml --locked
dctx0 verify complete examples/cinema-rust/dctx0 --project examples/cinema-rust --json
```

The applications use the same Cinema specification and central semantic verification plan. Go and Rust compile testing workers through managed native builds, with compiler inputs and worker executables recorded separately from central assessment.

## Learn the language

The [MiniBoard tutorial](examples/tutorial/README.md) introduces the language through 16 cumulative TypeScript applications, with independent production tests and external verification adapters. The final step creates and completes actual tasks.

To open the final application, install its dependencies and start it:

```sh
cd examples/tutorial-step-16-http-replay
bun install --frozen-lockfile
bun run start
```

Open <http://127.0.0.1:4173/>. To give an agent a fresh exercise while retaining the reference solutions, run from the repository root:

```sh
./examples/tutorial/agent-step.sh 1 -- codex exec --full-auto
```

## Use DCTX0 in your own project

Keep the reviewed specification root separate from the application root. Start by compiling the specification, then publish implementation guidance and tasks:

```sh
dctx0 check /path/to/app/dctx0
dctx0 profile typescript create --project /path/to/app --default --agents
dctx0 tasks plan /path/to/app/dctx0
dctx0 tasks show /path/to/app/dctx0 T42 --target typescript
```

`T42` is an example task ID; use one returned by your task plan. Profiles are editable Markdown under `.dctx0/profiles`. They guide implementation without changing language semantics. Used profiles are frozen with claims and reservations; free-form notes are not automatically verified properties.

See [Configure and extend profiles](docs/site/src/docs/profiles.md) for the difference between shared, implement, and use guidance, configuring a project, and editing the shipped defaults in a DCTX0 checkout.

For Go/Gin or Rust/Axum agent guidance, merge the optional framework presets into your local profiles. This is a Markdown choice, not a `--framework` CLI flag. A reviewed project adapter must separately connect the real production router. Existing Cinema HTTP defaults remain available alongside the framework variants. See [HTTP framework profiles and harness setup](docs/site/src/reference/http-frameworks.md).

Inspect structural candidates and initialize a testing adapter:

```sh
dctx0 verify discover /path/to/app/dctx0 --project /path/to/app --target typescript
dctx0 verify init /path/to/app/dctx0 --project /path/to/app --target typescript
```

`discover` reports unconfirmed structural candidates. `init` preserves existing files and creates an explicitly unavailable adapter scaffold. Implement the adapter against real application entry points and supply the observations required by your contracts.

Technical configuration lives in `.dctx0/testing/verification.json`: target, runtime, adapter, fixtures, explicit scope, seed, and budgets. An empty scope does not select the entire application. The default generated-case budget is 100 per obligation; explicit examples remain mandatory. Changed specification sources require a successful `check` before ordinary verification.

Prepared collaborator replies test consumer behavior under controlled conditions. Actual persistence and HTTP integration need real native checks and observers. See [verification setup](docs/site/src/reference/verification-setup.md), [planning](docs/site/src/reference/verification-planning.md), and the target guides for capability boundaries.

## Understand the evidence

| Result | Meaning |
| --- | --- |
| `PASS` | The selected mandatory finite-plan requirements are satisfied. |
| `FAIL` | A check found a concrete violation. |
| `UNKNOWN` | A required statement lacks sufficient evidence. |

Finite testing does not prove correctness for every possible input. Missing observers, unsupported constructs, incomplete provenance, and unexecuted mandatory checks remain visible. Opaque trace labels remain unmodeled; they do not establish resolved dependencies or prove runtime calls.

Work progress (`Open`, `Progress`, `Done`) is separate from testing status (`Unverified`, `PASS`, `FAIL`). **Done does not imply PASS.**

Without `--no-adopt`, a native run may deliver eligible open, unclaimed, unreserved tasks when complete task-specific evidence and transactional guards permit it. Historical results, running work, reviews, and incomplete requirements retain their protections. Imported PASS reports alone cannot authorize native adoption.

Saved evidence can be inspected or replayed:

```sh
dctx0 verify show <spec-root> <run-id> --json
dctx0 verify replay <spec-root> <run-id> --case <case-id> --project <app-root>
```

Replay executes saved inputs as a new comparison and never adopts tasks. Saved reports preserve their original provenance; they do not assert that today's working tree is unchanged.

Narrative requirements, named suites, and reviews can receive explicit source-bound inspections:

```sh
dctx0 verify inspect <spec-root> <run-id> --project <app-root> --file <inspections.json>
dctx0 verify attest <spec-root> <run-id> --project <app-root> --file <task-reports.json>
```

`inspect` publishes an effective program assessment without new execution or task adoption. `attest` delivers eligible inspected task reports through guarded transitions. Automatic and inspection evidence remain separate, and approval cannot override an actual automatic FAIL. Source changes invalidate the relevant bindings. See [verification history](docs/site/src/reference/verification-history.md) and [inspection delivery](docs/site/src/reference/verification-suite-attestation-progress.md).

### Cinema acceptance

The documented complete TypeScript, Go, and Rust Cinema acceptances each cover **5,674 mandatory finite cases**. Current planning assigns 25 manual/suite obligations to four Review tasks; a fresh Cinema plan has **105 tasks**. Source-bound reviews are reported separately from testing PASS. Actual persistence and HTTP checks, deliberate production mutations, and broken/repaired replay exercise the implementation and its wiring.

Native reports retain unresolved manual/suite obligations. Authored source-bound inspections produce a separate effective assessment. These results establish the documented Cinema scope, not unrestricted proof for arbitrary applications.

Evidence: TypeScript (historical evidence is available in Git history), Go (historical evidence is available in Git history), Rust (historical evidence is available in Git history).

## Track changes and coordinate work

```sh
dctx0 parse <spec-root>
dctx0 history <spec-root> --json
dctx0 diff <spec-root> <before-snapshot> latest --json
dctx0 impact <spec-root> <qualified-symbol> --snapshot latest --json
dctx0 rename <spec-root> <qualified-symbol> <new-name> --dry-run --json
```

`parse` also accepts a single file. Snapshot selectors accept `latest`, a full hash, or an unambiguous hexadecimal prefix of at least eight characters. `--json` supplies structured output; task commands default to Markdown. Global `--db <path>` selects another SQLite database.

Specs remain source files. Derived history defaults to `<spec-root>/.dctx0ctl/state.sqlite3`. Successful checks publish transactionally; failed compilation preserves the last valid head. Semantic cycles are supported, while task prerequisites form a separate DAG.

Changed requirements create new adaptation tasks with explicit predecessor links. Historical Done/PASS tasks remain evidence for their old requirements. Explicit renames record continuity; ordinary remove/add edits do not infer identity from similarity. Rename covers declarations and scoped descendants; fields, parameters, and enum alternatives require dedicated future operations. Dry runs write neither sources nor history. Real renames use a project lock and recovery journal for coordinated multi-file updates.

For worker coordination, use `tasks claim-next` or `tasks claim-slice` with an actor. Transitions require the returned claim/reservation and version guards. Context includes required types and acceptance criteria; large contexts expose missing sections and pinned continuation cursors. See [tasks](docs/site/src/reference/tasks.md) and [profiles](docs/site/src/reference/profiles.md).

## Agent skills and editors

The CLI bundles three complementary skills:

| Skill | Purpose |
| --- | --- |
| [`dctx0`](skills/dctx0/SKILL.md) | Navigate the workflow, select work, and interpret verification evidence. |
| [`dctx0-language`](skills/dctx0-language/SKILL.md) | Author, review, and repair specifications, including typed contracts, examples, and lifecycles. |
| [`dctx0-implementation`](skills/dctx0-implementation/SKILL.md) | Implement and repair application tasks and testing adapters using pinned requirements and native verification. |

Install them for your coding agent:

```sh
dctx0 skills list
dctx0 install --project /path/to/app --agent codex,claude
```

Use `--dry-run` to preview. Existing differing content is preserved unless you explicitly use `--force`. Global installation is available with `--global`. See [agent skills](docs/site/src/reference/skills.md).

The native language server supports diagnostics, completion, hover, navigation, symbols, and semantic colors, including unsaved multi-file edits:

```sh
dctx0 lsp --stdio
dctx0 editor install vscode --output "$HOME/dctx0-editors" --activate
dctx0 editor install zed --output "$HOME/dctx0-editors"
```

VS Code activation invokes the `code` CLI. For Zed, select the generated directory through **zed: install dev extension**. These are local extension packages. See [editor setup](docs/site/src/reference/editors.md).

## Architecture

Dependencies point toward the language-neutral core. AST and validated IR remain separate; targets consume validated semantics. Filesystem and process effects belong to adapters.

| Crate / directory | Responsibility |
| --- | --- |
| `dctx0-core` | AST, validated IR, values, diagnostics, shared contracts |
| `dctx0-language` | Lexer, parser, composition, resolution, type checking |
| `dctx0-interpreter` | Pure budgeted expression evaluation |
| `dctx0-verification` | Deterministic corpora, plans, scenarios, assessment, shrinking |
| `dctx0-target-sdk` | Versioned transport and testing-artifact contracts |
| `dctx0-graph` | Semantic dependencies, cycles, governing facts, impact paths |
| `dctx0-changes` | Fingerprints, snapshot diffs, history ports, rename lineage |
| `dctx0-tasks` | Pure planning, prerequisite DAG, reconciliation, context, slices |
| `dctx0-store` | Transactional SQLite snapshots, task events, verification history |
| `dctx0-engine` | Source I/O, publication, locking, native orchestration, recovery |
| `dctx0-lsp` | Language service and LSP transport |
| `dctx0-cli` | Commands, rendering, target registration, benchmark drivers |
| `targets/{typescript,go,rust,java}` | Target profiles and available native implementations |

Compilation follows sorted source discovery → parsing → composition and resolution → type checking → validated IR → semantic graph → fingerprints and snapshot publication. Diagnostics retain source files, codes, and positions.

Native execution uses bounded processes with timeouts, cancellation, output limits, and cleanup. On Linux, an available systemd user scope can cap collective memory at two thirds of startup available RAM, with swap disabled. `DCTX0_MEMORY_GUARD=0` disables this optional cap; unavailable support is reported explicitly. This is resource management, not a security sandbox. See [managed execution](docs/site/src/reference/native-executor.md), [verification architecture](docs/site/src/reference/verification-architecture.md), and [compiler boundaries](docs/site/src/reference/refactoring.md).

## Development and validation

Repository validation requires Python 3, Linux for performance measurement, the Rust coverage tools, and the native target toolchains. CI currently uses Bun 1.4.2, Go selected by the Cinema module, and stable Rust. Consult [.github/workflows/validate.yml](.github/workflows/validate.yml) for the full environment and tutorial/browser checks.

```sh
rustup component add rustfmt clippy llvm-tools-preview
cargo install cargo-llvm-cov --locked
python3 scripts/validate.py --coverage
```

The project runner couples functional validation with release speed and RAM measurements. It runs the relevant workspace/feature checks, formatting, Clippy with warnings denied, tests, native checks, and coverage. Rust production-code coverage must meet **97% merged source lines and 97% LLVM functions**. Raw LLVM metrics and native helper coverage remain separate; line coverage does not establish branch completeness.

Benchmarks use uninstrumented release builds and retain elapsed milliseconds, peak RSS bytes, raw samples, workload identity, input/source hashes, toolchains, cache conditions, and environment provenance. Rust core costs and native build/execution costs are reported separately. Core RSS is Linux post-exec `VmHWM`, including phase preparation; native workloads record their own memory scope. Unsupported measurement is explicit.

The latest 20 runs are retained under `benchmarks/history/`. The accepted baseline lives separately in `benchmarks/baseline.json`. Comparable regressions above **20% runtime** or **10% RAM** fail the performance gate. Missing or incompatible baselines are reported as unassessed. Configure tolerances with `--speed-tolerance` and `--ram-tolerance`.

Baseline promotion is an intentional review step, never automatic:

```sh
python3 scripts/validate.py --accept-baseline latest
# Or select a retained run:
python3 scripts/validate.py --accept-baseline <recorded-filename.json>
```

Reduced samples (`--samples 1 --iterations 1`) and `--skip-performance` are development options. Deferred or partial validation does not pass final performance/coverage acceptance. See [coverage](docs/site/src/reference/coverage.md) and the retained target validation reports for measured scope and limitations.

## Releases

Pushing a version tag such as `v0.1.0` creates a GitHub Release after full validation and all five native distribution jobs pass. The tag must match `[workspace.package].version` and the committed lockfile. Prereleases are marked explicitly and do not replace the installer's latest stable release.

```sh
git tag -a v0.1.0 -m "DCTX0 v0.1.0"
git push origin v0.1.0
```

The [release workflow](.github/workflows/release.yml) publishes Linux x86_64/ARM64 and macOS Intel/Apple Silicon archives, a Windows x64 ZIP, `install.sh`, `install.ps1`, and `SHA256SUMS`. [The maintainer guide](docs/site/src/reference/releases.md) documents release provenance and coupled installer/performance validation.

## Documentation site

The product landing page and English user guide use **mdBook**, a Rust documentation generator. Build and validate the static GitHub Pages artifact with:

```sh
python3 scripts/install_mdbook.py
python3 scripts/docs_site.py --dctx0 target/release/dctx0
```

Build the release CLI first using the quick-start instructions. The runner checks local links and complete DSL examples, then records release site-build speed/RSS. Output is `target/docs-site`; use `target/docs-tools/mdbook serve docs/site --port 4180` for a local preview.

[Publishing and maintenance](docs/site/src/reference/documentation-site.md) explains the GitHub Actions workflow, Pages setup, and source layout.

## Documentation

| Guide | Topic |
| --- | --- |
| [Tutorial](examples/tutorial/README.md) | Learn the language through working applications |
| [Task guide](docs/site/src/reference/tasks.md) | Planning, context, claims, slices, history |
| [Profiles](docs/site/src/reference/profiles.md) | Editable implementation guidance |
| [Verification planning](docs/site/src/reference/verification-planning.md) | Scope, fixtures, mandatory requirements |
| [Run and replay](docs/site/src/reference/verification-run.md) | Native observations, counterexamples, adoption |
| [Complete acceptance](docs/site/src/reference/verification-complete.md) | Authored inspections and task completeness |
| [Go target](docs/site/src/reference/go-target.md) / [Rust target](docs/site/src/reference/rust-target.md) | Native setup, provenance, capability limits |
| [Skills](docs/site/src/reference/skills.md) / [Editors](docs/site/src/reference/editors.md) | Agent and editor integration |

Capability descriptions reflect the Rust workspace as of October 12, 2026. DCTX0 is under active development; production-code generation and native Java execution remain future work.

## License

[MIT](LICENSE).
