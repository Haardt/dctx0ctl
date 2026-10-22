# Pinned work packages and workflow measurements

The Company Manager audit identified context traffic and agent coordination as
investigation priorities. Its categories are attribution proxies, not causal
costs; no historical task throughput can be inferred without a starting pin.
The following additive APIs support bounded handoffs and measurable future runs.
Existing task definitions, compiler goldens, verification reports and SQLite
schemas are unchanged. No evidence-reuse shortcut or new adoption authority is
introduced. Existing immutable native reports remain the source of execution evidence.

Compatibility: `tasks --as-of` is now rejected for mutations, including planning;
`--from-plan`/`--from-event` are accepted only for progress queries. Explicit help
requests now exit successfully on stdout. Existing unfiltered verification JSON
and stored task definitions retain their established shape.

## Export one actionable package

```sh
dctx0 tasks package path/to/spec T42 --project path/to/app --json > package.json
```

The anchor selects its actual prerequisite closure, including delivered boundary
results, other workers' ownership and external blockers. `body.data` contains:

- `id`: deterministic content identity of the complete export.
- `pin`: project, plan, snapshot, exact check occurrence and task-event sequence.
- `spec_source_hash`: source identity of the pinned published specification.
- `slice`: tasks, prerequisites, boundary results, open work and blocker reasons.
- `tasks`: requirements/work-order hashes, ownership/state, profile attribution,
  result artifacts/decisions, evidence IDs/binding hashes and references into `content`.
- `content`: complete core context items keyed by content hash, including typed
  contracts and original declaration bodies. Shared exact type declarations
  are normalized to one content item; shared profile prose is also stored once.
  Each task keeps its references and `profile_subjects` attribution map.
- `complete` and `limitations`: absent implementation profiles or incomplete
  mandatory context remain explicit. No token count is invented.

There is no silent pagination/truncation of mandatory package text. Existing
`tasks context --budget ... --cursor ...` remains available for incremental
reading. Package creation reuses snapshot-local context and overview indexes.
Profiles frozen by claims/reservations retain their original context; open work
can select current editable profiles. Local profiles never rewrite task definitions.

Package reads do not check source freshness, create claims, reserve execution,
replan, migrate history or adopt work. Use existing atomic `claim-slice` and task
transition commands when execution is authorized. The package's `slice.closed`
and `complete` fields answer different questions; neither proves implementation.

## Resume using a delta

```sh
dctx0 tasks package-delta path/to/spec T42 --project path/to/app \
  --file package.json --json > delta.json
```

The previous file may contain a direct package or the CLI `body.data` envelope.
Its content identity/references are validated. Comparison requires the same
project and nondecreasing event pins. New/removed task IDs, historical links and
changes to requirements, assignments, prerequisites, profiles, state and evidence
are explicit. `reload_context` also considers changed prerequisite-result text.

`added_content` transmits only new bodies; `removed_content` retires cached ones.
The after-task references, slice, pins, source hash, completeness and limitations
are retained so a consumer can reconstruct and validate the after-package.
An unchanged context body need not pass through the model again. To continue
another delta comparison, retain/reconstruct the full package, not just a delta.
Source drift in application files still requires relevant implementation inspection.

## Read compact native diagnostics

```sh
dctx0 verify show path/to/spec RUN --summary --json
dctx0 verify show path/to/spec RUN --outcome FAIL,UNKNOWN --limit 50 --json
dctx0 verify show path/to/spec RUN --subject Shop.Catalog --limit 50 --json
dctx0 verify show path/to/spec RUN --case CASE --json
```

Filtered results retain the complete saved run's result, mandatory/recorded
counts, open obligations, provenance gaps and aggregate issue counts. Rows carry
case ID, subject, result and observation count. Only an exact case request adds
its complete bindings/observations. A missing exact case is an error.

Continue with `--cursor '<next_cursor JSON>'` and the same filters/limit.
Cursors bind run, outcome/subject/case filter and limit. `complete` refers to the
selected diagnostic listing; `summary_only` explicitly marks omitted rows.
Filtering never converts an incomplete run into PASS. Legacy unfiltered show
and its full JSON shape remain unchanged. Summaries of pruned runs explicitly
retain unavailable detail/replay status. Compact history queries avoid corpus
loading; filtered diagnosis of a retained run currently decodes its saved corpus.

## Explain adoption before delivery

```sh
dctx0 verify adoption-preview path/to/spec RUN --project path/to/app --json
```

The report describes every active task: eligible in this proposal, outside the
saved scope, historical/stale evidence, incomplete cases/provenance, missing
requirements, changed profiles, claims/reservations, dedicated reviews or other
concrete blockers. Global issues are emitted once and block every task; per-task
blockers retain task-specific reasons. The engine uses the native publication proof evaluator and
checks the current input inventory, specification, runtime and profile binding.
Pruned/imported evidence cannot authorize new adoption. Existing current claims
and Done tasks remain protected.

