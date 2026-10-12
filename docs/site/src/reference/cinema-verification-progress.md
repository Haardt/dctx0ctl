# Complete Cinema verification goal

Active objective: the complete Cinema example project must be checkable through
spec-driven planning, generated Bun harness, verification CLI, immutable run
history, guarded automatic adoption, mocks, observable effects and lifecycles.
The earlier three-contract excerpt is not the completion criterion.

## Authoritative scope and current checkpoint (2026-10-11)

The complete current `examples/cinema/dctx0/` workspace contains 46 data
contracts, 26 operations, 12 examples, typed candidate-order assertions, the
ScreeningPoll lifecycle, three reviews, four decisions, nine named proof/test
suites and the Release policy. CommentText's visible-text refinement is an
intentional current-spec change; original compiler compatibility fixtures remain
pinned separately and are not rewritten.

The complete generated native plan records **5,674 mandatory cases, all PASS**
(Assessment 13, Interpreter 4 and corpus generators 2). These include all 46
production runtime validators, generated/reviewed real operations, fourteen
actual HTTP entry cases covering all twelve routes, twelve consumer examples,
101 assertion cases, two lifecycle paths and five persisted entity-key checks.
Actual captured dependency provenance has no incomplete entries. Production uses
its own application types, Hono, Awilix and SQLite, without testing imports.

The automatic native report retains UNKNOWN for its manual obligations. The
separate, source-bound effective assessment is **PASS with zero open obligations**.
Examples/Properties and Release rules are evaluated in Rust. Other suite/manual
claims carry explicit agent inspection and structured review evidence; they are
not relabeled as automatic proofs. A low, non-action-required finding records
that empty eligible voting yields None despite broader explanatory wording.

The actual example project's active plan has **104 Done/PASS tasks and 0 open
or non-PASS tasks**. `scripts/cinema_complete_acceptance.py` refuses changed reviewed
sources, empty plans, incomplete native corpora and active reservations/claims.
It preserves prior evidence and uses ordinary guarded transactional delivery.
See current evidence (historical evidence is available in Git history) and
[implementation/inspection limits](verification-suite-attestation-progress.md).
Fresh final aggregate coverage/performance acceptance is still running; the
coverage gate remains 97% and the accepted benchmark baseline is unchanged.

Earlier entity-key evidence (historical evidence is available in Git history) remains
historical: five mandatory state checks independently reconstruct observed keys;
actual replacement mutations detect duplicated rows and replay preserves old
results. They are bounded state checks, not a universal database theorem.

The earlier CLI proof (historical evidence is available in Git history) also verifies
that improved field mapping delivers 27 additional tasks in existing history
while preserving the earlier 14 Done/PASS tasks. All 5,552 cases still pass.

The Unicode workspace aggregate (historical evidence is available in Git history)
passed all nine stages, including all-feature and separate CLI feature tests,
with identical source hashes before and after execution. It predates the
additional malformed-precondition regression test, which independently passes.
Coverage is being measured again from a fresh isolated build; benchmark
conformance and release measurements remain explicitly deferred.

## Completion evidence required

| Requirement | Current authoritative evidence | Remaining work |
| --- | --- | --- |
| Values, refinements and spec-derived planning | Full current 46-contract corpus; exact mandatory manifest; deterministic generation, central semantics, setup and state preconditions | Final workspace coverage gate |
| Real Cinema application and Bun harness | All 5,655 mandatory cases PASS; independent native application tests and meaningful implementation/wiring mutations | Final aggregate validation |
| Adaptive integration | Same plan against frozen direct-function and differently named Hono/Awilix service fixtures; constant-adapter and wiring mutations fail | Final aggregate and closing release measurements |
| CLI and history | discover/init/plan/run/show/replay; schema-4 immutable runs/artifacts; historical replay and stale-source guards | Final complete workspace/feature validation |
| Provenance | Current complete Cinema source/dependency/adapter/harness/runtime capture; bounded native ESM observation discharges only actually loaded zero-call sites | Executed unresolved dynamic dependency sites stay UNKNOWN |
| Automatic adoption | Full Cinema adoption in isolated history; immutable manifest, per-task requirement coverage and actual-state discharge independently checked in SQLite transaction | Current strengthened aggregate validation; final coverage |
| Mocks and effects | All 12 examples with real consumers; actual arguments/call counts/errors/events; reviewed and generated real SQLite port scenarios | Explicit manual/dedicated integration requirements stay open |
| Lifecycles | Both real paths; observed before/after phases; session isolation, setup-preserving shrink and replay; production mutations detected | Final aggregate/coverage |
| Manual constraints/reviews, named suites and Release policy | Explicit UNKNOWN with complete inventory; never invented assertions from prose or names | Require supported explicit evidence contracts before a whole Release PASS can be claimed |
| Rust/helper coverage and performance | Native ESM helper 54/54 reported lines separately; historical Rust coverage is below 100% | Fresh full Rust line/function gate; release speed/RSS deferred until end, baseline unchanged |

Do not mark the goal complete while a required row lacks authoritative evidence.
Stored historical PASS is not current working-tree proof. Imported reports and
self-declared flags alone cannot authorize automatic native adoption.

## Foundation checkpoints (historical)

The pure checker derives alias/record/variant/list/option contracts, inclusive
ranges and modeled where conditions from validated IR. UUID validation and strict
RFC3339 Timestamp validation are explicit. Float has a separate finite IEEE value
wire tag instead of pretending to be exact Decimal. Pure call observations are
checked against their declared result type before they can satisfy assertions.
Unmodeled refinements and unavailable type closures stay UNKNOWN. There is no
native execution/adoption claim from these library checks.

The new tests cover invalid native-shaped values, boundaries, aliases, variant
payloads, nested records, missing types, unexecutable refinements and resource
budgets. At these checkpoints, validation commands paired workspace checks/coverage with release
speed/RSS measurements; reduced development samples must be reported as incompatible
with the accepted default-sample baseline, never as regression proof.

Historical contract checkpoint: 148 all-feature Rust tests and 8 Python tests pass;
merged source lines 11,590/11,590 and LLVM functions 977/977. The paired development
release run uses one sample/iteration and records all 78 existing workloads; its
baseline assessment is explicitly incompatible/unassessed. The accepted baseline
is unchanged. Raw checkpoint evidence (historical evidence is available in Git history) retains
coverage totals and speed/RSS samples. This is progress, not complete Cinema
acceptance; the other completion rows remain outstanding.

Generation checkpoint: deterministic type corpora and operation/quantifier
parameter tuples are implemented. The complete unmodified Cinema spec supplies
all 45 tested declaration domains. 155 Rust/8 Python tests pass, with 12,065 merged
source lines and 1,016 LLVM functions at 100%. Paired release evidence now includes
80 workloads; development sample settings remain explicitly unassessed. See
[algorithm and limits](verification-generation.md) and
raw evidence (historical evidence is available in Git history). Full plan projection and the
remaining native/application/workflow rows remain outstanding.

Projection continuation: `project_program` accounts for the full Cinema scope,
retains original contract/operation/manual/policy/suite clauses and records ordered
mock/invoke/check actions. Reviewed concrete symbolic inputs are required and
type-checked; missing inputs cannot imply scenario success. Lifecycle paths retain
connected prerequisites and loop transitions, with explicit unreachable/budget
issues. `verify plan` reads exact published check occurrences, validates task-plan
provenance and rejects stale current specs without modifying history. See
[planning contract](verification-planning.md). This adds two release projection
workloads (82 total). The coupled run passes 173 Rust/8 Python tests, 13,143 merged
source lines and 1,099 LLVM functions at 100%, with all 82 release speed/RSS
measurements complete. Development settings remain unassessed against the accepted
baseline. Raw evidence (historical evidence is available in Git history) preserves the successful
run, earlier incomplete runs and distinct raw LLVM totals. Native execution and
the remaining completion rows are still open.

Concrete scenario data is now supplied in
`examples/cinema/.dctx0/testing/typescript/examples.json`: all 38 symbolic inputs
for the 12 examples satisfy their IR types/refinements, with relationships chosen
from the spec's sample outcomes. The actual CLI accepts those inputs without
binding issues in an isolated copy. This neither executes an example nor proves
production behavior. The next executable step must provide the target SDK/Bun
worker and real adapter sessions, then consume these scenarios and the operation,
property and lifecycle plans. It must not treat planning checkpoints as complete
Cinema acceptance.


