# Project status and execution preflight

`status` answers the current task/evidence/source question without SQL, scripts,
an export, a query-index preparation, or a report directory. It performs reads
only; it does not check specifications, update tasks or authorize publication.

```sh
dctx0 status SPEC --project APP
dctx0 status SPEC --project APP --json
dctx0 status SPEC --since-plan 12 --since-event 340 --json
```

These commands are implemented in the development source. Build with
`cargo build --release -p dctx0-cli --all-features --locked` and use
`target/release/dctx0` until an appropriate release is installed. This increment
does not replace an installed executable or change runatron-business.

## What status means

The report includes historical Done/PASS, Done without PASS, Open, Progress and
blocked counts. Blocked counts overlap Open/Progress. It distinguishes current,
stale and unknown source bindings for historical Done/PASS tasks. Those three
counts partition Done/PASS; unknown is never counted as current. Native evidence
must match the saved check reference, requirement hash and implementation proof
hash before its source binding can be classified. Imported/authored reports,
legacy records without the compact evidence index and pruned details remain
unknown. None of these counts changes task history.

Specification freshness compares current spec bytes with the published check.
Task-plan freshness compares its snapshot and exact check occurrence with the
published head. Native source freshness compares the exact captured-input
inventory under the configured capture rules, including adapter/configuration
inputs. It does not assert private fixture availability, current runtime identity,
review approval or application acceptance. A changed source keeps the historical
Done/PASS result and marks its source binding stale.

The latest relevant run matches the configured target and overlaps the configured
subjects (including task-selected subjects and namespace containment). The
report gives its actual required/recorded/missing cases, PASS/FAIL/UNKNOWN counts,
result issue, open obligations and diagnostic cause counts. A narrow run or
replay is still a narrow run; its counts do not certify the entire application.
Without configuration, the newest saved run is shown without an inferred scope.

Unreadable configuration or source inventories produce structured status issues
and unknown freshness while preserving available task counts. Stored provenance
integrity errors still fail explicitly. Modern indexed histories avoid case/report
expansion; deriving summaries from legacy unindexed histories can cost more.
Status never migrates or prepares those indexes implicitly.

`task_event_pin_current: false` can be normal after the run itself delivered
tasks. `requirements_current` is separate and compares actual task requirement
hashes/check occurrence. Status grants no exemption from publication's event,
ownership, profile, runtime or source guards.

Progress uses the preceding relevant run's saved task pin by default. Supply
`--since-plan` and `--since-event` together for a chosen measured interval. The
report separates transitions of existing tasks from tasks added already Done or
PASS, and reports lost PASS separately. With no usable baseline, the delta is
unknown. A retained compact header can supply a historical baseline after case
pruning; it cannot authorize new delivery. Status does not save an implicit
last-query cursor.

At most three next causes identify concrete check/plan/pin/source/native evidence
or prerequisite problems. They are diagnostic next steps, not inferred business
priorities. A concurrent task event/head change is flagged; query again before
acting. Filesystem reads are observations during the query, not a transaction
across the application's files.

## Preflight commands

```sh
dctx0 verify preflight SPEC --project APP
dctx0 verify preflight SPEC --project APP --task T31 --json
dctx0 verify preflight SPEC --project APP --bindings bindings.json --json
dctx0 verify preflight SPEC --project APP --diagnostic --json
dctx0 verify preflight SPEC --project APP --offset 20 --limit 20 --json
```

Preflight reuses the published verification projection, configured fixtures,
budgets, central mandatory case manifest and the existing task/profile pinning.
It never creates a second case generator. Its compact response contains issue
counts and a page of positioned blockers/warnings, the mandatory case count,
required capabilities and a page/count of potential task deliveries. The same
offset/limit selects each list; limits are 1–200. `--full` explicitly requests
the entire preflight report, not the complete ProgramPlan.

Potential tasks are unclaimed Open contract/operation tasks with pinned profiles
and a mandatory native mapping for all needed task requirements. Authored intent
context is treated exactly as in the native publisher. The list is conditional
on actual successful execution, observations, reviews and transactional guards;
it is neither evidence nor a reservation. A known global blocker clears the
potential delivery list. Review/retirement/adapter/integration workflows remain
separate.

