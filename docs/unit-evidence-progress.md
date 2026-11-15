# Per-unit evidence implementation checkpoint

The approved plan is **not complete**. This checkpoint implements foundations,
an early execution safety gate and a working isolated Go decoder workflow. It is
not full Runatron acceptance. Composition-backed task publication is now connected
for the isolated Go decoder workflow; no real Runner task has been reconciled.
The original native runs, reviews and task results have not been rewritten.

## Implemented

- Core-owned, independently versioned unit bindings, native completion receipts,
  supplementary origin proofs, attempts, execution plans and evidence compositions.
  SDK and transport versions are unchanged.
- A pure planner validates exact, disjoint coverage and checks current unit
  identities. A candidate must match the latest passing attempt generation;
  a newer interrupted/failed attempt cannot fall back to old green evidence.
  Explicit source restrictions do not restrict contradiction checking.
- Logical catalog projection keeps contract acceptance/rejection corpora together,
  whole operations and scenarios atomic, and lifecycle paths complete. Unknown
  technical boundaries merge into their existing cohort. Syntax locations are
  removed from logical descriptions without stripping record data named `span`.
- Semantic closure follows references and applicable governing facts, without
  traversing container children into unrelated siblings. Review declarations are
  outside technical evidence identity.
- Optional target boundary analysis returns an explicit unknown boundary by
  default. No existing backend is advertised as proving fine isolation.
- Schema 8 adds indexed bindings, dependency consumers, monotonic attempts,
  origins, composition cases and source retention holds. Schema-7 writers migrate
  additively; read-only access does not migrate. Older storage-conversion paths
  remain available. Held/composed native sources and their ancestry survive GC.
- Origins are revalidated from original native bodies and hash-checked linked
  artifacts. Positive unit evidence requires the original binding and native
  completion artifacts, not just matching case IDs or authored booleans.
- Technical compositions have their own storage and explicit native sources.
  Recording one grants no task-delivery authority. Publication rechecks latest
  generations transactionally and preserves native observations on conflict.
- Adapter, application-test and hybrid entry points perform static preflight
  before native startup. A cold/no-reuse full scope stops with
  `FULL_EXECUTION_REQUIRES_EXPLICIT_OFF`. Hybrid preflight uses the full program,
  rather than treating each backend as an independent allowance. Static capture
  does not execute revision providers. Projection inputs are checked again after
  source capture to reject planning/capture drift.
- `verify incremental-plan` includes the new complete unit-plan projection.
  Unsupported boundaries remain explicitly unbound; the report grants no PASS.
- `verify run --unit-groups PATH --no-adopt` executes a version-1 configuration
  of isolated Go decoder cohorts through the unit planner. It projects all
  mandatory groups together and enforces exact coverage and the full-execution
  gate before acquiring workspaces, building or starting native workers.
- Generated testing adapters bind actual decoder symbols through compiler-checked
  calls and encode their actual returned values. Each cohort has its own process,
  full compiler-resolved package closure and initialization. Reflection stays
  within that whole compiled cohort; this is not a function-level call graph.
  Unrecognized standard imports with possible external/process inputs fail closed.
- Fresh cohorts allocate durable attempts before building. Original native runs
  carry their unit binding and cleanup/completion receipts. The store protects
  unit sources transactionally before GC, without adding a global retention hold.
  A fully warm invocation reads bound inputs and original artifacts, publishes
  technical coverage, and starts no child process. Passing from a failed attempt
  requires fresh native evidence; source restrictions cannot hide newer attempts.
- An opt-in Go build path accepts canonical read-only module roots declared by
  `DCTX0_GO_READONLY_MODULE_ROOTS_V1` (a JSON array in the configured child
  environment). It compiles original production sources directly, keeps the
  default rejection of undeclared external replacements, uses readonly module
  resolution, and binds selected compiler inputs plus package discovery in a
  schema-2 Go build receipt. External embedded resources remain unsupported.
  Source/discovery drift is checked after build and at native publication.
  Selected standard package discovery, module-resolution ancestors and Go
  configuration files are bound too. Unit environments explicitly require
  `GOENV=off`, `GOWORK=off`, `GOTOOLCHAIN=local`, and `CGO_ENABLED=0`.
- Composition-backed authored task publication uses ordinary transactional task,
  claim, reservation, profile and review guards. The author explicitly pins the
  reviewed profile bundle. Current native origins, attempt generations, complete
  case coverage and source bindings are revalidated inside publication. Reports
  identify their composition explicitly and do not invent a global native run.
- Status recognizes these authored composition reports and checks current source,
  configuration and profile bindings without starting workers or providers. This
  is task-report freshness for the supported decoder path, not effective whole
  Runatron coverage or a completed default-command integration.
- The Go decoder analyzer rejects ambient fmt input/output and opaque fmt aliases;
  its analyzer version is now 2. Importing fmt alone is no longer treated as proof
  that all selected calls are pure. A real-production wrapper using aliased
  `fmt.Scan` is rejected before executing its decoder.

