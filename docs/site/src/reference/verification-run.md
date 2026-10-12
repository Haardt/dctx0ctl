# Managed native verification runs

During implementation, run `python3 scripts/validate.py --focus verification
--skip-performance` to collect functional failures without release benchmarks.
The check report is stored in `target/validation-without-performance.json`, with
performance explicitly deferred; it does not enter benchmark history or certify
the performance gate. Final performance assessment remains a separate closing
step using the ordinary validation runner.

The central testing configuration is `.dctx0/testing/verification.json` under
the application root. Its `budgets.generated_cases` selects the bounded generated
corpus (default 100). Setting it to 1 provides a small development corpus; it
does not remove explicit mandatory examples or establish the same coverage as
the default plan. The plan and saved run retain the selected budgets. Cinema's
`typescript/config.json` separately configures its project adapter.

Assessment version 10 validates every actually delivered permitted collaborator
argument/outcome and event payload against its pinned contract. A matching call
count alone does not validate a collaborator result. A consumer's declared early
failure does not waive the type of an event it actually emitted. Missing
observations/signatures remain UNKNOWN; invalid values or unexpected exceptions
are FAIL. Explicit empty event constructors retain their empty record shape;
missing nonempty payload models are not invented. Nested lifecycle shrinking
preserves keys inside options, variants, records and lists while reducing
unrelated arguments. Retained acceptance (historical evidence removed during repository cleanup)
includes the whole Cinema corpus, guarded adoption and actual replay.

Automatic adoption evaluates complete evidence for each pinned task. A whole
run may remain UNKNOWN solely for explicit manual requirements or named suites
while unrelated, fully mapped tasks are delivered. Every mandatory native case
must still be present and PASS, with complete provenance. The stored case
manifest must account for every open obligation. A runtime-resolved state
precondition is re-evaluated from the immutable plan, pinned snapshot and actual
before-state observations inside the publication transaction; a supplied
resolution flag or artifact alone cannot discharge it. Missing task requirement
mappings, scheduling prerequisites, claims, reservations and profile changes
continue to prevent delivery. Reviews and integrations remain open.
Root subject selection pins active task subjects below that exact qualified
root; it does not silently select unrelated dependency tasks.
The complete Cinema integration test now checks native adoption and preserves
all other task states, then confirms that replay changes no task events.
Retained evidence (historical evidence removed during repository cleanup) records its scope.

Contract replay requires the saved `expect_rejection` binding to be an explicit
boolean. Missing or malformed markers are rejected before native execution,
alongside inputs that do not match the pinned corpus.
The contract case ID must select that exact value at its original corpus index;
switching valid/rejection cases or substituting another valid index is rejected.
Example IDs and assertion bindings are also checked against their saved plan.

State journals now retain the phase supplied by the actual observer call.
Preparation may produce consecutive Before observations; phases must not be
inferred by alternating entries. The optional journal field preserves readability
of older artifacts, whose omitted phase remains unknown. Fresh reports with an
unphased observation retain an open obligation instead of inventing a phase.
Older stored reports are unchanged; their alternating phase labels are not
evidence of the correct preparation/transition boundary.
Fresh state entries also carry the optional `phase_recorded: true` report field.
Missing metadata in older reports remains unspecified and is omitted when those
reports are serialized again. Markdown reports count recorded and unspecified
phase provenance separately; this adds no task completion or verification claim.

Replay is available with
`dctx0 verify replay <spec-root> <run-id> --case <case-id> [--project <app-root>]`.
It loads the saved content-addressed plan and recorded bindings/setup, captures
the current implementation anew and stores a new historical-requirement run.
It does not require current spec/example bodies to match the old occurrence and
never adopts tasks. The comparison names the original/new run and case IDs,
selected-case outcome, and changed implementation/dependency/adapter/configuration/
harness/runtime/interpreter/generator inputs. Generated value/operation case indices
may be reindexed in the one-case plan; the comparison preserves the original ID.
No prior execution time or PASS is reused. Missing native execution remains
explicitly absent/UNKNOWN; replay of one case never asserts whole-application or
whole-task coverage. A negative-only replay may keep a broader empty-valid-corpus
UNKNOWN while reporting the actual selected rejection-case result separately.
All-kind replay selection and missing-evidence guards are covered by conformance
tests. Pure and lifecycle shrinking use actual native sessions, and minimized
replay preserves required setup. Dedicated replay performance workloads remain
deferred; full workspace/coverage acceptance remains outstanding.
Retained replay evidence (historical evidence removed during repository cleanup) includes focused
acceptance and stable-source 90-workload release measurements. It does not replace
full workspace/feature/coverage acceptance.

Operation-clause continuation: reviewed `ProjectionFixtures.operations` entries
are pinned into each `OperationPlan.cases`. Each case has a unique ID, an exact
declared invocation and reviewed real setup calls. The native session records
actual before/after states, calls and events; Rust checks argument/result/failure
contracts and typed `requires`/`ensures` expressions. False preconditions prevent
execution and remain UNKNOWN. Unexpected native exceptions or violated clauses
are FAIL; missing observers and untyped effects are UNKNOWN. Reviewed cases do
not replace unresolved generated operation contexts or close their requirements.
Legacy verification JSON defaults the added case collections to empty.

Cinema includes real reviewed cases for castVote/closeVoting, covering success
and already-closed polls, plus additional real port and service scenarios.
Typed pre/postconditions, state, resource, call and event obligations are centrally
assessed. Missing projections, native value access and unmet setup preconditions
remain UNKNOWN. These cases are not a claim of whole-operation PASS.
Retained operation evidence (historical evidence removed during repository cleanup) includes
the successful focused tests, stable-source 90-workload release measurement and
actual Cinema run. Full workspace/feature/coverage acceptance is still required.