Bun transport checkpoint: the separate target SDK and generated testing worker
are implemented. Actual worker packets are decoded by Rust; success flags and
shape/version/size violations are rejected. Four Bun tests exercise real calls,
fresh sessions, actual counter state/events, external mock replies and failure/EOF
handling. One literal Cinema tie case invokes the existing production function
through its external adapter. This is neither complete pure-operation verification
nor full application acceptance. The coupled workspace checkpoint passes 177
Rust/8 Python tests and 13,244 merged lines/1,108 LLVM functions at 100%, with 84
release workloads plus separate native timing/RSS samples. Development settings
remain unassessed. See [transport contract](bun-harness.md) and
raw evidence (historical evidence is available in Git history).

Next required work is the managed Linux executor and central Rust scenario runner,
including correct adapter-vs-application exception classification and bounded
stream/time/log/process handling. Then the complete Cinema application, native
contracts/examples/properties/state/lifecycle checks, immutable verification
history and guarded adoption must use that real execution path. Do not substitute
additional library/transport checkpoints for those missing completion requirements.


Managed executor checkpoint: Linux process-group ownership, bounded streams,
timeouts/cancellation, explicit shutdown/cleanup and pure-case continuation are
implemented. Real Bun passes the complete modeled winner invariant in 101 fresh
cases with 201 calls; mutation of an isolated production copy fails centrally.
The full paired check passes 192 Rust/8 Python/4 Bun tests and 13,979 merged lines/
1,183 LLVM functions at 100%, with 84 release speed/RSS workloads. Raw LLVM still
reports separate region/instance metrics, and helper line coverage is outstanding.
Retained evidence (historical evidence is available in Git history) includes native process costs
and earlier failed coverage checks. Development measurements remain unassessed;
the accepted baseline is unchanged.

The next required implementation is central ordered example/mock/state execution
and the whole-plan runner, using actual Cinema application components. The CLI
currently exposes planning only; immutable run publication and guarded task
adoption remain missing. Adapter exceptions must be distinguished from explicit
application observations. Request timeouts are enforced; whole-case budgeting
still needs enforcement across multiple requests. Do not infer complete
application verification from this winner-only result.


Cinema example execution checkpoint: [central scenario execution](cinema-scenarios.md)
passes all twelve examples against real consumer methods, with sixteen actual
collaborator requests, seven events and nineteen checked expectations. Native
event/wiring mutations are detected. The regular TypeScript check passes. The
coupled checkpoint passes 202 Rust/8 Python/4 Bun tests, 14,526 merged Rust lines/
1,232 LLVM functions at 100%, and all 84 paired release workloads.
Raw evidence (historical evidence is available in Git history) retains native costs/source hashes
and failed intermediate runs. The accepted baseline is unchanged; development
settings remain unassessed.

Next implementation must deliver real production ports and actual state/lifecycle
observations, complete operation-obligation/whole-plan assessment and partial
transport evidence. Verification artifact setup/run/show/replay, immutable
provenance/history and guarded native task adoption still need implementation.
The native example library alone is not completion or adoption authority.


Real-state continuation: SQLite production ports and explicit Hono/Awilix
composition are implemented. Independent native tests cover persistence and HTTP,
including error handling, lifetimes and cleanup. Both Cinema lifecycle paths run
through the generated worker against actual SQLite observations. Closing-state and
HTTP wiring mutations are detected. See [scope and open modeling points](cinema-state-lifecycles.md).
The full paired checkpoint passes 217 Rust/8 Python/4 Bun transport tests and
36 native Cinema tests, with 15,332 merged Rust lines and 1,298 LLVM functions at
100%. All 84 release workloads complete; reduced settings remain unassessed
against the unchanged accepted baseline. Native application coverage separately
reports 210 lines/131 functions at 100%.
Raw evidence (historical evidence is available in Git history) retains measurements, native results,
raw LLVM metrics and intermediate failures. This is a checkpoint, not complete
Cinema acceptance or safe task adoption.

Required before whole-run publication/adoption: bind runtime observer evidence to
pinned adapter capabilities, including absent event/read/write observers; an empty
worker callback list alone does not prove an unwired production source emitted
nothing. Preserve partial setup/invocation/state evidence on transport failures.
Classify unexpected setup application exceptions explicitly, distinguish delivered
from successfully completed transitions, and report observed states even when a
transition fails. Configured persistent databases need an explicit isolation/reset
policy before independently fresh cases may be asserted. The current native
Cinema acceptance uses fresh in-memory databases only. These are orchestration
requirements, not reasons to lower coverage or silently modify the specification.

The next implementation priority is the whole-plan runner and usable verification
CLI (artifact initialization, discovery, execution and saved reports), followed by
immutable schema-4 run history, guarded task adoption and shrink/replay. Avoid
substituting further isolated library checkpoints for that missing workflow.


Setup continuation: `verify init` and configured `verify plan` are usable from
the CLI, with preserved developer files and empty default scope. The coupled
checkpoint passes 224 Rust/8 Python/4 Bun transport/36 native Cinema tests,
15,619 merged lines/1,313 LLVM functions at 100%, and 86 release workloads.
See [setup contract](verification-setup.md) and
raw evidence (historical evidence is available in Git history). No native run/history/adoption
authority follows from this setup checkpoint; the full goal remains active.


History continuation: immutable schema-4 storage and `verify show` are implemented.
229 Rust/8 Python/4 Bun transport/36 native Cinema tests pass, 15,894 merged
Rust lines/1,343 LLVM functions are covered, and all 90 paired release workloads
complete. [History contract](verification-history.md) and
raw evidence (historical evidence is available in Git history) preserve scope and metrics.
This is storage/read capability only. The next action must integrate real managed
whole-plan execution with report publication and guarded native adoption; imported
or self-declared native evidence alone remains insufficient.

Native transport failures now retain the latest case's actual ordered session
prefix, including returned values, observed states and mock requests/replies.
Cleanup acknowledgments and case separation are tested. The coupled checkpoint
passes 229 Rust tests with 15,931 merged lines/1,344 LLVM functions at 100% and
90 release workloads; see retained evidence (historical evidence is available in Git history).
This supplies partial evidence to the missing whole-plan runner, not another
claim of complete Cinema verification. Run publication and safe adoption remain
the immediate implementation priority.

Managed run continuation: `verify run` is now implemented with captured sources,
fresh generated harness/model files and immutable run publication. The complete
Cinema plan attempts all projected value cases, executes 100 generated pure
operation calls, 101 assertions, all 12 examples and both real lifecycle paths.
Its overall UNKNOWN retains remaining operation/type/manual obligations. Task
adoption is unavailable and explicit in the report; discovery and replay are
still missing. See [actual CLI workflow](verification-run.md). Functional workspace
tests passed, but the first full validation failed coverage (99.41% merged lines,
99.28% functions). New negative tests cover capture budgets, incompatible bundles,
missing runtimes, wrong value shapes, protocol errors and empty plans. Full
coverage restoration and the original full Cinema acceptance remain required.

## Handoff checkpoint, 2026-10-10

The objective remains incomplete. The latest focused precondition validation
passed on a stable source tree with 92 paired release workloads; its full record
is retained in `docs/verification-preconditions-validation.json`. The task delivery
and case mapping records are retained separately alongside it. All use one sample
and one iteration: development measurements are unassessed against the accepted
baseline and do not establish a performance-gate pass. No baseline was promoted.

Implemented since the earlier full coverage checkpoint:

- Pure adoption eligibility and mandatory requirement-to-case mapping.
- Task-selected native plan/event/requirement/profile pins and build configuration hashes.
- Explicit atomic Store start/complete/check delivery with profile guards, run links,
  idempotent receipts and rollback tests. Rollback changes SQLite history/state only.
- Read-only publication receipts and rejection of imported/incomplete linked evidence.
- Original application inventory comparison after execution; actual captured evidence
  survives detected production edits and no code is reverted.
- Kernel-owned mandatory case manifests, refinement requirement IDs and generated
  precondition selection. Explicit cases remain mandatory; empty/unresolved corpora
  remain open. The native precondition fixture executes only the two eligible inputs.

Still required for the original complete Cinema goal:

