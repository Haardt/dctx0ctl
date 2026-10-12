# Go target and Cinema acceptance

The Go target connects the existing language-neutral Rust verification engine to
actual Go applications. It supplies structural Tree-sitter inspection, generated
JSON-Lines transport and a managed native build. It does not interpret the DSL,
generate business logic or decide PASS in the worker.

## Project interface

An application needs a Go module and a testing-only adapter under
`.dctx0/testing/go`. `verify init --target go` preserves existing files and
creates an unavailable scaffold. The adapter implements `ProjectAdapter`, with
fresh `TestSession` instances and optional `ValueValidator`/`StateObserver`
interfaces. Normal Go interfaces connect declared collaborators; production has
no SDK imports. Subjects and implementation artifacts are explicitly described.

`discover --target go` reads Go syntax without running the adapter or compiler.
Imports, exported declarations and method candidates are structural evidence;
ambiguity and syntax errors remain visible. Native execution reads the target
from `.dctx0/testing/verification.json`, including `run`, `replay` and `complete`.
Both TypeScript and Go execution work with their respective Cargo feature; a
missing target feature is an explicit error.

The optional `HarnessBundle.build` is separate from its executable recipe.
Existing TypeScript bundles omit it, retaining their serialized form. SDK
protocol version 1 and profile manifests remain unchanged. Setup obtains the
adapter filename from the target scaffold. Go inspection adds the `go` language
variant; AST, validated compiler IR and Cinema snapshot goldens are unchanged.

## Build and provenance

Dependencies are prepared explicitly with `go mod download`. Managed builds use
`GOTOOLCHAIN=local`, `GOPROXY=off`, `GOWORK=off` and `CGO_ENABLED=0`. Preparation
vendors the selected pinned module dependencies into the captured application,
then `go list -deps -json` identifies the actual selected package sources and
embedded files. Builds use `-trimpath` and that vendor tree.

The build receipt hashes sources, embeds, module manifests, selected Go compiler
tools and the worker binary. Rust independently checks the receipt and actual
binary, and checks inputs again before publication. The worker executable is
retained by content hash under application-owned derived `.dctx0ctl/native/go`
state so its identity remains available for guarded subsequent task evidence.
Generated artifacts and vendoring do not rewrite production sources or the
application's module files.

Builds have a separate 300-second budget and managed logs/cleanup. Worker
initialization, cases, run, message and output limits remain the shared engine
limits. Build errors, unavailable tools, malformed receipts and changed inputs
prevent full evidence. Cancellation/timeouts reap managed children. Linux native
process ownership remains the accepted platform; the optional systemd memory cap
is enabled only when available and can be disabled with `DCTX0_MEMORY_GUARD=0`.
This is resource control, not a security sandbox.

The first supported project shape is one module without CGO. External local
replacements and workspaces are outside this increment. Uncaptured runtime
resources require explicit test configuration and suitable observations; no
static source pattern establishes runtime behavior.

## Cinema

`examples/cinema-go` shares the exact Cinema spec, examples and lifecycle inputs
with the TypeScript application. It uses net/http, ordinary constructor-injected
interfaces, arbitrary-precision integer values and real SQLite via the pinned
CGO-free driver. The adapter runs actual production methods, declared mock
collaborators, real SQL observations and real HTTP requests. Independent Go tests
cover exact integer JSON, Unicode trim differences, winner ties, durable SQLite,
changed votes, HTTP statuses and error propagation.

The native mandatory plan contains 5,674 cases. Free-text constraints, decisions,
reviews and undefined suites remain separate authored inspections; native UNKNOWN
for these requirements is not silently changed to a worker PASS. `verify
complete` binds the source-reviewed notes and requires 104 active tasks with no
unfinished/non-PASS task. Native automatic adoption and inspected delivery are
reported separately.

```sh
go mod download -C examples/cinema-go
dctx0 verify complete examples/cinema-go/dctx0 --project examples/cinema-go
python3 scripts/go_cinema_acceptance.py --binary target/debug/dctx0
```

The product command requires Rust and Go, not Python. The repository acceptance
script additionally uses isolated histories, compares target-neutral plans,
checks actual production mutations and broken/repaired replay. Run the coupled
workspace validation for lint, feature combinations and Rust coverage; release
measurements belong at the end. Coverage remains the user-approved 97% Rust
line/function threshold. Go coverage is separate and is not a branch-completeness
claim.

This increment does not port the 16-step TypeScript tutorial to Go, introduce a
web/DI framework, revive runatron-next, add production generation or claim
exhaustive correctness over all inputs. The accepted benchmark baseline remains unchanged. Historical records remain
subject to the existing latest-20-run retention policy; incomplete runs retain
their explicit failure status.

## Accepted validation, 2026-10-12

The final frozen-source workspace validation passed all 19 grouped commands,
including TypeScript compatibility, Go-only target selection, native Go transport,
Go vet/race checks, lint and the complete instrumented Rust test suite. Merged
Rust source lines are 26,694/27,202 (98.13%); LLVM functions are 2,126/2,183
(97.39%). Both exceed the approved 97% threshold. Raw LLVM metrics are retained
separately; this does not claim complete branches or native Go coverage.

The isolated Cinema acceptance passed all 5,674 mandatory cases and completed
104 tasks: 46 through native automatic adoption and 58 through explicitly
source-bound inspections. The 25 manual/suite obligations remain UNKNOWN in the
native report. The complete command combines that report with authored reviews;
it does not turn prose into automatic assertions. All five production mutations
(winner, persistence, lifecycle, HTTP wiring and collaborator calls) were detected;
broken and repaired replay produced FAIL and PASS respectively. Both applications
have byte-identical specs and the same semantic verification plan.

Run the same grouped validation with performance at the end:

```sh
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 python3 scripts/validate.py \
  --coverage --example cinema --samples 1 --iterations 1
```

The retained run contains 56 release workloads, including ten Go phases. Full
native execution including managed build took 9.05 seconds for the first project
build and 9.52 seconds for the repeated build. Worker peak RSS was 23.35 and
23.00 MB respectively; the separately measured compiler process peaked around
258–260 MB. Complete verification plus adoption took 11.38 seconds. These are
single samples with existing module/toolchain caches, not cold-cache guarantees.
The process RSS metric is the largest reaped process, not summed simultaneous
memory. New workloads and these reduced sampling settings are unassessed against
the accepted baseline; no baseline was promoted.

Retained acceptance, coverage and measurement evidence (historical evidence is available in Git history)
includes unchanged before/after source hashes and points to the full benchmark
history record.