The CLI now executes selected published requirements through the generated Bun
harness and stores a schema-1 run in schema-4 SQLite history. This is an executable
workflow in development, with guarded native adoption, structural discovery,
shrinking and replay implemented. Complete Cinema evidence and the full coverage
gate remain outstanding; partial native evidence never completes a task.

```sh
dctx0 check examples/cinema/dctx0
dctx0 verify init examples/cinema/dctx0 --project examples/cinema --target typescript
dctx0 verify plan examples/cinema/dctx0
dctx0 verify run examples/cinema/dctx0 --no-adopt --json
dctx0 verify show examples/cinema/dctx0 <run-id> --json
```

The committed Cinema testing configuration explicitly selects
`HomeCinema.MovieNight`. Other applications start with empty scope. Explicit
`--subject`/`--task` selections override configured scope. `--project` may be
supplied directly when no saved association exists. The run never publishes a
compiler head; changed specification files require a successful `check` first.
An explicit `--historical-check` uses that successful occurrence's requirements.
Markdown run/show reports include deterministic per-subject PASS/FAIL/UNKNOWN
case counts, so missing contract access and failed implementation areas can be
located without reading every individual case. These counts retain all subjects
and do not themselves certify complete subject coverage or adoption eligibility.
JSON continues to expose the unchanged structured run and individual evidence.

Stored reports never claim later working-tree freshness. `run` returns zero for
whole-plan PASS and nonzero for FAIL/UNKNOWN while retaining the report and ID.
`replay` returns zero for the explicitly selected case's PASS, without a whole
application claim. `show` returns zero when the historical report was read
successfully, including stored FAIL and UNKNOWN reports.

The engine creates an owned temporary application tree before native execution.
Files are captured in sorted order, with limits of 20,000 files, 16 MiB per file
and 512 MiB total. Symlinks and other nonregular entries are rejected. Git,
historical runatron/state directories, `target` and `.bin` directories are omitted;
this is not a complete dependency/build analysis. Capture failures execute
nothing. Generated worker/contract files and the pinned data/operation model are
written only into the captured testing directory; adapter collisions, escaping
artifact paths, duplicates and incompatible target/runtime versions are rejected.
Production and reviewed adapters in the original application are preserved.

Rust evaluates actual factory results, pure operation signatures, assertions,
ordered examples and lifecycle observations. A worker success flag is never a
verdict. Contract plans contain generated valid inputs and separately versioned
canonical rejection probes. Rust establishes that each rejection input violates
the pinned contract. The actual factory must reject it: native acceptance produces
FAIL, unavailability UNKNOWN. Probes cover wrong wire shapes, UUID text, integer
and decimal range violations, record fields, options, lists and enum alternatives.
Numeric offsets use the interpreter's exact arithmetic. These are bounded probes,
not exhaustive invalid domains. Pure operation generation currently
supports operations without extra clauses or declared failures. Other operations
retain mandatory open obligations, even when their examples pass. An empty plan,
empty valid corpus, unavailable factory/observer, aborted worker or missing case
cannot produce complete PASS. Budgets and bounded corpora remain in the report.

The Cinema adapter connects FilmId/ReleaseYear to their actual production
factories. It uses declared mock collaborators for examples and a fresh actual
Awilix/Hono/SQLite session for reviewed lifecycle case IDs. Its testing-only
`config.json` selects these sessions explicitly; no URL or production architecture
is inferred. `lifecycles.json` supplies reviewed keys, real setup invocations and
ordered transition arguments. Runtime observations and delivered mock replies are
recorded; a mock consumer case does not prove its production port adapter.

Runs bind the snapshot/check occurrence, plan, captured source/dependency hashes,
adapter/configuration, generated harness/model, versions, budgets and native
runtime. Session prefixes survive native errors, including actual values,
observed states and mock delivery. Typed example/lifecycle assessments and final
native metrics are retained as content-addressed artifacts. Task/profile pins and
dependency closure analysis are explicitly incomplete; current native origin is
not adoption authority. `--no-adopt` records evidence without task changes. The
default currently reports `ADOPTION_UNAVAILABLE`; it also changes no task state.

The existing Linux worker deadlines and protocol/log limits apply. Separate
processes are not a security sandbox. Library cancellation terminates managed
workers; CLI signal handling, source-mutation auditing and explicit capture cleanup
failure reporting still need completion. The existing validation runner retains
paired release speed/RSS measurements and now hashes the reviewed testing JSON
inputs too. Dedicated whole-run Rust performance workloads remain required.

This is an additive verification-plan JSON change: `ContractPlan` now contains
`rejection_corpus` and `rejection_generator_version` (`dctx0-rejections-1`). Legacy
schema-1 plans remain readable, but missing rejection obligations produce explicit
UNKNOWN and require reprojection. Snapshot IR/goldens and immutable task/report
definitions are unchanged. Native case bindings retain the original value and
whether rejection was required; unexpected acceptance is assessed by Rust.
The focused acceptance includes an independent exact-arithmetic/reference corpus
and a real production mutation that removes range enforcement while valid inputs
still pass. FilmId/ReleaseYear reject all 30 selected native negative cases.
Retained rejection evidence (historical evidence removed during repository cleanup) includes
the stable-source, complete 90-workload development measurement. This does not
replace full workspace/coverage acceptance or establish task-adoption authority.

Current functional acceptance executes the complete two-file Cinema projection,
both real value factories, 100 generated pure operation calls, 101 assertions,
all 12 examples and both lifecycle paths. The overall report is UNKNOWN because
remaining obligations are open. Compiler acceptance for both complete examples
and immutable task/history behavior remain unchanged. The last complete workspace
run passed functional tests but failed coverage at 99.41% merged lines and 99.28%
functions; new error-path tests are being added. Do not treat this checkpoint as
completion of the full Cinema objective or preservation of current 100% coverage.