1. Construct complete native per-task proofs and reliable dependency/adapter/runtime
   provenance; connect the private Engine execution path to guarded automatic Store
   publication. Neither imported reports nor origin flags constitute this authority.
2. Execute complete nonpure generated operation obligations, actual resource/effect
   boundaries and all required production port conformance, including full type closure.
   Six reviewed cases and twelve examples are not a replacement for that closure.
3. Complete central shrinking, prerequisite-preserving lifecycle shrinking/replay and
   all-kind replay conformance.
4. Finish generated/native helper coverage, separate release execution/init/mock/shrink/
   replay/adoption workloads, and restore 100% merged Rust lines/LLVM functions.
5. Run and inspect the full workspace/feature/coverage/performance acceptance. The last
   older full coverage gate failed at 98.31% lines and 97.86% functions; these figures
   are historical and are not fresh coverage for the new source tree.

The complete Cinema run remains UNKNOWN. The new storage groundwork is deliberately
not wired to automatic completion until full native authority is available.


### Gebündelte Implementierung (2026-10-10, übernommen; Abnahme offen)

Die aus `/tmp/dctx0-verification-batch` übernommene Implementierung ergänzt typisierte Ressourcenbeobachtungen im Core/SDK, tatsächliche SQLite-Statement-Beobachtung im Cinema-Testadapter, zentrale Prüfung deklarierter Ressourcen-/Aufruf-/Ereignisgrenzen, explizite vollständige Zustandsersetzung und deterministische Shrink-Versuche gegen echten Anwendungscode. Shrink-Artefakte erhalten die gelieferten nativen Beobachtungen; `verify replay --minimized` verwendet die gespeicherte minimale Eingabe als neuen Vergleich. Subject-Auswahl pinnt passende bereits vorhandene Task-Anforderungen. Der private native Übernahmepfad erstellt Vorschläge und verwendet die transaktionale Veröffentlichung; die bestehende Sperre für unvollständige Provenienz bleibt wirksam.

Compilerchecks bestehen mit allen Features und für die CLI ohne Standardfeatures. Neue Rust- und native Konformitätstests sind vorbereitet; ihre Ausführung erfolgt im gemeinsamen Lauf. Der frühere vollständige Lauf ist abgeschlossen (Coverage-Gate nicht bestanden). Der neue gemeinsame Lauf `/tmp/dctx0-verification-batch-validation.log` prüft den übernommenen Stand mit Coverage sowie einer Zeit-/RAM-Stichprobe pro Workload. Es gibt damit noch keinen vollständigen Cinema-Nachweis, keine freigegebene automatische Übernahme und keine bestätigte vollständige Abdeckung. SQLite-Beobachtung erfasst explizit ausgeführte Statements, keine impliziten Trigger- oder Fremdschlüsselzugriffe. Zustandsersetzung ohne vollständige Quelle bleibt offen, sofern keine modellierte Gleichheit oder eindeutige Lifecycle-Grundlage vorhanden ist.


### Aggregate validation failure found (2026-10-10)

The first combined workspace test execution failed in two managed Cinema native tests because their temporary-project copy lists omitted the newly imported `resource-observer.ts`. Sixteen other native tests passed before that test binary returned failure. The isolated corrective batch now copies this testing helper into both temporary projects and passes the compiler check. The ongoing measurement still describes the failing source state; the correction has not been transferred or accepted. The current runner stops subsequent validation stages after a failure, so this run cannot establish coverage or execution of later test binaries. A prepared runner correction uses Cargo `--no-fail-fast`, continues independent stages and retains per-stage command/exit-code evidence.


### Actual evidence from the aggregate verification run (2026-10-10)

The aggregate run `/tmp/dctx0-verification-aggregate-errors.log` confirms the corrected managed native Cinema tests: all twelve centrally mocked consumer examples and both SQLite lifecycle paths pass. The real failing-operation shrink/minimized replay test passes, including comparison after an actual implementation fix. Explicit replacement, source-boundary/dependency report validation and TypeScript dependency reference tests pass.

The complete Cinema run now records FAIL rather than the earlier UNKNOWN: `FilmCatalogStore.save` calls the existing summary helper, which actually queries `Rating` while that operation declares only `write Film`. The new real resource observer exposes this production/spec mismatch. This is not repaired by expanding the spec or ignoring the access. Two outdated test expectations (the former generic adoption-unavailable reason and the old resource failure precedence) and the whole-run UNKNOWN expectation are prepared for correction in the isolated batch; they have not yet been transferred. Remaining unexecuted/unsupported contracts and operation obligations are still open even when FAIL dominates the report. Automatic adoption and complete application acceptance remain unachieved.
# Real operation fixture expansion (2026-10-10)

The checked-in external adapter configuration now selects 28 reviewed real
operation cases across all 25 nonpure Cinema operation subjects. The additional
cases exercise each persistence port and public application service using actual
SQLite sessions, explicit input values and actual prerequisite operations.
The pure winner operation retains its separately generated corpus. No production
source or specification was changed for this expansion.

This is explicit case coverage, not complete generated nonpure contract closure.
The plan still reports that missing closure, absent value factories and incomplete
adoption provenance. Runtime results for the expanded cases are pending.

The preceding aggregate run is retained as
`benchmarks/history/20261010T120202.519130Z-1201954.json`. The unexpected-mock-call
mutation and recoverable dependency transport tests passed. One Cinema integration
assertion incorrectly rejected the genuinely passing closed-vote case; its
expectation now permits PASS only for that named original case while retaining
the five resource-access failures. The corrected assertion and expanded fixtures
have not yet completed runtime validation.


## Native adoption and complete case delivery evidence (2026-10-10)

`benchmarks/history/20261010T122655.403866Z-1234802.json` retains the aggregate
verification run and paired release speed/RSS measurements. The native adoption
regression, acknowledged runtime identity, and whole-Cinema mandatory-case
delivery tests passed. The complete Cinema report includes actual resource
violations and unknown factory/projection requirements; passing the delivery
test does not mean the application satisfies the whole spec. The run's only
validation failure was Clippy's test-module placement, subsequently corrected;
workspace Clippy now passes. No benchmark baseline was promoted.

## Complete configured CLI run (2026-10-10)

The release CLI now records the complete Cinema scope in the example's derived
history, with all 5,543 mandatory cases executed: 3,698 contract cases, 1,730
operation cases, 101 assertion cases, 12 examples and two lifecycle paths.
The result is FAIL (939 PASS, 579 FAIL, 4,025 UNKNOWN). Actual undeclared resource
reads and application exceptions are retained. Missing native value validators,
unmodeled result projections, ambiguous call aliases and unmet preconditions
remain explicit; the spec is not changed to produce PASS. No tasks were adopted.

Run `dctx0 verify show examples/cinema/dctx0
ca7ebeb7d3f4289bde72ff8882a7ef6c9b6eb6fe62bd899cdc8d3bad7592df67`
to inspect saved evidence. Retained scope and provenance (historical evidence is available in Git history)
include the exact plan, implementation, dependency, adapter, harness and runtime
bindings plus measured release CLI wall time and memory scope. The native run
took about 1.97 seconds; its RSS measurement includes reaped child processes and
is not an isolated Rust-core measurement. Full acceptance remains incomplete.

## Remaining completion audit (2026-10-10)

The current central state/projection/replay and verifier-identity implementation
passes focused validation and paired release measurements in
`benchmarks/history/20261010T155006.326205Z-1376553.json`. This does not replace the
last failed full workspace coverage gate.

- Native contract access: only FilmId and ReleaseYear expose actual production
  factories. The 3,570 unavailable value cases cannot be replaced by adapter
  implementations of spec logic. Additional real project access is required.
- Result modeling: 242 Cinema cases lack a complete executable result projection.
  Independently modeled equalities are supported, but no correspondence is
  inferred from record names, fields or prose.
- Valid state/input scenarios: 115 generated operation cases cannot establish
  their preconditions in the supplied fresh state. They are not PASS or executed
  business invocations.
- Actual implementation failures: 579 cases still fail (undeclared SQL resource
  access and application exceptions). Tool acceptance must retain and detect
  these failures; the specification is never weakened to remove them.
- Dependency provenance: structural inspection retains the entire inventory;
  native resolution now traverses the adapter and declared implementation roots,
  recording visited and nonselected sources separately. A native regression
  proves unused legacy imports do not block adoption while reachable dynamic
  imports still do. Awilix/Hono package closure remains unassessed and continues
  to prevent a complete adoption proof.
