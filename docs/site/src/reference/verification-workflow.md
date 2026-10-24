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