The subsequent focused verification validation passes on stable source provenance
with 90 complete paired release workloads. Its scope excludes the remaining
workspace/feature/coverage acceptance; performance comparisons use reduced
development sampling and remain unassessed. The earlier full coverage failure,
new focused measurements and actual Cinema report are preserved together in
retained development evidence (historical evidence removed during repository cleanup). The additional
negative tests have not yet been measured by a fresh complete coverage run.

State assessment now checks actual entity observations for read preservation,
complete-source insertion/upsert and keyed removal. Unrelated rows must survive;
direct entity/Option/List results must match the modeled state. A removed entity
result comes from the before-state, rather than searching the after-state.
Absent observers, duplicate declared identities, unmodeled replacement sources
and distinct nominal result projections remain UNKNOWN. Failed operations do not
implicitly acquire successful state effects or invented rollback requirements.

Cinema's reviewed operation fixtures include two real FilmCatalogStore.save
insert/update cases in addition to four castVote/closeVoting cases. Six cases
retain actual calls and state observations, while missing obligations remain
explicit. These bounded cases do not replace generated operation obligations.
The stable focused validation and 90 paired release workloads are retained in
the measurement record (historical reference removed during repository cleanup).
Validation passes; reduced sampling leaves performance unassessed. This is not
full workspace/coverage acceptance, automatic adoption, or complete Cinema PASS.

Modeled emit clauses now evaluate their typed payloads centrally and compare them
with actual observed events. Missing observers are UNKNOWN; omitted or different
successful emissions are FAIL. Call effects validate actual collaborator
arguments and returned/declared-failure values against pinned signatures. A call
declaration is permission, not an invented exact count; explicit trace clauses
still provide cardinality requirements. An `as` alias uses one actual, valid
collaborator result; absent or repeated observations are UNKNOWN, never guessed.
This does not prove undeclared-effect absence or add resource-access observers.

Focused verification tests and 90 release workloads pass with identical before/
after source hashes in the effects measurement record (historical reference removed during repository cleanup).
Reduced sampling leaves baseline comparisons unassessed. Full effect-boundary
coverage, resource-access evidence, generated nonpure cases, guarded adoption,
discovery, shrinking and full workspace/100% coverage acceptance remain pending.

`verify discover <spec-root> --project <app-root>` now inspects a read-only bounded
TypeScript/TSX source inventory using Tree-sitter. The engine reads files and the
target receives source text through the separate `ImplementationInspector` SDK
contract; no target filesystem or process effects are introduced. Reports retain
source hashes, imports/exports, top-level declarations and public class members,
with byte offsets and syntax-error flags. Malformed sources cannot suggest
candidates. Exact name matches are unconfirmed even when unique; ambiguity stays
visible and no mapping, adapter, spec, history or task is changed. Export aliases
and dynamic relationships are retained as syntax, not semantically resolved.

Inventory limits are 10,000 source files, 4 MiB per source, 128 MiB total source,
100,000 visited directory entries and depth 64. Symlinks and nonregular sources
are rejected; dependencies, testing artifacts and conventional generated output
are excluded. This inventory is not complete production dependency provenance.
The separate `verification-discover` release phase measures each example's actual
application inventory/parse with an application-source hash. It uses a temporary
history database and performs compiler publication before timing; process RSS
includes setup. New workloads remain unassessed until an authorized baseline.

### Task pins and atomic delivery groundwork (2026-10-10)

Native runs selected through tasks now capture the active task plan ID, task
requirements hashes and event watermark before application execution. Available
complete editable profile selections are bound by bundle hash; absent or
incomplete profiles remain explicit adoption provenance issues. Package, Bun lock
and TypeScript configuration files captured for execution are listed separately
as build configuration inputs. Subject-only runs retain no task pin.

`dctx0-tasks::propose_adoption` is a pure eligibility query. It requires exact
nonempty mandatory case coverage, PASS outcomes, complete requirement closure,
implementation artifacts, representation choices and a matching profile pin.
It preserves historical, ongoing and reserved work, excludes unsupported task
kinds and simulates delivered producers only to evaluate scheduling readiness.
A proposal is not execution authority and changes no history.

The store also provides an explicitly authorized atomic result-delivery batch.
It reuses the ordinary start/complete/check guards and profile binding events,
checks the current plan/check/event pins, and rolls back all earlier deliveries
if a later step fails. Rollback concerns SQLite task states, events, results,
check evidence and profile bindings only; production code, adapters and spec
files are never rewritten or reverted by this transaction. This general
in-process API does not authenticate a native run. Imported reports and self-declared `Native` origins still cannot trigger
automatic adoption. The engine now validates native evidence and freshness before
publishing transactional run-to-task links and idempotent adoption receipts.
The bounded native contract fixture demonstrates this path; incomplete Cinema
evidence does not authorize whole-Cinema adoption.

The task-pin checkpoint passed focused verification, task and store tests with
92 paired release workloads in
`benchmarks/history/20261010T093650.158088Z-934996.json`. Single-sample development
measurements remain unassessed against the accepted baseline. This checkpoint
contains no fresh full-workspace coverage result and is not full Cinema acceptance.

The explicit store publication API can now associate an authorized delivery
batch with an immutable run. It checks matching pins, mandatory case coverage,
PASS outcomes, no open obligations/provenance gaps and scoped implementation
artifacts; it stores task links and a hashed publication receipt in the same
transaction as lifecycle events. Identical requests return the original receipt
without new events; a changed request for that run is rejected. Link/receipt write
failures roll back task changes while preserving the separately recorded run.
These are storage guards, not authentication of observed execution. CLI imports
cannot invoke adoption; the native engine uses this API after its independent
execution, freshness and obligation-to-task checks.