- Whole-Cinema task adoption: complete native evidence is still missing. Bounded
  native contract adoption is demonstrated; partial evidence, claimed/reserved
  work and historical results remain protected.
- Coverage: the last complete run failed at 97.44% merged Rust source lines and
  97.43% LLVM functions. The required 100% gate has not been lowered. Bun helper
  LCOV is separate, with unmapped comment/function-identity limitations.
- Dedicated native release phases, final documentation reconciliation and a fresh
  complete workspace/coverage acceptance still remain.

The complete Cinema CLI scope has 5,543 required cases. Last retained assessment
reports 1,037 PASS, 579 FAIL and 3,927 UNKNOWN; subsequent supported features are
not a claim that those missing application/model obligations have been resolved.

The scoped-dependency CLI rerun retains the same 5,543 executed cases and outcome
counts. Its provenance now names only the actual unassessed Awilix/Hono package
closures, rather than the unused legacy binding imports. See
the retained summary (historical evidence is available in Git history), run
`59816bdb1d645f3116bd6bf5f58ff580b8a3fc55fad3104342e4bc365fd361f8`.
No task adoption occurred. Performance is deferred at the user's request;
this debug execution is functional evidence, not a release benchmark.

The subsequent direct coverage run passes its executed tests but fails the
unchanged 100% gate: 21,031/21,593 merged source lines (97.40%) and 1,693/1,733
LLVM functions (97.69%). Raw exports remain in `target/coverage`. This includes
the current Go target tests and is not directly comparable to the older source
count. Performance remains deferred. A separately added native regression now
confirms that exhausting the 1,000-reference dependency resolution budget cannot
adopt an otherwise passing contract or change task events; that regression is
not included in the preceding coverage measurement.

The combined verification-focused validation after bounded shrink generation,
legacy minimized replay, operation/lifecycle setup guards and static lifecycle
input inspection passes all selected stages on an unchanged source hash.
Retained check report (historical evidence is available in Git history) explicitly
marks performance deferred. This does not replace the full workspace/coverage
acceptance or resolve the missing Cinema production value access.

The full configured Cinema rerun with typed lifecycle input contracts still
executes all 5,543 cases: 1,037 PASS, 579 FAIL and 3,927 UNKNOWN. Both actual
lifecycle paths PASS; no tasks are adopted. The overall application verdict
remains FAIL. Retained evidence (historical evidence is available in Git history) includes
the two observed lifecycle cases and run provenance; performance is deferred.

The bounded-session evidence change passes all 20 native executor tests and all
20 verification-run tests. Byte/entry overflow preserves an incomplete prefix,
reaps workers and resets budgets for the next case. Native limits are versioned
run artifacts and replay/adoption bindings. An early-rejection diagnostic
regression found by this batch was corrected without weakening its existing
profile-pin assertion. This confirmation is functional evidence; it adds no
fresh full coverage percentage or performance assessment.

The next full instrumented run passes its functional tests but still fails the
100% gate: 21,381/21,951 merged source lines (97.40%) and 1,713/1,754 LLVM functions
(97.66%). Raw exports remain under `target/coverage`. Subsequent mock trace
consistency tests are not included in that measurement. Actual Rust-observed
callbacks now prevent a fabricated adapter trace from substituting for a mock
invocation; the genuine consumer, fake-trace rejection and argument mutation
regression pass in Bun. Performance remains deferred.

Central assessment version 3 covers mock trace consistency and unknown resource
operation labels. The resource-permission dispatcher now uses explicit typed
read/write/observe access kinds. An observed unmodeled owner stays UNKNOWN,
rather than becoming a false undeclared-call FAIL. Four resource tests, six mock
tests and four operation-effect tests pass; these newer changes have no fresh
whole-workspace coverage percentage yet.

The subsequent aggregate confirmation passes all 20 native process tests and
all 20 verification-run tests, including actual Cinema examples/lifecycles,
mock trace fabrication rejection, mutation detection, replay and guarded task
adoption. Resource regression coverage also checks that zero observed accesses
cannot discharge a clause with a missing pinned reference or resource contract.
The verification crate passes strict Clippy. These are functional confirmations;
performance remains deferred and no new full coverage percentage is asserted.

The complete workspace validation with `--coverage --skip-performance` finishes
against unchanged source inputs: all-feature tests (337), Python tests (11),
CLI without default features and CLI with only Rust enabled pass, as do build,
formatting and strict Clippy. The only failed stage is the strict coverage gate:
21,462/22,018 merged source lines (97.47%) and 1,718/1,759 LLVM functions (97.67%).
The source hashes and stage outcomes are retained in
`verification-workspace-functional-validation.json`; performance is deferred.

After that measurement, lifecycle shrinking was aligned with the common typed
input contract used by planning and execution. A pinned empty transition path
can no longer produce shrink candidates merely by removing setup operations.
Valid input candidate ordering remains unchanged. Six lifecycle and four shrink
tests pass. This subsequent change is not part of the preceding coverage result.

Assessment version 4 fixes state result classification for explicit aliases to
entities and Option/List wrappers. Independent compiled-spec tests prove genuine
selected results pass, unrelated rows fail and empty selections keep their
declared cardinality. Separate DTO projections stay UNKNOWN. All 13 state-effect,
six lifecycle and four shrink tests pass, as does strict verification Clippy.
This change is likewise newer than the retained full coverage measurement.
All 20 native verification-run regressions also pass with assessment version 4,
including the complete Cinema plan, actual examples/lifecycles, historical replay,
guarded adoption and production mutation checks. No performance or fresh full
coverage result is inferred from this focused confirmation.

Dependency analyzer version 2 now includes real CommonJS member loads and keeps
escaped loader values explicitly unresolved. Four independent syntax tests pass.
The real Bun adoption regression loads two captured CommonJS modules, requires
both in persisted dependency scope, and confirms an escaped loader keeps passing
behavior incomplete without rewriting task history. Strict TypeScript-target
Clippy and workspace formatting pass. Full package closure remains open; this
increment does not claim new whole-workspace coverage or performance evidence.

The subsequent complete verification-library batch passes 80 tests. The new
state-contract rejection regression requires missing pinned state references,
entity declarations and complete keys to remain UNKNOWN even with unchanged
actual snapshots. The valid control case passes. Strict verification Clippy and
workspace formatting pass; no ordinary error path is excluded from coverage.
These tests are newer than the retained whole-workspace coverage measurement.

Run/show summaries now explicitly separate case records and invocation evidence.
The retained whole Cinema run has 5,543 recorded cases, 1,865 returned invocation
observations, 17 exception observations and 3,813 cases without invocation
observations. The latter may have value checks or central assessments; a record
count is not a count of actual production invocations. The historical CLI test
passes and verifies an imported UNKNOWN with zero invocation observations.
The saved run itself is unchanged; these counts assert no working-tree freshness.

Captured package syntax closure is now implemented, including package manifests,
native conditional import/require resolution and canonical builtin module IDs.
All 20 native verification-run tests, four engine dependency tests, five target
dependency tests and the generated Bun transport conformance pass. The fresh
whole Cinema run `60b2ca5c892922b5b1d4bd562f13a772d690eededc76b811a87305a7fe09097b`
retains all 5,543 case records with 1,037 PASS, 579 FAIL and 3,927 UNKNOWN.
Its provenance no longer reports unassessed Hono/Awilix package files or uncaptured
builtin sources. Four dynamically unresolved Awilix loader sites remain explicit.
See `cinema-package-provenance-validation.json`. No tasks were adopted. Performance
remains deferred; no new full coverage percentage is asserted.

Capture now rejects ambiguous POSIX backslash filenames rather than merging
their hashes with forward-slash paths. Four capture tests, strict engine Clippy
and workspace formatting pass; the collision regression preserves the original
production file. Native Windows separators retain their previous normalization.

A real Bun 1.4.2 probe investigated transparent module observation for the four
remaining Awilix loader sites. An onLoad hook returning undefined fails; returning
the identical source with a js loader changes CommonJS exports. An onResolve-only
hook preserves the tested application but misses an actual bare package require.
Neither supplies complete adoption evidence. Results are retained in
`bun-module-observer-probe.json`; no such observer is shipped or used to erase
the remaining UNKNOWN reasons. A dedicated, semantically transparent observation
contract remains required.

