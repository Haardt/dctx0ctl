# Suite and agent evidence implementation checkpoint

Date: 2026-10-11. Assessment: dctx0-assessment-13.

The active objective is complete Cinema validation and zero open tasks. This
checkpoint is implementation progress, not that objective's completed acceptance.
The previous full Cinema result remains the historical 5,660 PASS cases and
46 delivered tasks; a fresh complete run is deferred until the implementation
bundle is ready, as requested by the user.

Implemented:

- Pure reference proof catalog and exact, nonempty, unique suite membership.
- Projection/evaluation of all fifteen existing Cinema Release rules from validated
  semantic nodes and resolved references, without compiler/IR golden changes.
- Examples and Properties derive results from actual pinned native case records;
  dedicated proof, adapter and external categories remain unknown without evidence.
- Typed suite/policy artifacts. Engine discharges only reproduced successful suites
  and policies; Store reconstructs these decisions before native task delivery.
  A violated policy produces FAIL even when individual runtime cases pass.
- Core agent inspection and evidence-basis models with exact binding validation;
  automatic status is retained and an automatic failure cannot be overridden.
- Explicit `verify attest` command and Engine publication with original captured
  input inventory, current source/check/plan/profile guards and exact requirement
  and artifact bindings. Reports are supplied by the reviewer, never fabricated.
- Transactional inspected task delivery using normal start, structured review,
  complete and check events. Review approval is mandatory for review tasks and
  unresolved high/action findings block inspected PASS. Automatic native adoption
  still cannot complete reviews through this separate explicit path.

Focused verification includes six pure Rust tests, actual native example/production
mutation and Release contradiction checks, a bound agent-delivery test preserving
old UNKNOWN evidence, and a review-batch rollback test. The latter reaches zero
open tasks in its small independent fixture; this is not whole Cinema acceptance.
Affected Engine/CLI compilation and strict Clippy were checked during development.
Full workspace, fresh coverage, complete Cinema and release measurements remain
scheduled for the end. Memory guarding and the user-authorized 97% gate remain.

Next implementation work: dedicated production-adapter and real Hono/Awilix
integration evidence, architecture/ownership checks or explicit justified
inspection, complete Cinema review reports and effective suite/Release aggregation
with pinned agent evidence. Then verify actual Cinema task state, not just report
counts, and run the bundled final acceptance.

Compatibility: report schema 1, task report schema 2 and SQLite schema 4 remain;
new inspection/report artifacts are additive. Historical native results are not
rewritten. Review delivery retains existing event/version/claim and profile rules.

## HTTP and effective-program continuation

The testing adapter now has fourteen explicit reviewed HTTP cases covering all
twelve Hono routes, including closed-vote failures. These cases reuse existing
spec-driven operation/setup expectations and actual Awilix/SQLite sessions;
Rust still assesses returned values, states, events and effects. A helper maps
native arguments to application-owned HTTP routes and translates actual responses,
without rebuilding results. Two focused Bun tests and normal TypeScript checking
pass. The complete native acceptance now includes a broken Hono-route replay
mutation and is intentionally deferred to the bundled final run.

A pure effective-program assessment now combines bound suite/manual inspections
and structured review findings with deterministic Release rules. It retains
automatic results and evidence basis, rejects missing execution and stale bindings,
and does not invent zero high findings. A focused reference test covers effective
PASS, absent/stale reviews, high findings and preserved native failure. Its Engine
publication/CLI integration and actual Cinema inspections remain pending.

## Stored inspection continuation

`verify inspect <spec-root> <native-run-id> --project <app-root> --file <reports.json>`
now stores the effective program assessment and its bound inspection reports.
The new report references the original native run and reuses its observations;
its origin is Imported and elapsed execution is zero, so it cannot authorize
native automatic adoption or fabricate a new execution. The original report stays
unchanged. Repeated publication of identical inspections is idempotent. Source,
dependency, spec occurrence, task plan and profile drift block current publication.

The focused real-native review-batch test verifies stored effective PASS, preserved
native UNKNOWN, exact reused cases, idempotency, no task mutations and refusal after
production source changes. Three focused TypeScript tests exercise HTTP mappings
and durable SQLite persistence across real owned session lifetimes, including
replacement counts and unchanged state after rejected closed-poll votes. The full
validation runner now includes normal TypeScript checking and all Cinema source
and testing-only conformance tests before coverage/performance. Full native Cinema
execution and fresh aggregate coverage remain deferred until the implementation
bundle and concrete inspections are complete.

Code review found a production timestamp acceptance gap: JavaScript Date.parse
normalizes impossible dates. A shared application-owned calendar validator now
checks month/day/leap-year, time and offset bounds directly, preserving precision,
leap seconds and the wire text. Its accepted syntax follows the pinned chrono
RFC3339 parser used by Rust, including lowercase separators and a single space.
Focused positive/negative tests and strict TypeScript checking pass. This is a
production repair, not a spec change or a new assertion inferred from free text.

## Concrete Cinema completion checkpoint

The guarded `scripts/cinema_complete_acceptance.py` validates authored review
source hashes before binding their judgements to current native evidence. It
creates a real active task plan, executes the entire Cinema scope, records the
explicit effective inspection and delivers remaining inspected tasks through
normal transactional guards. The actual reference project's plan has 104 tasks,
all Done/PASS, with zero open/non-PASS tasks; all 5,674 mandatory native cases pass.
See `cinema-complete-validation.json` and the retained SQLite evidence. These are
actual task states, not an empty task-plan count. Agent evidence remains explicit.
The no-eligible-vote rationale is recorded as a low, non-action-required finding;
the executable None outcome is preserved.

The first aggregate found four older isolated fixtures missing newly required
HTTP/timestamp files; they were corrected and all 21 focused native tests pass.
The second aggregate has every functional test and all 92 release measurements
complete, but function coverage is 96.90% versus the authorized 97% gate. Two
meaningful suite/policy rejection/UNKNOWN checks now cover the identified gaps;
the third final aggregate is running. No complete-validation claim is made yet.
Accepted benchmark baseline remains unchanged.


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