Before recording native evidence, the engine now inventories the original
application again under the project publication lock and compares it with the
pre-execution capture. Edits, added/removed files and changed captured dependencies
produce `APPLICATION_INPUTS_CHANGED`; unreadable/uncapturable current inputs
produce `APPLICATION_INPUTS_UNAVAILABLE`. Both remain explicit adoption provenance
gaps. The report continues to describe the captured implementation actually
executed, and current files are never reverted. Inventory reads enforce the same
file/count/byte limits as capture and avoid copying production files. This check
does not establish dependency completeness or prevent arbitrary external editors
from changing files after publication; native adoption validation remains pending.

Mandatory case inventory now belongs to the pure Rust verification kernel.
`case_manifest` deterministically projects contracts (positive and rejection
cases), generated/reviewed operation cases, assertions, examples and lifecycle
paths, retaining corpus/version/unsupported/manual/suite issues. The engine uses
that inventory and saves it as `application/vnd.dctx0.case-manifest+json`.
Operation cases link the modeled operation and its clause IDs; assertion cases
link their assertion and property IDs. These links describe planned scope, not
observed success. Duplicate mandatory IDs are explicitly rejected before native
execution instead of silently reducing the plan.

`dctx0-tasks::task_case_coverage` purely maps requested requirement IDs to every
associated mandatory case. Missing mappings, missing executions and UNKNOWN
remain incomplete; relevant FAIL dominates incompleteness. Unrelated passing or
failing cases do not establish the selected task's outcome. This query does not
mutate tasks or authenticate supplied outcomes. Native task proof construction
and automatic publication additionally check provenance, profiles, scheduling and
each task's full requirement closure.

Contract plans now add an optional `refinement_requirements` field binding the
source node IDs for `range`, `length`, `format` and `where` clauses actually
visited by the Rust value checker. The mandatory positive/rejection value cases
map to those IDs as well as the contract subject. Planning these links does not
prove that the native factory executed successfully or that an unsupported
refinement is true. Free-form intent IDs are not included. Historical plans that
omit the field retain empty refinement mapping; loading them never invents new
coverage. This is an additive verification-plan projection change, with no AST,
validated IR, snapshot golden or saved task-definition changes.

Fresh plans additionally retain optional `structural_requirements` for record
field declarations. The central checker enforces exact field names/counts and
each declared field type/refinement on factory results, so these cases now map
to the actual field requirement IDs. A separately executed native record fixture
checks topological scalar/record delivery and detects a changed production field
type without rewriting historical PASS. This does not assert entity-key
uniqueness from a single value check. Entity keys, architecture notes and manual
requirements are not added to these value-case mappings. Historical plans that
omit `structural_requirements` retain no invented field coverage. Assessment
version 9 records this additive proof-mapping change; mandatory case IDs,
compiler IR and immutable task definitions are unchanged.

Generated operation candidates now pass through the versioned pure Rust
`select_precondition_inputs` projection. Boolean false preconditions classify
candidates outside the valid domain before native execution. Selection reports
examined, accepted, rejected and unresolved candidate counts. Conditions needing
unavailable invocation/state observations remain source-related open issues;
unresolved candidates are retained and no exhaustive valid-domain claim is made.
An empty selected valid corpus is UNKNOWN. Explicit reviewed cases and examples
remain mandatory regardless of generated candidate classification.
`OperationPlan.precondition_selection` is optional for historical readability;
plans without preconditions retain their existing serialization. This does not
expand native support for nonpure operation generation or resource observers.


### Pending bundled execution changes

The separate implementation batch adds optional typed resource observations. Missing observations remain unknown; an observed empty trace is distinct. The worker records snapshots of delivered event, call and resource values and limits the combined recorded trace to 10,000 observations and 1 MiB per invocation. Exceeding either limit is a technical execution failure even if adapter code catches the original exception. These limits supplement the managed-process and JSON-lines limits.

The pending `verify replay <spec-root> <run-id> --case <case-id> --minimized` option selects a saved pure-operation shrink counterexample. It validates the shrink version, original invocation and matching failure signature, then executes the saved minimal invocation against newly captured code. The report identifies minimized input and changed provenance; replay never adopts tasks. Shrink attempts retain actual acknowledged native observations. Stateful/lifecycle shrinking remains unfinished. These changes were transferred from `/tmp/dctx0-verification-batch` after the previous validation finished. Full validation of the combined implementation remains pending.


### Dependency evidence and aggregate failure reporting

The Engine now saves a Tree-sitter dependency report for captured TypeScript/TSX implementation and adapter inputs. It checks the report version, exact source inventory and UTF-8 byte ranges before using references. Exact relative captured files and Engine-generated testing artifacts are distinguished; unresolved expressions, uncaptured paths, unknown package resolution and syntax errors retain explicit provenance issues. Production imports into `.dctx0/testing` are reported. This structural report does not establish semantic call graphs, hermetic execution or complete package dependency authority; automatic adoption remains gated.

The validation runner now executes Cargo tests with `--no-fail-fast`, continues independent stages after failures and retains each command/exit-code pair as `validation_steps` in the measurement record. A failed test run remains failed even if benchmarks finish successfully. Coverage thresholds and baseline promotion rules are unchanged.

Native dependency requests use `resolve-dependency` with explicit `import` or
`require` resolution. A successful reply is `dependency-resolved`; an ordinary
missing module returns `dependency-unresolved` with a reason. This keeps the
worker available for mandatory application cases while retaining incomplete
dependency provenance. Malformed requests and transport failures remain
technical failures. Resolution itself must not execute the selected module.