Runtime builtin regression tests found and corrected an incorrect `node:` prefix
on Bun-specific IDs. The worker now preserves `bun:sqlite` and standalone `bun`;
unknown node:/bun: names stay unresolved in both import and require modes. The
generated worker conformance tests, strict engine/target Clippy and formatting
pass. Historical package-provenance output remains immutable and can contain the
earlier labels; no fresh full coverage result is asserted.

The external Cinema adapter now checks five additional types through existing
production code: MemberId/CommentId/PollId use the shared UUID validator, and
StarRating/FilmAvailability use the actual CinemaStore validators in fresh,
finally-closed in-memory databases. Production code is unchanged. All 129 cases
for these types pass. A two-process mutation test disables the real validators
in an isolated copy and detects invalid-value acceptance for every added type.
Enum translation records the actual accepted alternative, including invalid
names, so the adapter cannot substitute spec-side validation for production.

The full Cinema run `2a0a2397b9bf88d365a41831ac9dce8558bdf8aed77c01736c2412ae2f5397f3`
records all 5,543 cases: 1,166 PASS, 579 FAIL and 3,798 UNKNOWN. See
`cinema-existing-validators-validation.json`. Assessment version 5 also separates
explanatory Intent context from machine assertions. Explicit constraints remain
unproved. The actual native adoption regression now includes an Intent and passes
with the store independently checking its kind from the pinned snapshot.
The aggregate confirmation passes 21 native process tests, 20 verification-run
tests, all 81 verification-library tests, 17 task tests and four store verification
tests. Strict Clippy for the affected crates and workspace formatting pass.
The subsequent changes have no fresh full Rust coverage measurement; performance
remains deferred.

Five further native contracts now use actual persistence entry points, without
production changes: RuntimeMinutes/Genre are embedded in a valid save request;
PageOffset/PageLimit/FilmSort are embedded in a valid search request. Each probe
uses a fresh, finally-closed memory database and records accepted native values.
All 387 cases pass. The independent mutation test now covers all ten added types,
including removed range/alternative checks in the real producer; every mutation
allows a centrally rejected value and is detected. Clippy and formatting pass.

The fresh complete Cinema run
`d5b1a27df7be542134d98505296803e1c7356cabf0fe48576625ea10a70899e2`
records all 5,543 cases: 1,553 PASS, 579 FAIL and 3,411 UNKNOWN. Evidence is retained
in `cinema-boundary-validators-validation.json`. Dynamic Awilix loaders, missing
remaining validation/projection access and full coverage still prevent completion.

Film/SaveFilm/FilmSearch record probes now invoke the real persistence save/search
entry points with the entire translated input. A selected dependency-closed run
records 898 PASS and four FAIL across 902 cases. The four actual violations are
acceptance of additional fields for each record and omission of FilmSearch's
required availability field. Translation does not rebuild a record validator
from the spec. An isolated regression retains these precise violations and still
detects mutations of the real scalar/enum validators. Clippy and formatting pass.

The complete run `7ef37f2713b59b7ea278e7bd9454f48817bbf478af484e1719f9313c726d1b35`
records 5,543 cases: 1,897 PASS, 583 FAIL and 3,063 UNKNOWN. See
`cinema-record-boundary-validation.json`. No production or spec change was made,
and no task was adopted. Coverage and performance claims remain unchanged.

ChangeAvailability, RateFilm and Rating now use their actual persistence entry
points with a fresh memory database and a real film setup. The full run
`057a70f47b9d80fd5744fff870e901c7bbba09d33e8846df30c2650922be5e14`
records all 5,543 required cases: 2,196 PASS, 586 FAIL and 2,761 UNKNOWN.
The three added contracts contribute 299 PASS and three FAIL: each production
entry point accepts an unexpected record field. These remain actual failures,
not adapter-side rejections. No production or spec code changed and no tasks
were adopted. The independent real-validator mutation regression passes,
including all seven known record violations. Retained evidence is in
`cinema-rating-boundary-validation.json`. Full coverage and remaining unknown
requirements still prevent completion; performance remains deferred.

AddComment and OpenVoting now invoke actual comment/voting persistence entry
points. The selected dependency-closed native run
`4f5c88d7a14f7ffb06b4027b4310cd4b74c6a15a75ce16fbec144e7a42239f9b`
records all 320 cases: 302 PASS and 18 FAIL, with no UNKNOWN. Seventeen failures
are valid AddComment inputs with empty text rejected by production; the typed
String contract has no modeled nonempty refinement. OpenVoting accepts one
record with an unexpected field. No spec-side checks are inserted into the
adapter. The expanded production-validator mutation regression passes and
retains the eight known extra/missing-field violations across its subjects.
Evidence is retained in `cinema-comment-voting-validation.json`. This selected
run does not replace the preceding full Cinema run or establish full coverage.

CastVote now prepares an actual film and open poll in a fresh memory database,
then calls the real voting port. Selected run
`f62180d229723b518ba1375d96952a61118386941064658c6a42aafdf363629b`
records all 205 cases: 204 PASS and one FAIL, with no UNKNOWN. Production accepts
the unexpected-field rejection case. The production mutation regression passes
with CastVote included and preserves nine known record violations. Retained
evidence is `cinema-cast-vote-validation.json`; production/spec code and task
progress remain unchanged. Performance remains deferred.

The full run `2558c3e7a6241afca36730b70e0ab684737e13949a936c841c6443b2bdecc950` after all added input boundaries records 5543 required cases: 2522 PASS, 605 FAIL and 2416 UNKNOWN. No tasks were adopted. `cinema-current-full-validation.json` retains counts and explicit unknown reasons. The CLI plan now separates explanatory Intents from mandatory manual requirements; its real command regression and strict Clippy pass. This does not complete the outstanding coverage/provenance and observation requirements.

The separate `domain-adapter.ts` connects the five existing UUID/year contracts
without loading unselected frameworks. Its native integration test compiles the
adapter with strict TypeScript checking, uses both complete original Cinema spec
files and byte-identical copies of the production validators, and executes all
218 selected cases successfully. Tasks 14, 31, 42, 47 and 53 are automatically
delivered Done/PASS in isolated history; all other tasks remain Open. Removing
the actual UUID check produces FAIL, adopts no tasks and preserves the earlier
task results and event sequence. Evidence is retained in
`cinema-domain-adoption-validation.json`. This establishes real Cinema adoption
for these contracts, not whole-application acceptance. Performance is deferred.

To select this adapter in an application, set `adapter` in
`.dctx0/testing/verification.json` to
`.dctx0/testing/typescript/domain-adapter.ts` and explicitly select FilmId,
MemberId, CommentId, PollId and ReleaseYear. Existing configuration is preserved;
the example's default adapter continues to cover its broader application scope.

The state-assessment regression suite now also checks duplicate entity keys in
either observation phase, including identical before/after snapshots and rows
with conflicting field values. These remain UNKNOWN rather than certifying an
ambiguous observation. Reordered rows with unique identities still pass. All
15 state-effect tests pass; this is evaluator evidence, not a new full Cinema
run or a fresh coverage percentage.

`cinema-full-gap-audit.json` groups the retained complete run's 5,543 cases by
all 85 subjects, with separate outcome counts, reason codes and the four
unresolved Awilix dependency sites. This is a derivation of stored evidence,
not another execution. It identifies 2,059 missing-validator cases, 242 state
result projection gaps and 115 unmet operation prerequisites.

Some remaining failures are genuine specification/implementation differences:
the production film summary reads ratings, polls and votes, while catalog port
effects declare Film access only. Saving a film returns that summary and thus
also performs reads beyond its declared write effect. The voting open contract
models an observation of a poll while production creates one and removes its
old votes. Output-only DTOs, events and failure payloads have TypeScript types
but no runtime validators to exercise. Adapter-side assertions cannot replace
those validators or establish the missing intended projection. Completing this
acceptance therefore requires reviewed application/contract changes as well as
the remaining verification coverage work; changing expected results or hiding
observed resources would invalidate the proof.