## Validation scope

Only release compilation and non-writing format checks are run in dctx0ctl.
Rust tests, selftests, coverage, Cinema, benchmarks and release acceptance suites
are deliberately not run, as required by the approved plan. Native probes use an
isolated new SQLite database and real Runatron production decoders. They do not
copy the production repository or its large database, change its reviewed spec,
or execute the complete Runatron finite plan.

The isolated fixture retains the original generated-case, recursion, value,
evaluation, cleanup and run budgets. It invokes `contracts.DecodeCounter` and
`runtimepolicy.Decode`, as well as the four actual scalar decoders required by
their unchanged type closure. Counters sit at those actual call boundaries;
worker initialization is counted independently. Failed setup/native attempts
remain in the fixture history. Machine-readable measurements and final source
pins are recorded in `unit-evidence-validation.json`.

## Outstanding implementation and mandatory evidence

1. Ordinary native execution without `--unit-groups` still uses the legacy global compatibility path
   after the new static preflight. The new planner does not yet replace every
   decision in run/complete/application-tests/hybrid/continuation. Hybrid live
   validation still follows application execution; the static gate is only a
   conservative preliminary decision, not the completed union planner.
2. General operation, observer and lifecycle roots, and Bun dependency/loader
   integration remain open. Fine Go support is limited to the compiler-checked
   isolated decoder workflow. Other Go/target paths retain conservative groups;
   the generic static projection still reports its unknown boundary.
3. The new attempt/binding/completion integration is active for isolated decoder
   cohorts. Legacy, hybrid, replay and continuation paths do not yet all use it.
4. Warm decoder cohorts start no build/catalog/worker/fixture processes; the
   legacy path still builds and starts its worker. Lazy stateful fixture
   acquisition, versioned logical fixture references and source-bound cached
   catalogs/build recipes remain unimplemented.
5. Historical source-bound migration and regular Runatron reconciliation remain
   open. All 1,939 compiler-selected inputs of original native run a5277222 have
   now been reconstructed with their exact saved SHA-256, including vendor
   metadata and generated worker subjects. This is source reconstruction only:
   historical per-unit boundaries, relevant inputs, completion and cleanup must
   still be proved before adoption. Current task/profile/claim/review guards and
   task-report status are connected for decoder compositions only. Existing task
   counts and diagnostic case totals do not establish whole-project acceptance.
6. Native-origin validation still loads the selected full source run rather than
   only selected case bodies. This and source-hold retirement need further work
   before claiming the plan's resource behavior.

The final-source native probe has 701/701 PASS, a warm run with 701 reused,
zero fresh cases and zero sampled child processes, and a RuntimePolicy-only
change with 585 fresh cases and 116 unchanged Counter cases reused. A new shared
package file stops auto before subprocess startup. Killing the owned Policy
worker records UNKNOWN and blocks older green evidence; the next run repairs
only Policy, followed by another fully warm run. Partial observations and missing
failure-audit telemetry are reported as partial/unknown, never zero or full PASS.

No full Runatron rerun is authorized by this checkpoint. Missing cleanup,
isolation or historical inputs must remain specific evidence gaps; they must not
be bypassed through imported PASS, changed requirements or manual task updates.

## Composition publication checkpoint

The installed CLI SHA-256 is
`77584b135751e3ae3a737d9568a1cfe83f342ae4f25539d467990d1e2d1e4cb4`.
The final isolated database is `composition-final-publication.sqlite3` under the
fixture directory. Its fresh native probe passed 701 cases; the following warm
probe reused all 701 with zero fresh cases or worker starts. Authored Counter
task T2 then published through the guarded CLI in 0.179 seconds with no change to
the actual production-call counter. Read-only status confirms one Done/PASS task
with current authored evidence. These are fixture tasks, not Runner tasks.

Separate retained probes reject an active conflicting claim, a wrong authored
profile pin and a wrong inspection-plan hash. Changing the fixture profile makes
the authored task stale; restoring the exact bytes restores current status.
Earlier probe results retain their original CLI identity and are not relabeled
as final-source runs. Release build, non-writing formatting and diff checks pass;
the approved exclusions on dctx0ctl test suites remain in force.

Real Runner acceptance remains incomplete: 171 implementation tasks are
Done/Unverified and 464 remain Open, with no authoritative current composition.
Go coverage is 81.24% against required 100%. The latest independent performance
comparison passes all 141 unchanged workloads. The 37,945 diagnostic PASS case IDs and exact
historical build reconstruction are inputs to the missing migration, not adoption
authority. No full Runner re-execution was performed or requested.

Work stopped at the user's explicit request to commit and push. Owned unstarted
reservation 208 was released; no task remains in progress. Implementation
completion used ordinary guarded CLI claims/completions and did not set testing
PASS. The Runatron repository records the portable checkpoint in
`docs/runner-acceptance-checkpoint.json`; full native artifacts and task history
remain local. Remaining migration, generic command integration and mandatory
acceptance are not represented as complete.
