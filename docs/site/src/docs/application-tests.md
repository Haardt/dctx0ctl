# Application-owned tests and task delivery

Go and TypeScript/Bun application tests call production directly while Rust owns
the specification, mandatory cases, deterministic inputs and assessment. The
existing adapter path remains supported; choosing a backend does not change the
required case IDs or budgets. Application protocol 2 and library 0.2.0 are
separate from the historical worker protocol and SDK.

The workflow below describes guarded task delivery and complete acceptance.
Its integration and final repository validation are in progress; these
instructions do not claim a successful release validation.

## Connect real tests

Publish the shipped testing-only package:

```sh
dctx0 verify application-tests library --project APP --target typescript
# Or: --target go
```

The package and version/source pins live under `.dctx0/application-tests/`.
Publication refuses edited owned files and preserves authored tests and
production dependencies. Production imports no testing package. Publishing the
library alone supplies neither production bindings nor execution evidence.

Author the native root tests, typed local production connections, fixtures,
observers and `native-run.json` recipe. Cinema includes a generated typed facade,
real SQLite/HTTP checks and explicit asynchronous contexts for Bun. Missing
observers and unsupported native mappings remain incomplete. Prepared mocks
form a closed scope: every injected call is checked centrally, including
unexpected subjects; actual real/setup scopes remain real.

The recipe runs frozen dependency preparation, test builds, collection and native
execution inside captured sources. Ambient node_modules are not executed. An
offline build requires the locked dependencies to be available locally; install
with the project's frozen lockfile beforehand when populating a cold cache.
Native process, stream, cancellation and cleanup limits remain active.

## Prepare the current plan

For the authored Bun Cinema suite:

```sh
dctx0 verify application-tests prepare examples/cinema/dctx0 \
  --project examples/cinema --plan .dctx0/application-tests/plan.json \
  --group cinema-native --test-name CinemaSuite --form all \
  --boundary value-constructor --group-seconds 300
```

Preparation explicitly checks/publishes the specification and derives the one
mandatory inventory. It does not execute tests or finish tasks. The group budget
does not change seeds, generation/interpreter budgets or the old adapter timeout.
Go uses `--boundary json-decoder` and its authored Go root test identity.
Existing schema-1 configuration keeps its meaning; no automatic rewrite occurs.

For a focused diagnosis of the adapter group in an existing mixed plan:

```sh
dctx0 verify application-tests adapter-group SPEC --project APP \
  --plan .dctx0/application-tests/plan.json --group legacy --json
```

This executes only that group's centrally assigned cases, retaining their pinned
inputs and budgets. It never adopts tasks or certifies the complete cohort; the
report retains `HYBRID_COHORT_REQUIRED`. The complete mixed execution still needs
all assigned groups. Cancellation retains managed process cleanup.

## Deliver eligible native tasks

```sh
dctx0 verify application-tests run examples/cinema/dctx0 \
  --project examples/cinema --plan .dctx0/application-tests/plan.json \
  --group cinema-native --recipe .dctx0/application-tests/native-run.json \
  --report .dctx0ctl/application-tests/native.json
```

`run` stores a native run and attempts guarded task delivery by default. To
collect diagnostic evidence without task delivery, use the same command with
`--no-adopt`. Ordinary delivery requires a current configured task plan and
selected profiles. A native PASS is necessary evidence, not a promise that every
task is eligible.

Task requirements and profile bundles are pinned before execution. The store
checks required cases, source/build/evidence binding and current task, claim,
reservation, profile, review and event versions when publishing deliveries.
Running, claimed, reserved, retired/review-only or stale work is protected by the
same guards as the established native path. Historical PASS for older
requirements cannot satisfy new obligations.

Inspect the application report's fields separately:

| Field | Meaning |
| --- | --- |
| `evidence` | Stored native evidence when publication succeeded. |
| `adopted_tasks` | Tasks actually delivered by guarded native publication. |
| `adoption_issues` | Reasons publication was refused or incomplete. |
| `adoption_proposal` | Candidate task coverage and eligibility; not a delivery itself. |
| `adoption_authority` | True only when this execution actually delivered a nonempty task set. |