Preview never executes application code, publishes task events or rebases an
old task pin. It cannot authorize a later mutation. Ordinary verification and
explicit inspection/delivery workflows still perform their transactional checks.
Changes after the preview may invalidate its answer.

## Measure progress and external usage

Save the starting package's plan/event pin before work. Then export:

```sh
dctx0 tasks progress path/to/spec --from-plan 12 --from-event 240 --json > progress.json
# Optional historical endpoint: --plan 14 --as-of 310
python3 scripts/workflow_metrics.py --progress progress.json \
  --package package.json --usage usage.json
```

Progress reports new/removed active work, Done and PASS endpoint transitions,
new tasks already Done/PASS, removed/lost active PASS, unfinished/blocked work,
intervening events and new historical links. It does not rewrite historical
states. Work created/removed entirely within the interval remains visible in the
exact events even when absent from both active endpoint plans. No feature
completeness or current implementation freshness is inferred from these counts.

The optional repository-owned Python analysis tool is external to product CLI
execution. It joins exported packages to supplied usage by package ID. Usage is
a JSON array with `response_id`, `package_id`, `input_tokens`,
`cached_input_tokens` and `output_tokens`. Duplicate response IDs, unknown
packages, invalid counts and foreign/out-of-interval pins are rejected. Reasoning
is already part of output. No billing estimate or provider-specific integration
is assumed. Unattributed usage remains explicitly unmeasured.

Optional `--receipts receipts.json` accepts a JSON array of external execution
receipts. Each requires `run_id`, `package_id`, `outcome` (PASS/FAIL/UNKNOWN),
`mandatory_complete` and `source_current` booleans, timezone-qualified
`started_at`/`finished_at`, and an `identity` object with hashes named
`spec_sources`, `implementation`, `dependencies`, `adapter`, `configuration`,
`scope` and `inputs`. The `inputs` identity must include effective harness,
runtime/verifier/interpreter/generator versions, seeds, budgets, profiles and
execution inputs before treating repeats as comparable. A complete current PASS
may supply `delivered_at`.
The tool reports identical-input repeats, first-pass yield by package,
FAIL-to-current-PASS intervals and median/nearest-rank-p95 evidence-to-delivery
latency. Overlapping checks use result completion order for repair intervals;
first-pass yield uses attempt start order. These external assertions never become
native proof or task status.

Compare equivalent package types/scopes, keeping mandatory cases and acceptance
constant. Task endpoint transitions per million fresh input tokens are labeled
as historical transitions, not accepted feature throughput. Missing data yields
null metrics, never fabricated zero-cost success. The audit's proposed 25%
improvement remains a hypothesis until comparable new runs establish it.

## Validation and release measurements

```sh
python3 scripts/validate.py --coverage --samples 1 --iterations 1
```

The coupled runner includes `tasks-package`, `tasks-package-delta`,
`tasks-progress`, `verification-diagnostics-summary` and
`verification-diagnostics-cases` release phases. Diagnostic phases use a clearly
synthetic 1,000-case UNKNOWN corpus, without native execution claims. Package
phases use the actual compiled Cinema task closure; absent benchmark profiles
remain explicit. Runtime excludes phase preparation; process VmHWM includes it.
Native compiler/harness costs remain in separate Cinema measurements. Raw samples,
source/toolchain provenance and the latest 20 run records retain the existing
storage and baseline rules. These new workloads are unassessed until an explicit
baseline promotion; no baseline change is implied by implementing them.

The 2026-10-21 checkpoint passes 578 Rust and 38 Python tests, with 97.43% merged
source lines and 97.05% LLVM functions. All 78 release workloads complete on stable
sources with one development sample/iteration; baseline comparison is unassessed.
The local Go race probe remains unavailable on its ARM64 VMA host, so aggregate
local validation stays partial. Publication requires fresh full CI and all five
platform gates. `docs/workflow-efficiency-validation.json` retains the exact
results, prior failures and provenance.

## Apply a saved delta

```sh
dctx0 tasks package-apply --file package.json --delta delta.json --json > next-package.json
```

This command needs no spec root, application, database or native toolchain. Both
inputs accept direct objects or the established `body.data` CLI envelope. It
reconstructs the complete next package, verifies source and target identities,
ordered project pins, content references and explanatory delta metadata, then
prints the direct package with `--json` (or `--format json`). The default is the
complete Markdown package view, following the other task commands. Keep the full
JSON output for the next `package-delta`.
Do not use the same file as shell input and output: shell redirection truncates
it before the command reads it.

Only added content and tasks marked `reload_context` need reloading into a retained
agent context. Shared constraints, acceptance and types stay complete. An agent
without the retained context still needs the referenced full bodies. `tasks show`
and its complete requirements remain available with the existing output shape.