After explicit authorization to make Cinema work as a verified application,
application-owned `src/domain/contracts.ts` now supplies runtime validators for
all named data/message/event/failure shapes. The external adapter exposes the
24 previously missing declarations through these actual validators. Real
SQLite port results, published events and ApplicationFailure payloads use the
validators during normal application execution; production imports no harness
or spec. Strict TypeScript checking and all 40 native application tests pass,
including rejection of a malformed persisted film through a real port.

The expanded Rust/Bun corpus regression covers 42 selected contracts and
detects mutations of the real validators. Output validation now prevents two
previous extra-field violations; seven other known boundary violations remain
explicit. Valid Int values outside JavaScript's safe range are explicitly
unavailable, not rounded or marked passed. This is a remaining representation
gap for output DTO count fields, so this checkpoint is not full acceptance.
The preceding aggregate test command completed its checks but returned failure
because application sources changed during execution; it does not certify the
final tree and must not replace the earlier pinned validation evidence.

The next repair applies the actual input contracts before every persistence
port invocation. Missing and extra fields are now rejected before state changes;
the corpus regression retains no known field-acceptance violations. Comment
append accepts empty/whitespace String values as specified. DTO integer counts
use the application-owned `ExactInteger` representation (safe number or bigint),
and the adapter preserves large integers exactly for these contract checks.
The 43-subject corpus/mutation test now executes without skipping unrepresentable
values. All 41 native application tests, strict TypeScript checking and focused
Rust Clippy pass. The full Cinema execution regression has been started against
this implementation; its result is still pending. Resource-contract differences,
state projections and adoption provenance remain separate outstanding work.

The full post-repair execution regression passes, including all examples,
assertions, both lifecycle paths and replay checks. Its complete native run
records 5,543 mandatory cases: 4,624 PASS, 562 FAIL and 357 UNKNOWN. All 45 data
contracts (3,698 cases) PASS against actual production validators. No
NATIVE_VALUE_UNAVAILABLE cases remain. UNKNOWN is now restricted to 242
STATE_RESULT_PROJECTION_REQUIRED and 115 OPERATION_PRECONDITION_UNMET cases.
The overall application is not PASS and no tasks were adopted in this run.
Evidence is retained in `cinema-runtime-contracts-full-validation.json`; the
test's raw stored report remains in `target/native-executor-conformance`.
This does not replace the outstanding full coverage gate or performance phase.

The first explicit Cinema DTO postcondition is now modeled on
CommentStore.listByFilm: the result contains exactly the matching observed
comments, translated to CommentView, and their independently counted total.
The published spec check passes. All 17 cases for this operation in the selected
CLI run PASS (the broader required closure still has other failures/unknowns).
An actual Bun/SQLite test creates two films and two comments, verifies the
correct result, and rejects real production mutations of the total and film
filter. Evidence is `cinema-comment-projection-validation.json`.

This deliberately adds executable meaning to the working spec. Complete
original Cinema inputs are retained in `fixtures/semantic/original-cinema` for
the historical AST/model/snapshot compatibility gates; the existing golden
files are unchanged. Those gates all pass. Current-spec CLI/native acceptance
continues to use `examples/cinema/dctx0`, rather than the historical copy.

FilmPresence.exists now explicitly projects the matching observed Film ID as
an Option using the central `first` collection builtin. All 17 selected native
operation cases PASS. A real SQLite test covers both present and absent films;
production mutations returning false absence or false presence both FAIL.
The same test continues to verify actual lifecycle paths and comment projection
mutations. Compiler/interpreter tests and Clippy pass. Evidence is retained in
`cinema-presence-projection-validation.json`; its broader selected closure is
still not PASS because rating projections/resource declarations remain open.

Rating identity is now explicitly `(filmId, memberId)` and vote identity is
`(pollId, memberId)`, matching the stated per-film rating/per-poll active-vote
contracts. This is a deliberate spec identity correction, not observer-side row
filtering. A central state-effect test proves that updating one film's rating
preserves another film's rating for the same member, and rejects deletion or
accidental modification of that other row. All six real persistence/HTTP tests
pass, including the existing multi-film member and vote-replacement scenarios.
The updated spec check and focused Clippy pass. Rating result projection remains
unfinished; changing entity identity alone does not certify its DTO result.

RatingStore.summary now models the actual required film-scoped result: count,
mean of the five declared star alternatives, and the selected member's rating.
All 89 native operation cases PASS. An independent actual SQLite test covers no
ratings and two films sharing a member, with two ratings on the selected film.
Incorrect mean, missing member rating and cross-film aggregation mutations all
FAIL. `cinema-rating-projection-validation.json` retains this evidence. Core
compiler/interpreter/state tests and Clippy pass. Rating upsert and other
operations still need their own complete projections/effect contracts; the
broader CLI closure does not yet PASS and no tasks were adopted.

RatingStore.upsert now explicitly projects RatingSummary from after-state and
declares the Rating read needed to compute that aggregate. The API consumer
also declares this aggregate read. All 96 selected upsert contract cases PASS;
95 generated cases without film setup return the declared persistence failure,
so these counts alone do not claim successful replacement for every input.
Independent real successful replacement tests establish preserved other-film
ratings and stable count; incorrect count and an ignored SQL update both FAIL.
`cinema-rating-upsert-validation.json` records the observations and this limit.
Success-result projections are guarded by `success implies`, preserving the
separate declared failure contract.

The explicit visible-comment constraint is now modeled as CommentText's
`where length(trim(value)) > 0` refinement in Comment, AddComment and CommentView.
Production again rejects empty/whitespace text; the earlier plain-String
checkpoint did not satisfy this narrative requirement and is superseded.
All 41 application tests and the expanded contract mutation test pass. Cinema's
technical configuration explicitly uses recursion depth 16 for the nested
guarded DTO projections; the global default stays 8 and no mandatory case is
silently discarded. Full current-spec execution still needs fresh aggregate
validation after these changes.

The refreshed full-spec run records 5,552 mandatory cases: 4,758 PASS, 560 FAIL
and 234 UNKNOWN. All 46 data contracts (3,713 cases) PASS, including the new
visible-text refinement. Remaining UNKNOWN cases are 119 result-projection gaps
and 115 unmet operation prerequisites. This run predates the subsequent strict
equivalent-record assessment change; `cinema-community-full-validation.json`
retains its exact scope and counts.

Assessment version 7 allows differently named result/entity records only when
their complete field names and declared field types match. It permits
CommentView's actual stored Comment value without guessing a field conversion.
Tests preserve UNKNOWN for extra fields or differing nominal field types and
FAIL for different actual values. CommentStore.append additionally models
filmId/authorId/text correspondence with its request. All 96 selected cases PASS;
actual SQLite mutation tests reject incorrect stored text and a returned text
that differs from persistence, while preserving another film's comment.
Evidence is retained in `cinema-comment-append-validation.json`. State tests,
real native projection/lifecycle tests and Clippy pass. These selected results
do not establish a full application PASS or authorize adoption of incomplete
operation/integration tasks.

Catalog find/save/changeAvailability now explicitly project FilmDetails from
observed film, rating, poll and vote state. Their resource declarations include
the aggregate reads required by the existing CatalogReadModel decision and
FilmSummary fields. Availability replacement explicitly preserves every other
film field. Selected CLI cases all PASS: 17 find, 102 save and 65 availability
changes. Independent real SQLite scenarios use two films, three ratings, an
open poll and a vote, and also check absent-film lookup. Incorrect average,
cross-film vote count, persisted title and availability mutations all FAIL.
`cinema-catalog-details-validation.json` records full observations, including
declared-failure counts and the broader closure's remaining non-PASS outcome.
Compiler/interpreter tests pass; catalog search and voting projections remain
unfinished, so no whole-application/adoption claim follows from this checkpoint.

The shared Rust evaluator now supports explicit substring search and bounded
list slicing for the upcoming catalog search postcondition. Independent vectors
cover Unicode text, case sensitivity, zero limits, out-of-range offsets and
integer indices beyond JavaScript precision. An end-to-end state projection
test verifies the correct page and unpaged total, and rejects wrong-page and
page-sized-total results. Compiler/interpreter/projection tests pass. These
mechanisms are not yet the complete Cinema search postcondition; sorting and
the remaining filters must still be bound to its real operation.

