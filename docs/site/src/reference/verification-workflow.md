# Verification workflow diagnostics

The 2026-10-24 increment adds structured cause projections, metadata comparisons
and external usage attribution. It does not implement the entire eight-part
workflow proposal.

`verify show SPEC RUN --summary --json` includes cause aggregates for newly saved
runs. These counters cover non-PASS recorded cases; run-level open obligations
and provenance gaps remain separate fields and can keep the aggregate UNKNOWN
even with no failed or unknown cases. Historical summaries without the field report null. `verify show SPEC RUN
--cause adapter,environment --limit 20 --json` filters diagnostic rows without
changing the run result. Exact-case diagnostics contain issue code, subject,
source span when present, attribution and next step. Unknown issue codes remain
unclassified; message text never establishes a cause.

New ordinary application exceptions are UNKNOWN. Existing saved FAIL exceptions
remain FAIL. Actual false returns and observed contract violations retain FAIL.
The assessment version is now `dctx0-assessment-17`. TypeScript adapters can throw
`AdapterError` or `EnvironmentError`; Go adapters can return corresponding wrapped
errors. These testing helpers use the existing technical-error envelope. They
never require production code to import dctx0. Explicit environment errors are
UNKNOWN. Missing requested entity keys are adapter failures; an explicit empty
entity list is a witnessed empty state.

`verify compare SPEC BEFORE AFTER --limit 20 --offset 0 --json` compares immutable
indexed metadata, without decoding observation bodies. Requirements are compared
by exact plan identity, generator/interpreter versions and budgets. Added and
removed mandatory case IDs are separate from issue counts. Resolution requires a
matching newly executed native PASS; removed cases, changed requirements, pruned
case bodies and reused inspection observations are unproven. Explicit query
preparation is required for older unindexed runs. This is historical comparison,
not source freshness or authorization to publish task results.

`scripts/workflow_metrics.py` accepts optional response correlation IDs, explicit
activities, nullable token counts and declared cache/reasoning semantics. These
are external assertions. The CLI does not yet produce correlation IDs or phase
traces. See the bundled workflow-measurement reference for the field contract.

Still pending from the proposal: shared native clause/scenario continuations,
preflight probes, registered external source roots, common closure preview,
complete separate inspection/native obligation aggregates, CLI phase telemetry,
and the isolated Identity/Access acceptance. Existing adoption, inspection,
completion and source-inventory guards remain authoritative.

The coupled checkpoint validation completed on 2026-10-25 (Europe/Berlin):
627 Rust test functions, 40 Python tests, the supported CLI feature matrix and
native Cinema mutation/replay acceptance passed. Merged source coverage is
38,344/39,492 lines (97.09%); raw LLVM function coverage is 3,139/3,234 (97.06%).
This is not branch completeness. All 81 release speed/RSS workloads completed;
one sample/iteration and one warmup leave general baseline comparisons unassessed.
The accepted baseline is unchanged. See `docs/verification-workflow-validation.json`
for pinned source hashes, raw evidence, measurement provenance and outstanding work.

## Constructive state preflight (2026-10-27)

`verify plan` now reports `STATE_REPLACEMENT_SOURCE_REQUIRED` for replacements
without an explicit complete selector/source shape, and
`STATE_OBSERVES_WRITE_CONFLICT` when an operation declares both read-only
`state observes` and write access to the same resolved entity. Messages retain
the operation identity and state-clause source position and explain the remedy.
These are open plan obligations, not evidence of a failed native implementation.
The native state assessor still requires actual observations and never infers a
replacement from observed state or postconditions.

For compatibility, compilation and historical snapshot IR remain unchanged;
selector-only historical specifications still compile, but their verification
plan explicitly exposes the missing constructive source. Plans are read-only
and do not rewrite snapshots, tasks or historical results. `observes` still
requires unchanged successful state. Write access and after-state postconditions
do not give it mutation semantics. See the [state language guide](../docs/language/control-and-effects.md#state-on-a-port-operation)
for complete replacements using inputs, explicit results and before-state fields.
The WorkshopHub tutorial now supplies full sources for publication/cancellation.


The 2026-10-27 follow-up reviews the expanded finite Cinema corpus explicitly:
`dctx0-values-6`/`dctx0-rejections-3` and the additional candidate-film/poll setup
produce 5,766 mandatory cases (3,862 contract, 1,784 operation, 101 assertion,
12 example, five entity-identity and two lifecycle cases). The six native
acceptance/performance entrypoints share exact case-family expectations and
still require every requested case PASS with complete native provenance.
This intentionally updates the former 5,674-case acceptance inventory; compiler
snapshot goldens and production/specification files remain unchanged.

The changed testing fixtures were inspected separately: the new castVote setup
saves the selected film and opens its referenced poll, identically for all three
targets. Only that reviewed fixture hash and its inspection scope are renewed;
other authored findings remain. The review is not native testing evidence.
The State-result regression now checks the explicit pending oracle request and
verifies that application call traces cannot substitute for its independent
continuation tape or silently discharge an unsupported state projection.


Follow-up acceptance completed on 2026-10-28: the full workspace/feature checks,
647 regular Rust tests, 648 instrumented executions, 41 Python tests, qualified
Go Race and native Cinema mutation/replay acceptance pass. Coverage is 97.10%
merged source lines and 97.04% LLVM functions, with unchanged thresholds and no
new exclusions. All 82 release speed/RSS workloads complete; the additional
standard sampling retains five samples/20 iterations for core measurements.
Baseline comparison remains explicitly unassessed due to incompatible metadata;
the accepted baseline is unchanged. Full receipts and the preserved failed
checkpoint are recorded in `docs/state-contract-followup-validation.json`.