Known blockers include missing/mismatched task context, missing profile bundles,
non-executable planned obligations and absent/invalid reviewed lifecycle inputs.
Planning-time `STATE_OBSERVER_REQUIRED` hints from generated input eligibility
are runtime requirements rather than proof that an observer is absent. They
remain warnings in preflight. Only the unchanged native resolver can discharge
them, after every corresponding generated case has valid actual before-state
evidence and satisfies its precondition. An explicitly missing observer binding
still blocks delivery. The whole-Cinema preflight regression retains all 5,766
mandatory cases and distinguishes these two situations.
An optional authored schema-1 `BindingDescription`, read using the existing
strict binding reader, checks required operation/validator/entity-observer roles
and declared effect observers. Missing role descriptions are explicit. A
description confirms neither a native symbol/type nor a real production call.

The existing schema-1 verification configuration has one additive optional field,
`adapter_scaffold_hash`. New `verify init` configurations remember the hash of
the generated unavailable adapter. An unchanged placeholder is a known blocker.
Existing files/configuration are preserved, and older configurations without the
field remain valid. Editing a placeholder only removes that exact negative
identification; it does not establish that a binding works. The mandatory native
assessment still decides the outcome.

Backward reading is supported by the new tool; an older strictly validating CLI
can reject the new optional field in a newly generated configuration. Keep the
CLI/configuration versions together when adopting this development increment.
There is no automatic downgrade rewrite of an existing configuration.

Without an authored binding description, static binding/observer coverage is
unknown. Private fixture readiness is also unknown until authored runtime setup;
no database key is inferred and no provisioning or environment mutation occurs
in preflight. These are reported warnings, not fabricated confirmations.
An authored fixture-readiness probe and native symbol conformance remain future
work; static preflight does not claim to discover every adapter defect.

## Execution intent and changed inputs

`verify run` performs the same preflight automatically. Its default intent is
task delivery: a missing task pin, unchanged generated placeholder, known
lifecycle gap or absence of possible native deliveries stops before build/worker
startup. No verification run is recorded for this refusal.

```sh
# Delivery: known prerequisites must be satisfied.
dctx0 verify run SPEC --project APP --task T31
# Explicit diagnostic evidence, even with known delivery blockers.
dctx0 verify run SPEC --project APP --no-adopt
# Equivalent explicit intent spelling.
dctx0 verify run SPEC --project APP --diagnostic
```

Historical checks are diagnostic. A diagnostic run still uses the complete
selected mandatory plan, the same budgets and central result rules. It can fail
or remain UNKNOWN; it never attempts task delivery. Invalid reviewed cases and
other fatal input/configuration errors remain errors rather than runnable plans.

Immediately before native generation/build/start, the guarded execution path
compares the captured source-inventory, snapshot, occurrence, plan and task/profile
pins with preflight. Changes yield `PREFLIGHT_INPUTS_CHANGED` or
`PREFLIGHT_TASK_PIN_CHANGED`, requiring a new preflight without native execution.
The existing post-execution source/provenance checks and transactional publication
guards remain in force. Normal text output shows preflight before execution;
JSON run receipts include its compact report. For a JSON planning response before
a potentially long execution, invoke `verify preflight` explicitly.

`verify complete` also uses the guarded path after its explicit check/profile/task
publication. If native contract/operation tasks are already Done, any needed
native revalidation is explicitly diagnostic; source-bound review/integration
delivery remains separate. Known execution blockers still prevent startup.
Replay and generated binding tests are already explicitly diagnostic workflows;
they retain their historical-input/pin checks. Lower-level engine run APIs remain
compatible and retain their existing explicit adoption flag/publication guards;
new consumers should use the preflighted path.

## Validation and measurement

```sh
python3 scripts/memory_guard.py python3 scripts/status_acceptance.py
python3 scripts/validate.py --coverage
```