The Linux managing process also captures a runtime-identity artifact directly
from the live worker's `/proc/<pid>/exe` and `/proc/<pid>/environ`. It streams an
exact executable hash (limit 256 MiB) and hashes the initial environment (limit
1 MiB); it does not persist environment names or values. Replay reports a
changed or newly available identity separately from the runtime version string.
Unavailable identity capture leaves incomplete provenance. This is not a
sandbox or a claim that arbitrary application access to external resources is
captured. Actual native runtime-identity tests cover this additional evidence.

Reviewed operation cases can supply typed `mocks` alongside their actual
invocation and setup calls. Rust validates prepared collaborator arguments and
outcomes and matches actual callback requests centrally. Repeated matching calls
do not imply an invented exact call count. Unexpected arguments are a contract
failure; an ensuing rejected adapter promise does not erase that observed
failure. The acknowledged session prefix and cleanup failure remain evidence.
Prepared mock scenarios supplement generated nonpure invocation cases; missing
collaborator plans remain explicit rather than invented.

Generated invocation cases now also cover nonpure operations whose clauses use
the implemented requires/ensures, state, read/write/observe, call and emit forms.
Each starts a fresh session and uses the same central operation assessor as
reviewed cases, including declared failure results. Missing mock plans or state
prerequisites are UNKNOWN; they are not manufactured or silently discarded.
Cinema explicitly routes its generated operation cases to real persistence.
Pure-only shrinking remains separate; minimized replay rejects nonpure cases.
These newly expanded generated cases await runtime validation.

Shrinking now validates the original invocation's type and precondition domain
before proposing candidates. Invalid originals retain their structured issue
and cannot be converted into apparently valid counterexamples. Added reference
cases cover negative integers, exact decimal truncation, finite float halving,
Unicode scalar boundaries, unchanged record siblings, list element removal and
shrinking, option payloads and preserved variant alternatives. These additions
are compiler-checked; aggregate runtime verification is pending.

The unconditional `ADOPTION_PROVENANCE_PENDING` marker has been replaced by
actual incomplete-input checks. Native execution with resolved captured source
imports can now reach automatic task publication when all mandatory cases pass,
the task/profile pins match, and the live worker identity was captured after an
acknowledged request. Publication rehashes that executable and the current launch
environment; changed or unavailable identity prevents adoption. Package closure,
unresolved dynamic dependencies and unknown resource access remain explicit
blocking issues. Initial environment hashing sorts complete entries, preserving
their values without persisting them. A native adoption regression case checks
Open→Done/PASS, repeated verification without new task events, and preservation
of historical PASS after changed code fails. Runtime validation of this adoption
path is pending.

Publication also rechecks standard-run spec source hashes after native execution.
A concurrent edit leaves the executed pinned-snapshot evidence intact but blocks
current task adoption. Explicit historical checks and replay always suppress
automatic adoption, including an explicitly historical selection of the current
check occurrence. Base-spec cases cannot automatically complete profile
adaptations or check/review follow-ups whose additional assignment has no
dedicated evidence. These guards preserve task definitions and append-only
history; they never undo source edits.

Operation and lifecycle assessment artifacts now include their mandatory
`case_id` alongside the assessment. Distinct cases with identical outcomes
therefore retain distinct evidence without violating the store's unique artifact
references. Generated replay validates the original case index, parameter
bindings, actual argument vector and empty generated setup against the saved
plan before execution.

The aggregate run `20261010T115642.301546Z-1180670.json` passed the central mock
tests and the store's missing/duplicate/corrupt mandatory-manifest rejection
cases. It failed two Engine integration tests and the concurrent Go target's
Clippy check. Worker error-path corrections made after that run pass formatting
and compiler checks; their runtime verification is pending. The recorded release
measurements belong to the preceding source state, not those corrections.

The whole-Cinema acceptance test now replays saved assertion, example, reviewed
real operation, generated nonpure operation and lifecycle inputs against fresh
sessions. It compares each selected case result with the original, requires one
case in the replay report and verifies no task adoption/event changes. An
assertion absent from its stored plan now fails replay selection explicitly.
These additional replay cases are compiler-checked and await runtime validation.

### Lifecycle counterexample replay (2026-10-10)

The central shrinker proposes typed lifecycle inputs while retaining the pinned
transition path and entity keys. Each proposed input runs in a fresh native
session. Acceptance requires the same failure code and source location, the same
completed transition count and reached-state prefix. Removing setup is a proposal,
not proof that setup was unnecessary. Native attempts and their assessments are
retained in a versioned lifecycle-shrink artifact. `verify replay --minimized`
accepts this artifact only for its recorded failing case and executes the saved
input against freshly captured application code without task adoption.

The focused validation and paired release measurements in
`benchmarks/history/20261010T140327.322289Z-1288163.json` passed. The production-mutation lifecycle replay acceptance test also passes in
`benchmarks/history/20261010T140835.466152Z-1295153.json`.
Additional mandatory-setup preservation and missing-path rejection checks are
pending the next run.
This does not supersede the remaining full-Cinema and 100% coverage gates.

The mandatory-setup test exposed an invocation-access gap: a reviewed lifecycle
could need an actual setup operation outside the selected verification closure.
Harness generation now includes declared setup operations from selected reviewed
operation cases and selected lifecycle fixtures. Unselected lifecycle fixture
entries grant no access. This does not expand selected/required subjects or claim
verification of setup operations. The real lifecycle test now preserves necessary
setup while minimizing a separate argument, reproduces the original failure and
passes against repaired production code. The missing-path guard is also tested.
The failed aggregate remains recorded in
`benchmarks/history/20261010T141339.816392Z-1302224.json`; the focused release
regression test passes after the fix. Full workspace/coverage validation follows.

### Execution-copy freshness and validation binding

After shutdown, the engine compares the captured execution tree with its
pre-execution inventory, including generated harness overlays. Changed or
unreadable captured sources, adapters, harnesses and dependencies block automatic
adoption while the actual observations remain saved. The original application
tree is independently checked. This is an optimistic freshness check; native
execution remains a managed process, not a security sandbox.

