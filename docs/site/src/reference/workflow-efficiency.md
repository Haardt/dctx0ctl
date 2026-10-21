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