## Prepare a source-bound review renewal

```sh
dctx0 verify review-changes path/to/spec --project path/to/app --task T42 --json
```

The read-only report compares the selected task's latest stored review with its
pinned requirements and current application inventory. It reports changed, added
and removed files with old/new hashes; changes to requirements, snapshot and spec
sources; the complete affected duty group; and historical findings. A missing or
legacy review is explicitly noncomparable. A later rejection or actionable finding
is never replaced by an older approval. When a renewed task has no report, explicit `ReviewRenewal` ancestry supplies
the nearest predecessor's latest judgement. `previous_task` identifies that
source; unrelated history is never selected and approval is never inherited.

`template` contains a fresh empty request-changes report suitable for substantive
renewal; it never inherits approval text or reviewer identity. Spec drift instead
produces a blocker and no template. Check and publish current task requirements
explicitly before renewing. The command writes no events and supplies no new
execution evidence. Existing inventory-wide review acceptance rules remain intact;
there is no inference from one changed file to a particular duty and no automatic
review-hash refresh. This API handles stored dctx0 task reviews, not consumer-specific
inspection JSON formats.

The coupled runner adds `tasks-package-apply`, `tasks-review-changes` and
`verification-codecs` to the release phases. Package apply uses a prepared complete
Cinema package and an unchanged delta, including serialization of the result.
Review comparison uses a synthetic source-bound report constructed from Cinema
requirements and source hashes; it measures the pure comparison, not filesystem
capture, review reasoning or native execution. Core measurements report post-exec
VmHWM including preparation; phase runtime excludes preparation. See
[codec scaffolds](codec-scaffolds.md) for generation scope and configuration.

## Tooling validation checkpoint — 2026-10-22

The final coupled run passes 586 regular all-feature Rust executions plus the
separately deferred benchmark check, 587 instrumented Rust executions, 38 Python
tests and 77 Bun codec cases. The isolated release CLI-to-Bun check passes nine
representation scenarios. Go and Rust Cinema each pass all 5,674 required cases,
five meaningful production mutations and repaired replay. Local aggregate status
is **incomplete** solely for `RACE_HOST_VMA_UNSUPPORTED`; the unavailable native
race detector is not replaced with ordinary Go tests.

Fresh isolated Rust coverage is 35,646/36,587 merged source lines (97.43%) and
2,896/2,984 LLVM functions (97.05%). Raw LLVM lines are 96.67% and raw regions are
94.22%; neither establishes complete branch coverage. The 97% thresholds remain
unchanged and no exclusions were added. TypeScript helper coverage is separate:
217/294 source lines and 31/36 functions across the two generated configurations.

The 81 release workloads complete in
`benchmarks/history/20261022T011947.427072Z-18852.json`. New phase samples are:

| Phase | Elapsed ms | Peak RSS bytes |
| --- | ---: | ---: |
| `tasks-package-apply` | 2.761231 | 29,409,280 |
| `tasks-review-changes` | 0.007384 | 27,803,648 |
| `verification-codecs` | 0.269230 | 11,059,200 |

Runtime excludes preparation; RSS includes it. The review phase measures a pure
comparison of a synthetic report, not filesystem capture or review reasoning.
These reduced development measurements use one warm-up, sample and measured
iteration. The baseline is incompatible/unassessed and unchanged; no regression
or throughput improvement is inferred. Recent comparable history remains in the
raw record, with the newest 20 runs retained.

The serial QA run uses test/dev optimization level 1 with debug assertions and
overflow checks enabled, no debug symbols and no incremental cache. The sampled
process tree peaks at 1,065,418,752 bytes against a 2,866,156,885-byte budget.
Release measurements remain uninstrumented and use the ordinary release profile.
`docs/workflow-tools-validation.json` records the exact environment, complete
checks, unchanged source pins, baseline hash and raw measurements. The two earlier
intentionally interrupted runs are explicitly incomplete and supply no baseline.

## Workflow follow-up validation (2026-10-22)

Codec generator version 2 adds `decodeInput` for contract-invalid native shapes;
strict `decode` remains available. Review comparison follows explicit renewal
ancestry and publishes `previous_task`, without copying approval or emitting events.

`python3 scripts/validate.py --focus workflow-tools --samples 1 --iterations 1`
passes 38 Python tests, seven focused Rust tests, one deferred benchmark test,
strict TypeScript checking and 79 Bun cases, alongside workspace check/build,
formatting and Clippy. This is targeted development validation, not a replacement
for full workspace coverage acceptance. All 81 release workloads and separate
native costs completed. The guarded run took 700,382.502 ms with peak sampled
process-tree RSS of 929,284,096 bytes against a 3,604,647,936-byte limit. Reduced
samples remain unassessed against the accepted baseline, which was not promoted.
`docs/workflow-tools-followup-validation.json` retains full measurement provenance.