The validation runner now hashes checked inputs before and after all checks and
binds that hash to the release measurement's source hash. A changing source tree
cannot become successful validation evidence. Both full example specifications,
reference fixtures and native Go helper sources are included. Benchmark input
hashes discover spec sources recursively. Existing baselines are retained;
changed input identities may require an explicitly reviewed new comparison.

The preceding complete workspace run is retained in
`benchmarks/history/20261010T144129.684573Z-1308216.json`: functional tests passed,
while coverage failed at 20,738/21,283 merged source lines (97.44%) and 97.43% LLVM
functions. It predates these additional freshness and replay guards.

Native helper coverage is now collected separately by the Bun transport
conformance fixture: actual generated worker variants run inside Bun's coverage
runner, retain their protocol behavior and export raw LCOV plus source hashes,
line/function counts and uncovered lines. Native coverage-instrumented timing
samples are explicitly labeled; they are not release benchmark evidence and are
never merged into Rust/LLVM coverage. This implementation awaits its next
aggregate validation. Go and other target helper coverage is outside this scope.

The combined copy-freshness/replay/native-helper implementation passes focused
verification validation and paired release measurements in
`benchmarks/history/20261010T150010.334286Z-1342894.json`.
Raw native helper coverage (historical evidence removed during repository cleanup) retains 15 actual
worker executions, independent source hashes and LCOV. Bun function unions remain
unavailable as documented in [coverage limits](coverage.md). Reduced release
sampling remains unassessed against the accepted baseline; the baseline has not
been promoted. The previous failed aggregate runs remain in measurement history.

### Declared early failures and call aliases

A declared failure may bypass a success-only collaborator. For an observed empty
call set on such a failure, the alias remains unbound; a short-circuited success
condition does not need its value. An unconditional condition using that missing
alias remains UNKNOWN. Explicit required call counts still fail if not delivered;
missing aliases on successful outcomes remain UNKNOWN. No call/result is invented.

The early-failure increment introduced an independent version artifact,
`dctx0-assessment-1`; the state-projection increment uses version 2, and the
current mock-observation/resource-label assessment uses version 3.
Replay reports changes in that artifact. This is distinct
from corpus generation and expression interpretation versions. The focused test
and paired release run passes in
`benchmarks/history/20261010T151436.349735Z-1349024.json`.
Complete Cinema recheck (historical evidence removed during repository cleanup) executes all 5,543
mandatory cases, with 1,037 PASS, 579 FAIL and 3,927 UNKNOWN. It removes all 98
premature alias UNKNOWNs without changing the spec or production implementation.
No tasks are adopted. This still does not satisfy full Cinema/coverage acceptance.

### Independent explicit state-result projections

Central assessment version `dctx0-assessment-2` supports one complete explicit
result equality, optionally guarded by `success`, evaluated against actual
before/after snapshots. It evaluates the source without the actual `result`
binding: a circular equality such as `result == result` cannot prove a result.
The full modeled state effect is still checked first. Wrong projected values
fail; unavailable observations remain UNKNOWN. Multiple equalities retain the
reference's requirement for an unambiguous projection.

This implementation passes focused verification and paired release measurements
in `benchmarks/history/20261010T152435.784689Z-1357181.json`. It adds no missing
Cinema assertions or DTO correspondence: the 242 unmodeled projections remain
open unless their specification supplies executable evidence. The earlier
compile failure remains retained as an incomplete measurement run.

### Rust verifier executable binding

Native runs now retain a versioned verifier-identity artifact read from the
actual Linux `/proc/self/exe` image with streaming SHA-256 and a 256 MiB limit.
This binds the executed Rust interpreter, generator, assessment and orchestration
code independently of their version names and independently of worker claims.
Failure to read this identity prevents automatic adoption. Replay identifies a
changed or previously missing verifier identity as a new comparison. Task
implementation evidence signatures include verifier, native runtime and
assessment identities along with the existing provenance. Old saved evidence is
not rewritten. This does not bind every operating-system/shared-library input or
provide a security sandbox.

The initial implementation passes focused verification and paired measurements
in `benchmarks/history/20261010T153833.016483Z-1367673.json`. Imported legacy/missing
verifier-identity replay tests passed in the subsequent run described below.

Changed and missing verifier executable identities are now independently covered
by actual replay tests, alongside legacy/missing assessment versions. Imported
comparison inputs are never adoption authority. Old reports and task event
sequences stay unchanged. The combined focused validation and release
measurements pass in `benchmarks/history/20261010T154434.885634Z-1373222.json`.

Successful Rust executable identities are cached only within the same managing
process, whose running Linux executable image is fixed. Failed reads remain
retryable. This avoids rehashing a large debug executable for every replay; no
spec inputs, native outcomes or verification verdicts are cached by this change.
Result-projection boundary tests and this cache passed focused validation in
`benchmarks/history/20261010T155006.326205Z-1376553.json`.

State assessment boundary tests also reject incomplete selectors, missing insert
or upsert sources and unavailable entity identities. Deep recursive actual
snapshots remain UNKNOWN when the wire-value budget is exceeded, even when
before and after are identical. The 12 focused state-effect tests pass; these
new tests have not yet been included in a fresh full coverage measurement.

Shrink candidate generation version `dctx0-shrink-2` bounds intermediate list,
record and payload candidates while constructing them, rather than collecting
all copies before truncating. A large explicit list regression retains the
deterministic bounded prefix and honors zero shrink steps. Minimized replay
continues accepting version 1 recorded inputs after checking the original case,
failure and actual accepted shrink attempt. A real-operation regression proves
both old and new artifacts replay the same failing minimized input and detect
the later production repair. No historical evidence is rewritten.

