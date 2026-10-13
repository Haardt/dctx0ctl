# Rust product self-verification

`dctx0 verify self .` captures this repository and executes the own specification
through the existing Rust harness. The testing adapter lives in
`.dctx0/testing/rust/`; production crates do not import it or the specification.

```sh
cargo build -p dctx0-cli --all-features --locked
./target/debug/dctx0 verify self . --json
# Explicit partial development scope:
./target/debug/dctx0 verify self . --subject Dctx0.Compiler --subject Dctx0.Evidence
# Coupled affected regressions, self cases, production mutations and release measurements:
python3 scripts/self_verification_acceptance.py --checks --mutations
```

Use a rustup toolchain with Cargo/Rust sources and the committed lockfiles. Build
receipts inventory the compiler, standard library and SQLite host C toolchain.
The first managed acceptance platform is Linux. No systemd scope is required.

The engine captures required manifests, crates, embedded editor/skill/target
resources, fixtures, specification, adapter, packaging sources and lockfiles into
`target/self-verification/<run>/`. Build directories, node_modules and existing
project databases are excluded. Uncaptured compiler inputs reject the receipt.
The captured CLI is built before executing cases. Its dedicated executable
capture has a 256 MiB budget, SHA-256 and a compiler-input receipt; ordinary
sources retain their 16 MiB per-file limit. The native worker has its own receipt.

The adapter invokes the real compiler, requirement coverage, engine/store task
transitions, SQLite publication and profile operations. CLI commands and LSP
requests use the actual captured executable. Inner native tests use an independent
small Rust fixture and never invoke self-verification recursively. Independent
cases have fresh sessions; ordered examples share one session and release their
files, connections and processes at closure. Managed cleanup also tracks nested
workers that create independent process groups.

The default reviewed configuration uses seed 42, at most 16 generated cases per
obligation and collection size 2. Every explicit example remains mandatory.
Self-execution limits are 60 seconds initialization, 60 seconds per case, 15
minutes execution and 5 seconds cleanup. Compilation has a separate budget.
`ApiInt` explicitly limits test-facing integer APIs to Rust's signed 64-bit range;
there are no hidden input defaults or narrowing conversions.

`<captured-root>/.dctx0ctl/self-verification.json` reports exact source hashes,
captured root, CLI hash,
preparation/build/execution durations and the immutable native report. A source
change during execution prevents current confirmation. `partial` remains explicit;
a successful partial command does not set `machine_complete`. Machine completeness
requires every unique mandatory case to pass, complete native provenance and no
open machine obligations. Free-form architecture/review requirements remain
separate; their UNKNOWN status does not become automatic PASS.
The report is derived state outside the captured source inventory, so publishing
it does not invalidate subsequent inspection or replay.

The development project's tasks are never adopted. Adoption, idempotence,
no-adopt and replay are tested inside isolated projects. Source-bound architecture
reviews can subsequently be delivered with `verify inspect` and `verify attest`
against the captured specification/application and saved native run. Follow the
[conformance review workflow](agent-evidence.md); authored
approval remains distinct from testing PASS. Do not approve an uninspected
requirement or treat a compiler/evidence subset as whole-product acceptance.

The acceptance runner records raw speed and child/descendant peak RSS using Linux
`wait4`, separates preparation/CLI build/warm execution/replay, and retains the
latest 20 JSON reports under `benchmarks/history/self-verification/`. These
workloads have no accepted baseline; the core baseline is unchanged. Independent
fixtures, existing regressions and actual production mutations remain necessary
because the specification and verifier could share a defect.

Compatibility is additive: verification configuration accepts optional per-project
`execution` budgets and preserves the previous serialized shape when absent.
Native command audits now keep `stdout` separately from `stderr`; old audits remain
readable with an empty default stdout. Rust/Go syntax inspection records its language
scope while every embedded asset remains bound by bytes. Compiler IR, snapshot
goldens and historical task definitions are unchanged. The LSP canonical-URI token
fix routes requests to the already analyzed open source. The Rust Cinema adapter
and Zed package lockfile versions were corrected from `0.1.1` to their manifests'
`0.1.0`, without dependency upgrades.

Workspace LLVM coverage measures production crates, not the separately built project
adapter or generated harness. Native scenarios and mutations exercise those testing
helpers; no exhaustive helper, branch, all-input or cross-platform coverage is claimed.

Acceptance on 2026-10-13 executes all 1,115 mandatory cases across eight product
areas, including all 14 original examples and 29 additional examples. Every case
is PASS, the required case set is complete, and the original source inventory
remains current. Compiler and evidence mutations produce three and one relevant
failures respectively. The nested native fixture also exercises production
mutation detection and broken/repaired replay. Twelve source-bound inspections
complete the manual requirements separately; the saved automatic run retains
UNKNOWN for those requirements.

[Retained validation evidence](../../../self-verification-validation.json)
and [authored inspection reports](../../../self-verification-reviews.json)
record the exact run and artifact bindings. Preparation, cold build, warm finite
execution and replay include separate time/RSS measurements. These new workloads
remain unassessed against a baseline.

Fresh full-workspace Rust coverage passes 31,313/32,087 merged source lines
(97.59%) and 2,538/2,615 LLVM functions (97.06%), with unchanged compilation-input
hashes and no new exclusions. Raw LLVM lines/regions remain separate in the
evidence. Final editor packaging validation follows the Zed lock correction; the
earlier whole-product validation report retains its failed packaging status.

The final coupled editor regression/release measurement run is complete and PASS
with 71 workloads. It uses a frozen source copy because concurrent calendar-spec
edits invalidated two broader inventory measurements in the shared checkout.
Its source hash is unchanged before/after checks and measurements; the inventory
is retained in `docs/self-verification-performance-capture.json`. One sample and
one iteration are development evidence, unassessed against the accepted baseline.