Stable central sorting is now available for the catalog search model. Typed
compilation rejects nonnumeric/nontextual keys. Tests cover numeric and Float
keys, UTF-16 text ordering, stable equal-key order, a nested secondary-ID sort,
incompatible keys and comparison-budget exhaustion. Incorrect tie ordering
fails an independently modeled state-result projection. Compiler/interpreter
and all 20 state-effect tests pass. This remains supporting semantics until
the full Cinema search postcondition and native mutation scenarios are added.

The full FilmCatalogStore.search postcondition is now executable: explicit
case-insensitive title matching, the six AND-combined optional filters, all four
sort modes with ascending ID tie order, slicing and unpaged total. All 101
selected native search cases PASS. A real SQLite matrix with three films,
ratings, an open poll and a vote passes seven scenarios (including all sort
modes, combined filters, empty results and paging); six actual production
mutations of query/genre filtering, sort direction, tie order, page offset and
total all FAIL. Evidence is `cinema-search-projection-validation.json`.

The central conditional evaluates only its chosen branch; descending sorting
preserves equal-key order. Interpreter version 3 pins these semantics and exact
mixed Float comparisons. Cinema explicitly configures recursion depth 32 for
the full nested search projection; default budgets remain unchanged. Core
compiler/interpreter/state tests, native mutation tests and Clippy pass. Voting
contracts, prerequisite setup and complete adoption provenance are still open;
the broader run remains non-PASS.

## Voting persistence projection checkpoint

VotingStore.cast now explicitly upserts the composite poll/member vote identity,
while retaining the poll state. VotingStore.close explicitly preserves poll ID,
schedule and candidate order while changing its status to Closed. Their result
projections independently derive per-candidate vote counts and the deterministic
winner from observed persisted votes; they do not call the production winner as
an oracle. Actual Film and ScreeningPoll reads are declared at the port and
consumer boundaries. The updated full Cinema spec compiles.

The selected CLI corpus has 100 cast and 17 close cases, all PASS; many generated
inputs exercise declared failures rather than successful poll setup. Independent
real SQLite scenarios therefore also verify vote replacement and closing on a
filled poll. Both pass, and four production mutations (incorrect counts, ignored
replacement, incorrect closed status, missing winner) fail centrally. The wider
selected closure remains UNKNOWN (missing required state observer), so this is
not complete Cinema acceptance or an adoption claim. See
retained voting evidence (historical evidence is available in Git history).

## Voting open/current checkpoint

Opening a poll now explicitly upserts its ID, appointment, candidate order and Open
status, removes only that poll's previous votes, and returns zero candidate counts
with no winner. Its read/write FilmVote effects are explicit. Current voting
projects the most recent open poll, falling back to the most recent closed poll,
with actual vote counts and deterministic winner. The projection uses central
Rust collection semantics, not a worker success flag.

The full spec compiles. All 101 selected open cases and both selected current
cases PASS. Seven independent native scenarios additionally cover populated
reopening, current open/closed/empty state and three actual production mutations;
correct scenarios pass and mutants fail. This covers meaningful successful state
that generated failure cases alone cannot establish. Retained evidence is in
open/current validation (historical evidence is available in Git history). The native
Cinema lifecycle/state suite and Clippy pass at this checkpoint; complete
aggregate validation and the complete adoption/provenance gates remain open.

## Whole-plan checkpoint after voting projections

The complete current native Cinema plan executed all 5,552 recorded cases:
5,437 PASS, 115 UNKNOWN, zero FAIL. All remaining non-PASS cases are generated
ScreeningPlanningApi.castVote (99) and closeVoting (16) calls whose required poll
was not prepared. Preconditions are preserved; a reproducible preparation
mechanism is still needed for these generated cases. Open obligations also
retain missing actual state observation for mocked examples and manual review
points. No tasks were adopted (--no-adopt). This is substantial full-plan
progress, not complete acceptance.
Whole-plan evidence (historical evidence is available in Git history) pins the snapshot,
check occurrence, budgets, counts and open obligations.

## Prepared full native corpus

The common plan now supports pinned typed preparation for exact generated inputs.
Cinema explicitly configures 115 required voting preparations: save the actual
candidate film where needed, then open the corresponding real poll. Both selected
surface operation corpora pass (101 castVote and 18 closeVoting cases, including
reviewed cases). The complete current Cinema run executes all 5,552 mandatory
cases, and every case is PASS. This does not yet make the complete run PASS:
open obligations retain two statically unresolved preconditions, 18 manual review
requirements and nine suite inspections. The executed case evidence must be
reconciled with those requirements, and automatic-adoption provenance plus final
aggregate/coverage validation remain outstanding.

The preparation planner rejects duplicate/non-corpus binding selections and
invalid setup operations/types. Projection tests, pinned-setup replay unit tests
and focused all-target Clippy pass. Performance is deferred and no baseline is
changed. Full prepared-corpus evidence (historical evidence is available in Git history)
retains the truthful overall UNKNOWN result and all remaining obligations.

## Runtime precondition reconciliation

The full current Cinema run again executes all 5,552 cases with PASS. Rust
independently rechecks every generated voting precondition against explicitly
recorded Before state and discharges the two planning-time missing-state hints.
The run stores their resolution as a separate immutable artifact. Remaining open
requirements are 18 manual requirements and nine named proof/test suites; the
overall report truthfully remains UNKNOWN. See
complete run evidence (historical evidence is available in Git history).

The positive central reconciliation test and ten evidence mutations pass;
missing/duplicate cases, unknown result, changed setup/inputs, missing invocation
and absent/unrecorded/empty state cannot discharge a requirement. Workspace
Engine checks and focused all-target Clippy pass. The full native acceptance test
is strengthened to require all current Cinema cases PASS while retaining only
manual/suite obligations; its running result is recorded separately.

## Complete captured dependency provenance

The complete current Cinema CLI corpus again has 5,552 PASS cases and no remaining
provenance gaps. Actual Bun instrumentation records that Awilix's four captured
ESM load sites were not executed in this run. Their gaps are discharged only for
this execution, using source-bound runtime evidence; the static inspection and
observer trace remain immutable artifacts. CommonJS and executed dynamic loaders
retain conservative limits. Overall status remains UNKNOWN for 18 manual
requirements and nine named suites, and this run requests no adoption.
Retained complete-corpus evidence (historical evidence is available in Git history) records
the actual scope and open requirements.

Target planning/conformance and nine Rust evidence rejection mutations pass. Six
real native observer scenarios preserve actual values and exercise dynamic
loading, changed hashes, invalid ranges and trace exhaustion. A large-protocol
shutdown issue found under native instrumentation is corrected and tested.
Separate Bun LCOV reports 54/54 observer-helper lines; raw function counters retain
their Bun identity limitations. See native helper evidence (historical evidence is available in Git history).
The strengthened full native integration test passes, including complete dependency
provenance and replay. Final aggregate and Rust coverage validation are still required.

## Additional rejection-path validation (2026-10-11)

The engine's three native dependency-resolution tests pass using managed protocol
workers. They cover absent captured roots, invented or unassessed dependencies,
production imports into the testing boundary, unexpected replies and mock calls,
and retention of the executed prefix when the resolution budget is exhausted.
These technical tests are not application execution evidence.

All 22 state-effect tests pass. Additional cases verify that unavailable or
incomplete entity sources cannot certify an empty write and that an unknown
state-effect mode cannot certify matching snapshots. Workspace all-target,
all-feature Clippy passes with warnings denied. No new whole-workspace coverage
percentage is claimed from these focused tests; performance measurements remain
deferred as requested.

All five deterministic shrink tests also pass. The additional guard regression
starts with a valid invocation and verifies that smaller inputs with an
unevaluable precondition are discarded with visible issues, rather than treated
as admissible counterexamples. Verification-crate all-target Clippy passes.
The CLI also passes all-target compilation with no targets enabled and with
only TypeScript enabled; these are compilation checks, not native acceptance.

## Fresh aggregate functional acceptance (2026-10-11)

`python3 scripts/validate.py --skip-performance` completes successfully across
all nine runner stages. The workspace and feature jobs report 419 Rust test
executions with zero failures (including repeated executions across features).
The full Cinema integration and adaptive TypeScript business/wiring mutations
pass in this aggregate run. The runner confirms identical source hashes before
and after validation. Retained evidence is
aggregate functional validation (historical evidence is available in Git history).
This invocation does not run coverage or release measurements: the strict
coverage gate remains unfinished, performance is deferred, and the whole Cinema
report still retains its manual and named-suite UNKNOWN requirements.