Reviewed operation setup calls now evaluate their own declared preconditions
before invoking production code. Available state observers supply actual before
state; missing required observations remain UNKNOWN. Setup validation checks
input/result contracts and these guards, without claiming full effect coverage
for an unselected setup operation. A real Bun regression confirms an invalid
setup is never invoked, while the valid setup and actual subject both execute.
The preceding 19 native verification-run regressions also pass.

Lifecycle setup uses the same central precondition assessment before each
preparatory invocation, with actual before-state observations when needed.
A false guard ends and cleans up the session without setup or transitions;
the blocked context remains in the report. The native setup/failure/deadline
conformance case and the real Bun lifecycle shrink/replay regression pass with
this change. Setup effects are not inferred or certified by this guard check.

Planning also validates reviewed operation setup declarations, argument counts
and central type contracts before native execution. Invalid setup entries remain
in the mandatory case manifest and produce explicit planning issues; they are
never discarded to obtain a complete result. All 18 projection conformance tests,
including the complete Cinema scope, pass with this validation.

For configured applications, `verify plan` statically reads the adapter's sibling
`lifecycles.json`. Missing path inputs or an incomplete transition-argument list
are explicit `LIFECYCLE_INPUT_REQUIRED` planning issues. The native run applies
the same check to its captured inputs before saving the plan. This inspection
executes neither adapter nor application and changes no SQLite history. Escaping
input paths and files larger than 512 KiB are rejected. Full state-dependent
input readiness remains a native-session check, not a static success claim.

Lifecycle input contracts now also validate the complete declared entity key,
setup operation signatures and every transition argument using the shared Rust
type checker. Invalid configured inputs appear in planning, and invalid keys
are rejected before a native session or setup begins. The native regression
checks that no session evidence, setup calls or transitions are produced for a
wrongly typed key. These checks do not infer the required initial state.

Markdown run/show output begins with recorded PASS/FAIL/UNKNOWN counts and a
deterministic reason-code table, plus required/nonselected scope, seed and generated
case budget. Incomplete provenance is listed separately. All saved case details
remain available in `show`, and JSON remains the complete unchanged report shape.
Recorded outcomes do not imply that an UNKNOWN case invoked application code.

Automatic publication now rejects a FAIL/UNKNOWN run or open required obligations
before preparing any task proposal, matching the store's existing complete-PASS
transaction guard. It reports `ADOPTION_COMPLETE_RUN_REQUIRED`; previous task
results remain unchanged. Individual passed cases inside an incomplete selected
run are not delivery authority.

Native session evidence is bounded while it is retained: defaults are 4 MiB of
canonical journal entries and 10,000 entries per session. Both budgets reset for
each new case. Exceeding either preserves the recorded prefix, terminates and
reaps the worker, and leaves incomplete execution evidence rather than PASS.
These are journal limits, not a claim about total process RSS. The exact native
limits are saved in a schema-1 artifact, included in adoption implementation
signatures, and compared independently during replay. Invalid zero budgets are
rejected before spawning a process. The early adoption guard retains captured
provenance issues and missing profile-pin diagnostics alongside its run rejection.

For prepared operation mocks, adapter-recorded calls are cross-checked against
the actual collaborator requests and replies observed by Rust. Invented, dropped,
duplicated or altered mock calls fail with `MOCK_OBSERVATION_MISMATCH`; absent
call observers remain UNKNOWN. Matching uses occurrences rather than completion
order, so concurrent calls introduce no undeclared ordering assertion. A real
Bun regression rejects an adapter that records a fabricated mock call and returns
the expected constant without requesting the collaborator. Honest consumer calls
and the existing production argument-mutation failure remain covered.

Assessment version 3 also preserves UNKNOWN for observed resource events whose
operation label is unmodeled. Such an event cannot be classified as an undeclared
modeled collaborator. Missing actual call observations retain their separate
unknown reason. Read/write/observe dispatch now supplies a typed access kind;
there is no fallback pretending another clause kind is a resource permission.
The four resource-permission tests cover matching and mismatching access kinds,
missing observations, unknown operation labels and genuine undeclared access.

Assessment version 4 resolves explicit type aliases before deciding whether a
state result is a direct entity, an optional entity or a list of entities. It
checks the actual selected rows and preserves missing/ambiguous cardinality and
unrelated-result failures. It does not infer mappings from separate DTO records
or prose. Alias traversal obeys the recursion budget. Earlier run evidence keeps
its original assessment version; replay reports the changed assessment artifact.

TypeScript dependency analyzer version 2 includes CommonJS member loaders:
`module.require` and `require.resolve`, including literal indexed access and
whitespace variants. Nonliteral arguments remain unresolved. These structural
references are resolved by the actual Bun worker; they are not runtime call-count
evidence. Unrecognized dynamic loader aliases remain outside the supported
static analysis. The native adoption regression exercises a real
CommonJS module loading a second captured module and requires both in the saved
dependency-scope artifact.
Escaping a recognized member loader as a function value produces an unresolved
dynamic-code reference. A passing application call therefore cannot turn that
unresolved loader into complete adoption provenance. This classification does
not claim that the escaped loader was actually invoked or how often.

Human-readable run/show summaries separate recorded case outcomes from recorded
invocation observations (returned, threw, unavailable), and name cases with no
invocation observations. Value checks and central expression assessments have
separate evidence. Imported observations remain imported claims; showing them
does not confer native execution or adoption authority.