## Complete native and authored obligations

```sh
dctx0 verify application-tests complete examples/cinema/dctx0 \
  --project examples/cinema --plan .dctx0/application-tests/plan.json \
  --group cinema-native --recipe .dctx0/application-tests/native-run.json \
  --report .dctx0ctl/application-tests/complete-native.json
```

`complete` uses the already prepared check/snapshot, creates missing target
profiles and plans tasks without another implicit check occurrence. It validates
existing authored inspection notes, executes the fresh managed native run and
uses the same complete-assessment/task-publication implementation as the old
`verify complete` path. Notes default to
`.dctx0/testing/TARGET/inspection-notes.json`; `--notes PATH` selects an explicit
file. Missing/rejected or stale reviews remain blockers, not generated approvals.

Cinema's fresh authored-complete target is **105 Done/PASS tasks**, including
four source-bound review tasks reported separately in `reviewed_tasks`. Actual
Go and Bun `complete` runs have each confirmed all 5,766 cases and this 105-task state;
full workspace validation now passes (99 steps, 97.03% merged Rust source lines and 97.04% LLVM functions). This path publishes explicit inspection attestations backed by the native run; it
does not derive testing PASS from review approval alone. Ordinary approval or
manual completion still leaves testing Unverified. Previously completed
Done/Unverified reviews remain preserved; the existing complete workflow may
accept their current source-bound review evidence without upgrading old states.
The complete report checks that no task remains unfinished or lacks the
appropriate native/review evidence.
Native and effective inspected results remain distinct; inspection cannot erase
an actual native FAIL or supply missing execution.

The workflow retains the existing phase/transaction boundaries. Native evidence,
valid deliveries and inspection history may already be retained if a later
complete gate fails. This does not turn the failed overall completion into PASS.
Repository coverage, release performance and platform acceptance are additional
release gates.

## Keep diagnostic evidence separate

| Operation | Task-delivery authority |
| --- | --- |
| Library publication, `prepare`, `serve` | None. |
| `run --no-adopt` | Diagnostic; no task delivery. |
| Fresh managed `run` | Only actual eligible guarded deliveries. |
| `complete` | Guarded native delivery plus source-bound authored completion checks. |
| Replay, imported or saved diagnostic reports | No independent authority. |

Replay runs concrete saved inputs again in their owning native tests. Its PASS
is a new diagnostic result, not reused historical PASS. Repair validation should
also rerun the current full scope before guarded delivery. A manually edited
report, matching case counts or a green native parent alone cannot authorize
Done/PASS.

Bun currently collects the supported top-level root registrations through a
mocked `bun:test` registration API, then validates independent JUnit and exit
status. This is not an official Bun test-list command or individual native case
enumeration; the central journal checks every mandatory case and finalization.
Unsupported registration forms fail collection. Go independently checks its
parent/subcases through native listing and test2json. The new path's current
native acceptance is Linux; adapter platform evidence remains separate.


## Accepted Cinema validation, 2026-11-09

The source-stable full runner passes all 99 steps: 773 all-feature Rust tests,
47 Python tests, the native Go/Bun matrices, all seven Go and eighteen Bun
full-suite mutations, and legacy Go/Rust compatibility. Each new native suite
executes all 5,766 mandatory finite cases. Each fresh managed completion leaves
105 tasks Done/PASS, including four explicit source-bound review attestations.
Assertion replay detects a production mutation and passes after repair; lifecycle
replay repeats both connected cases, and identity replay reexecutes 239 cases
including its actual source observations. Replay itself has no delivery authority.

Rust coverage is 47,437/48,889 merged source lines (97.0300%) and 3,798/3,914 LLVM
functions (97.0363%). These figures do not establish complete branch coverage.
Go library helper statement coverage is separately 217/279 (77.78%). Bun's
reported file means are 98.80% lines and 98.64% functions; its child-process
catalog is outside those metrics and has separate functional probes. Native
helper metrics are not part of Rust coverage and do not establish full helper
branch coverage.