## Native event-observer rejection and mutation (2026-10-11)

An additional real Bun integration test passes three runs of an owned application
fixture. An undeclared event observer keeps the explicit example in the required
case set but leaves it unexecuted and open. Declaring the real observer permits
the example to pass. Mutating the application's emitted payload from 5 to 6,
while preserving its return value, fails the example and retains the actually
observed payload 6. None of these runs adopts tasks. The focused test passes in
8.63 seconds; engine all-target Clippy passes. This additional test postdates the
aggregate checkpoint above and does not establish a new coverage percentage.

The state-effect suite now passes 24 tests. Missing declared identity fields and
incomplete replacement sources are rejected as UNKNOWN even when actual
before/after rows match. These tests exercise incomplete pinned metadata rather
than asserting new application requirements.

## Current isolated coverage measurement (2026-10-11)

The fresh instrumented workspace run completes with 386 passing Rust tests and
zero test failures, including full Cinema and the new rejection-path tests.
Merged line coverage is 23,390/24,038 (97.30%); raw LLVM function coverage is
1,864/1,927 (96.73%). There are 648 uncovered source lines. Captured Rust input
hashes match before and after the run. See
current isolated coverage evidence (historical evidence is available in Git history).
This supersedes the earlier 96.95%/96.59% measurement for this source revision.
Benchmark conformance remains intentionally skipped, so the measurement is
partial and fails the unchanged 100% gate. No release benchmark or baseline
promotion was performed.

## Escaped loader access and complete Cinema recheck (2026-10-11)

Dependency analyzer 4 retains escaped custom loader reads that analyzer 3 missed.
The initial stricter recheck correctly blocked adoption on Awilix's unresolved
loader access. Observer 2 now instruments opaque reads with the original value
and getter evaluation preserved: executed reads remain incomplete; only actually
unexecuted source-bound reads can discharge a gap. Direct field writes are not
invented loader reads. Harness generator 3 records the changed implementation;
protocol 1 and legacy observer-manifest readability are retained.

All 12 TypeScript target tests pass. Seven real native observer scenarios cover
56/56 helper source lines, with Bun function identities still unavailable. A
native engine test confirms passing factory cases do not adopt tasks when an
escaped loader remains unresolved. The full Cinema integration passes again in
123.09 seconds, including complete provenance, mandatory cases, native adoption
and replay. See opaque-loader evidence (historical evidence is available in Git history).
The earlier Rust coverage measurement predates this production correction; the
strict final coverage and aggregate gates remain open.

The verification crate now passes 104 tests. Additional pinned-plan rejection
cases retain UNKNOWN for non-Boolean postconditions, unmodeled result types and
unsupported clause kinds, without recording those clauses as checked.
Verification-crate all-target Clippy passes. This focused check does not replace
the still-required final workspace coverage measurement.

All 29 Store tests pass after extending independent native-proof rejection.
Content-hash-valid but malformed JSON or wrong typed proof structures cannot
discharge state requirements. A damaged pinned snapshot also prevents proof
reconstruction. The test restores its owned database rows and supplies no native
execution authority; the outer delivery gate still requires complete real cases.
Store all-target Clippy passes. No production source or application task state
is rolled back by these test mutations.

Pure native shrinking now rejects PASS/UNKNOWN as counterexamples, matching the
existing lifecycle guard. A managed worker that accepts only shutdown verifies
that those guards and a disabled step budget invoke no native case. All 36 engine
unit tests pass, and the real failing-operation/minimized-replay integration
passes in 10.77 seconds. Engine all-target Clippy passes. Counterexample generation
and the shrink version remain unchanged for valid failing inputs.

All 25 interpreter tests pass after adding numeric and evaluation-budget
boundaries. Exact 400-digit integers cancel before mean conversion; an
unrepresentable Float mean remains UNKNOWN. List search and mean exhaustion
never produce a partial answer. Unsupported/incompatible sort keys and an
exhausted comparison budget cannot establish an ordering. The source span and
specific open reason are retained. Interpreter all-target Clippy passes; these
are semantic checks, not native performance measurements or a new Rust coverage
claim.

A real Bun initialization integration test passes three invalid adapter-description
scenarios: protocol mismatch, description exception and malformed typed metadata.
Each retains the required case set, executes zero cases, stores readable UNKNOWN
evidence and leaves task event sequences and Open/Unverified states unchanged.
The three-run focused test takes 2.15 seconds; engine all-target Clippy passes.
This covers initialization and protocol-description rejection, without claiming
an additional native build phase or a new aggregate coverage result.

The real pure-counterexample replay integration now rejects five independently
stored imported evidence mutations: absent or ambiguous shrink artifacts,
unrecognized shrink versions, missing matching failing attempts and mismatched
pinned plan occurrences. Minimized contract-value replay is also rejected.
The actual original evidence remains byte-equivalent through the read API; valid
legacy and repaired-production replays still pass. The focused test takes 10.68
seconds. A second replay integration reports missing historical runtime identity
and native limits as new comparison components without adopting tasks or changing
task events; it passes in 17.14 seconds. Format and engine all-target Clippy pass.
These checks add no source-based freshness claim to historical evidence and do
not complete the outstanding aggregate coverage gate.

All four managed native dependency-resolution tests now pass, including an
explicit syntax-failure case. The resolver retains the source-specific syntax
gap and an empty resolution artifact without issuing a native resolution
request; its managed worker shuts down cleanly. The complete engine unit suite
passes 37 tests, and engine all-target Clippy passes. These are transport and
provenance boundary checks, not new application execution authority.

## Batched measurement and Unicode trim correction (2026-10-11)

The next isolated instrumented batch completes with 395 passing Rust tests and
identical before/after Rust input hashes. Merged coverage is 23,596/24,180 source
lines (97.58%); LLVM functions are 1,873/1,934 (96.85%). The unchanged 100% gate
is not met and benchmark conformance remains deferred. Raw retained results are
batched coverage evidence (historical evidence is available in Git history).

After that measurement, independent runatron/Bun reference vectors exposed a
Unicode `trim` discrepancy: Rust trim includes NEL and excludes BOM, unlike the
reference. The failing regression was reproduced before correction. Interpreter
4 now uses the explicit ECMAScript whitespace/line-terminator set. The actual
Cinema spec's CommentText refinement and its real production validator agree on
BOM-only rejection, NEL acceptance and surrounding BOM with visible content.
Neither the spec nor production code was rewritten to obtain this agreement.
All 131 interpreter/verification tests and workspace all-target/all-feature Clippy
pass. The batched coverage above predates this semantic correction and is not
claimed as final-tree acceptance.

## Unicode boundaries through the complete native harness (2026-10-11)

Central value and rejection generators 2 include BOM and NEL boundaries. The
complete actual Cinema run records 5,655 required/executed cases, all PASS, with
41 task adoptions and zero provenance gaps. Its CommentText NEL acceptance and
BOM rejection cases both pass against the actual production validator through
the generated Bun harness. All 27 manual/named-suite obligations remain open and
the whole result remains UNKNOWN. Replay and historical task preservation pass
in the same full integration test (126.26 seconds, not a release benchmark).
See retained Unicode harness evidence (historical evidence is available in Git history).
Earlier 5,552-case checkpoints describe generator 1 and remain historical. The
strict final Rust coverage gate remains unfinished; performance is deferred.

Lifecycle shrink rejection now has the same managed-worker guard coverage as
pure operations: PASS/UNKNOWN require no native requests, and a failing assessment
with zero budget retains the original sequence and reports budget exhaustion.
The worker accepts only shutdown, so accidental execution fails the guard test.
The real lifecycle minimization/repaired-production replay integration remains
passing (8.72 seconds), and engine all-target Clippy passes. This is not a claim
that an unexecuted lifecycle was verified.

## Entity-key continuation (2026-10-11)

Assessment 12 adds the `entity-identity` report case and optional typed contract
identity requirements; compiler IR/snapshot goldens, task definitions, SQLite
schema and native protocol versions remain unchanged. Keys are separately mapped
from factory/field checks. Identity replay runs the pinned operation inputs that
supplied the actual source observations. Contract-only selection cannot claim a
persistence-key proof without populated, complete real operation observations.
Fresh aggregate coverage and final release measurements are recorded separately.


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