The engine now inspects captured package sources as well as application sources
and traverses their actual native import/require resolution. Package manifests
must be captured; dynamically unresolved loaders and unsupported executable
formats stay incomplete. JSON data is bound by its captured content hash.
TypeScript dependency analyzer version 3 keeps uncertain custom `require`
members and `createRequire` factories open, without interpreting ordinary DI
`resolve` calls as module loads. Bun confirms bare Node builtins through its
actual module API and returns canonical runtime module IDs; these are bound to
runtime provenance rather than represented as captured source files.
Canonical IDs preserve both `bun:` and `node:` namespaces and the standalone
`bun` module. Runtime classification is tested for import and require modes;
unknown names in either namespace remain unresolved. Earlier evidence generated
before this correction may contain `node:bun:...` labels; it is retained as
historical output, not silently rewritten.

Assessment version 5 retains explanatory Intent nodes in the plan as context
without treating them as new mandatory assertions. Explicit constraints, reviews,
policies and unsupported executable requirements remain open. Adoption proofs
separate context facts from machine-covered requirements; the transactional store
recognizes only actual Intent nodes from the pinned snapshot for this exception.
Subjects and types cannot be bypassed as context. Task definitions and historical
results remain immutable, and PASS still asserts no free-form intent meaning.

## Bounded ESM dependency observation

Bun harness generator version 2 optionally receives Engine's captured inventory
through `HarnessGenerator::module_observer_artifacts`. The default SDK method
returns no artifacts; other targets retain their existing execution behavior.
Tree-sitter identifies supported dynamic call argument ranges in `.mjs` files.
CommonJS, arbitrary code execution, spreads and overlapping argument rewrites
remain unsupported. The manifest binds exact source SHA-256 hashes and ranges.

Before importing the project adapter, the worker installs a Bun onLoad hook only
for those exact ESM files. It verifies the source hash and wraps arguments without
changing their values. Native dynamic load attempts and loaded sources are
retained for the entire worker run, including testsession cleanup. Bounds are
256 KiB/4,096 trace entries and 4,096 characters per observed specifier. Invalid
values and exhausted traces mark evidence incomplete. Manifest/source failures
are technical errors; they cannot establish adoption evidence.

The additive `observe-dependencies` SDK request returns typed observer evidence.
Rust validates version, completeness, source hashes, loaded-source identity and
site membership. For an actually loaded, fully instrumented source, a site with
zero observed attempts can discharge its dynamic-dependency provenance gap for
this run only. Executed sites remain unknown even when a native resolver supplies
a candidate path: resolution alone does not establish their complete closure.
Unloaded sources also retain gaps. Static dependency inspection is preserved as
its own artifact; no call graph or architecture proof is invented.

Reports store both manifest and observation artifacts. Worker writes use Bun's
stdout sink and await flushing on shutdown; native coverage wrappers await actual
worker completion, rather than merely stdin closure. Tests include a large trace,
changed source hashes, invalid ranges, unsupported argument values, UTF-8 byte
offsets and budget exhaustion.

The dependency analyzer's version 4 also retains escaped custom loader references
such as `const load = dependencies.require`. Direct assignments to loader fields
are writes rather than escaped loader reads. Observer version 2 can instrument
an opaque loader read as a whole-expression site: the native hook returns the
exact original value, and any executed opaque access makes evidence incomplete.
It records no invented loader call or resolved module for that access. A loaded,
source-hash-matched module whose opaque access was never evaluated can discharge
that site's gap for the current execution. Version-1 argument-site manifests
remain readable. The Bun harness generator is version 3; JSON-Lines remains
protocol 1. Seven real native observer scenarios and a native adoption rejection
test cover the added boundary. The full Cinema recheck is recorded separately.

Both pure-operation and lifecycle shrinking require an observed FAIL before
searching for smaller inputs. PASS and UNKNOWN are not counterexamples. A zero
step budget retains the original input with a visible budget issue and executes
no shrink case. A managed protocol-worker test verifies these guards without
supplying application execution evidence; the real failing-operation and
minimized-replay integration test remains passing.
# Session failure regression evidence (2026-10-11)

The real Bun integration test
`real_session_initialization_and_cleanup_failures_retain_attempts_and_cannot_adopt`
injects separate initialization and cleanup exceptions into an external adapter
calling an actual production value factory. Both runs retain the first attempted
case and its session artifact as UNKNOWN, stop subsequent mandatory cases, store
immutable history and preserve the task event sequence. Neither run adopts work.
The focused test and all-target Engine Clippy check pass. Its 6.35-second test
duration is not a release performance measurement; performance remains deferred.

The real Bun test
`profiles_changed_during_native_execution_preserve_evidence_without_partial_delivery`
also changes an actual profile file during factory execution. Separate cases
append text to the common profile and corrupt the profile manifest. Captured
application-input freshness reports `APPLICATION_INPUTS_CHANGED` in both cases:
the successful finite value checks remain stored, while no task events or
partial deliveries are published. The test confirms that the edited files remain
edited; rejecting adoption does not roll back source/profile changes. The focused
test and Engine Clippy check pass; performance measurement remains deferred.

The TypeScript-enabled CLI integration test
`cli_lifecycle_replay_runs_real_sessions_detects_mutations_and_preserves_saved_history`
checks actual session setup and before/after state through `verify run`, then
successful saved-input replay in JSON and Markdown. Mutating the production
transition leaves it in the initial state: replay returns FAIL and identifies the
changed implementation. `verify replay --minimized` reproduces that failure;
the original run and compiler check history remain unchanged. This is a small
CLI conformance fixture supplementing the complete Cinema native acceptance,
not a replacement for it. The test and CLI Clippy check pass. Its test duration
does not certify release performance.

The same CLI fixture now also covers missing lifecycle inputs, an unadvertised
state-observer capability, and an advertised observer that actually throws. The
path stays mandatory in all three saved reports. Missing input/capability leaves
it unexecuted with an explicit obligation; an observer exception retains an
attempted UNKNOWN case and records native abortion. None becomes PASS or adopts
work. After restoring the test adapter and inputs, successful and mutated replay
continue to pass their respective conformance assertions. CLI Clippy also passes.