All 126 release speed/RAM workloads completed. The managed full-run samples
are Go 33,125.35 ms / 1,041,174,528 bytes and Bun 25,703.94 ms / 718,131,200 bytes
(sampled process-tree RSS, shared pages counted per process). One sample and
one iteration remain unassessed against the accepted benchmark baseline.
The retained before/after comparison marks incompatible workloads explicitly;
it is not a statistical no-regression claim. The accepted baseline is unchanged.

The Go and Bun diagnostic acceptance runners execute the current finite plan
even when it differs from the frozen before reference. They report historical
comparison as incompatible and unassessed in that case, while still requiring
the current group assignment, native collection, central finalization, and each
current case to pass. Use `--require-reference-match` on either runner to require
the frozen plan for a strict migration comparison. Their `effective_plan_hash`
binds the current plan, manifest, case assignments, configuration, boundary and
form independently of snapshot and check occurrence; benchmark runs with
different hashes are not comparable.

Evidence: `benchmarks/application-tests/after-cinema-native-20261109T060938.334343Z/`;
actual project-store completion and exported runs are retained separately under
`benchmarks/application-tests/root-completion-20261109T010744Z/`.
This acceptance covers Cinema on Linux with Go and Bun. Cross-backend cohort
adoption, a Rust application-test library and the Runner package migration
remain separate later increments.


## Mixed backends

The development CLI supports one adapter group and one application-test group
of the same target. The assignment retains the complete central manifest:

```sh
dctx0 verify application-tests hybrid SPEC --project APP \
  --plan .dctx0/application-tests/hybrid.json --group native \
  --recipe .dctx0/application-tests/native-run.json \
  --report .dctx0ctl/application-tests/fresh-UNIQUE.json --no-adopt
```

Both groups execute freshly. Registration, actual native outcomes, receipts,
source/toolchain pins and complete case coverage must agree. The Store checks
the combined proof independently before guarded delivery. A partial adapter
run remains diagnostic and cannot supply project PASS. Existing report files
are never overwritten by this command.

Use `hybrid-complete` with the same plan/group/recipe/report arguments and
`--notes PATH` for explicit complete acceptance. Its authored notes must match
current sources and specifications; reviews and claims retain their guards.
Done/Unverified Contract and Operation tasks may receive automatic native
checks only when the actual cases cover their complete pinned requirements and
the guarded publication succeeds. Facts without a native case mapping do not
become automatically proved. Alternatively, an explicit source-bound
`done_inspections` entry can request `INSPECTED + actual native` evidence for
Contract, Operation and Adapter tasks. That path requires the same fresh complete
native cohort plus task-specific findings, complete requirements and profile
bindings, reviewed sources and explicit shared fact findings. An authored review
or imported PASS alone cannot authorize it. State versions captured before both
groups start, task claims and reservations, current requirements, profile and
event pins, native proof and inspection bindings are checked transactionally.
Historical results, reviews and task definitions remain intact. See
[inspected historical tasks](../reference/inspection-inputs.md#inspected-historical-done-tasks-in-a-complete-hybrid-run).

Runner's reviewed assignment contains 101 native `PreparedPlans.save` cases,
100 native `AttemptWorker.Execution.publish` cases and 35,576 existing adapter
cases, for the unchanged 35,777-case manifest. A 201-case native diagnostic
passed in an isolated project; it does not certify the remaining adapter cases
or final Runner acceptance. Identity/Access stays on its current path; the
PluginHost path is retained. The Runner wrapper selects hybrid complete and
retains its PostgreSQL, filesystem and Git-LFS fixtures. `--legacy` selects its
previous workflow. Full Runner execution, current authored reviews and release
coverage/performance acceptance remain separate pending gates.

The installed Runner CLI may be older than the development CLI: check the actual
binary's `verify application-tests --help`, including `hybrid` and
`hybrid-complete`, and its content-addressed pin. Changing the Runner toolchain
pin or captured Host inputs requires preserving and regenerating the Host bundle,
reviewing any changed source-bound findings and running a fresh complete cohort
on the final sources. Earlier execution remains historical evidence. Installing
or repinning a binary does not create task PASS. For diagnostics, `hybrid` accepts
`--no-adopt`; `hybrid-complete` requires `--notes` and has no `--no-adopt` flag.