The focused runner couples engine/CLI regressions with release CLI speed and RSS
for both mandatory Cinema spec copies, using fresh isolated histories, one warmup
and three samples by default. Its release scopes are status/planning with an
unavailable adapter, not complete application acceptance. Native engine
regressions separately execute real production validators and guarded task
delivery. Measurements retain commands, source hashes, output bytes, toolchains
and memory-method limitations under `benchmarks/history/status` (latest 20,
locked/atomic writes). The workspace validator runs the release portion after
its normal tests. New workload baselines are unassessed and never promoted by
these commands.

Regressions distinguish Done from PASS, preserve authored evidence as unknown,
detect source/task changes before worker startup, reject an unchanged scaffold,
show a real adapter abort/missing cases, and select a relevant earlier run over
an unrelated newest run. A deliberately corrupted case payload does not force
status to expand it; full evidence retrieval rejects it. Pruned/missing contexts
and corrupt provenance are distinct. This is not authority to adopt evidence
without its full required integrity/provenance checks.

The corrected 2026-11-07 workspace coverage run covers 42,925 of 44,223 merged
source lines (97.065%) and 3,479 of 3,582 LLVM functions (97.125%). Both unchanged
97% gates pass on identical before/after compilation inputs. Raw LLVM lines are
44,970 of 46,662 (96.374%); raw regions are 93.418%. The established merging
policy is unchanged, no new exclusion is introduced, and branch completeness is
not measured.

The final coupled workspace run passes all 35 validation steps and completes
88 release workloads in 3,895,588.849 ms. Sampled process-tree peak RSS is
1,075,568,640 bytes (50 ms sampling, shared pages counted per process).
Its performance comparison remains explicitly unassessed and the accepted
baseline is unchanged. Full results and identical before/after source pins are
retained in `benchmarks/history/20261107T170945.545000Z-3044904.json`;
`docs/status-preflight-validation.json` links the workspace, coverage and focused
measurement evidence.

An earlier full attempt is explicitly failed/incomplete in
`benchmarks/history/20261107T154832.707263Z-2722966.json`: preflight had mistaken
the two runtime state hints for established missing observers and prevented the
Go/Rust acceptance paths. The corrected-source component acceptance then passes
both complete applications, with 5,766 cases, 105 tasks, five production mutations
and replay per target. The fix changes prerequisite classification only; native
state checks, case selection and publication guards are unchanged.

The final-source focused measurement retains eight workloads (three samples
after one warmup). On the fresh isolated TypeScript/Go Cinema histories, status
takes 101.417/101.462 ms with 87,674,880/87,744,512 bytes peak command RSS;
whole-plan preflight takes 384.117/384.670 ms with
112,881,664/112,680,960 bytes. These status histories have no native run, while
the separate engine regressions exercise saved native evidence. Command RSS is
wait4's child/reaped-descendant high-water mark, may include inherited pre-exec
memory and is not collective simultaneous RSS. New workloads remain unassessed
against a baseline; these measurements do not claim a speedup. See
`benchmarks/history/status/20261107T163317.397090Z-3276882.json` and
`docs/status-preflight-validation.json` for the final validation index.

## Remaining delivery-workflow increments

This implements the status/preflight increment; it does not claim completion of
the whole six-part improvement request. The existing compact diagnostics, paging,
explicit exports and native adoption guards are reused, not newly invented here.

1. Add authored fixture readiness and target binding conformance to preflight
   without executing business operations or manufacturing production bindings.
2. Confirm the specific EventJournal panic with a native regression, then add
   typed case-isolation guarantees, independent worker recovery, dependent-case
   blocking and durable checkpoint/resume. Current execution can still abort its
   remaining cases; a status report does not repair that behavior.
3. Expose exact per-task mandatory coverage and a guarded saved-evidence delivery
   workflow. Missing historical pins cannot simply be synthesized after execution;
   requirement/profile/source compatibility must be proven. Current adoption
   rules and historical results are unchanged.
4. Extend metadata aggregation by subject/issue where existing diagnostics are
   insufficient, keeping default responses bounded.
5. Extend the existing Go/TypeScript/Rust generators from representations,
   assertions and central test scaffolds to typed dispatch and required observer
   interfaces. Real production bindings remain authored and unavailable until
   implemented.

Measure those increments by command count per diagnosis, blockers detected before
native startup and measured time to evidenced task delivery. Do not substitute
case counts, reduced code size or this focused runner for application acceptance.
