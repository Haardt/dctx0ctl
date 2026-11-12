# Large source-bound review inputs

`verify inspect`, `verify attest` and `verify complete` read JSON incrementally with
one shared engine-owned reader. Their default wire limit is **64 MiB**. Set
`--max-input-bytes N` to change it; positive byte counts below `u64::MAX` are
accepted. The limit includes whitespace and trailing bytes. Oversized or malformed
JSON is rejected before publication. Regular files are checked before parsing;
the reader also enforces the bound if input grows during reading.

```sh
dctx0 verify inspect spec NATIVE_RUN --project . --file reviews.json
dctx0 verify inspect spec NATIVE_RUN --project . --file reviews.json --max-input-bytes 134217728
dctx0 verify attest spec NATIVE_RUN --project . --file attestations.json --max-input-bytes 134217728
dctx0 verify complete spec --project . --inspections authored-notes.json --max-input-bytes 134217728
```

Inspection groups remain JSON arrays of complete `ProgramInspection` records.
There is no implicit accumulation: submit the complete intended group against one
retained native basis. An imported effective report cannot become a new native
basis. Unknown obligations, duplicate subjects and mismatched snapshot, plan,
requirements or implementation vectors are rejected atomically. Native failures,
missing/duplicate cases and incomplete provenance cannot be promoted to PASS.
Unreviewed manual obligations remain UNKNOWN. Source judgements remain separate
from automatic observations and do not automatically adopt tasks.

Rust callers use `read_review_input`, `DEFAULT_REVIEW_INPUT_BYTES` and, when a
custom notes bound is needed, `Project::complete_verification_with_input_limit`.
The existing `complete_verification` method uses the same default. These resource
limits do not truncate mandatory contracts or native evidence.

## Reproduce the acceptance and measurements

```sh
python3 scripts/inspection_acceptance.py
```

The runner builds an uninstrumented release CLI and runs actual Bun native
verification against 543 production modules. Groups of 19 and 47 bound reviews
exceed the former 1 MiB cap. It checks every submitted effective outcome, retains
an unreviewed UNKNOWN, and rejects invalid bindings, conflicting duplicates,
malformed input, resource overflow and invalid native evidence. Separate complete
CLI fixtures also use authored notes larger than 1 MiB.

`benchmarks/history/inspection/20261017T095416.009809Z-20553.json` completes both
command stages against unchanged inputs. The 19-review group is 1,497,179 bytes:
93.13 ms and 21,987,328 peak RSS bytes. The 47-review group is 3,703,635 bytes:
122.58 ms and 30,572,544 peak RSS bytes. These expected UNKNOWN CLI results reflect
an intentionally unreviewed obligation, not failed input processing. Every
submitted review is separately checked as effective PASS over its fixture.

Memory is Linux `wait4` process peak RSS for each actual release CLI invocation;
inspection invokes no compiler or native worker. Compiler/test orchestration and
Bun fixture costs are recorded separately. These are single-sample development
measurements with warm toolchain caches, not an accepted regression baseline.
Latest 20 reports are retained transactionally under `benchmarks/history/inspection/`.
The groups are acceptance fixtures, not professional reviews of Company Manager
or claims that its remaining implementation gaps are resolved.

The first corrected-candidate CI completed all individual workspace checks and
reported 97.56% merged source-line and 97.09% LLVM function coverage. Its aggregate
result correctly failed the source-change guard: recursive harness JSON discovery
had included Cargo `target` fingerprints created by the native tests. The input
inventory now excludes `target` and `node_modules` cache directories while keeping
authored harness JSON and embedded UI assets. A regression test checks both cache
creation stability and detection of an actual configuration change. Fresh complete
CI remains required before publication; no coverage or performance gate is waived.

The follow-up full workspace run passed with unchanged source provenance and the
same 97.56%/97.09% coverage. Its subsequent Gin/Axum stage exposed a separate
runner defect: `cargo fmt` on the reference adapter manifest required `worker.rs`,
which exists only inside captured native builds. The format stage now checks both
the authored adapter and the shipped worker template directly with Rustfmt.
Framework acceptance and complete CI must still pass before release.

## Inspected historical Done tasks in a complete hybrid run

An optional `done_inspections` object in the authored `verify complete` notes
can request new evidence for historical Done/Unverified Contract, Operation and
Adapter tasks. It has schema version 1, the same `actor` as the original notes,
`snapshot`, `check_occurrence`, the full `task_pin` (including plan/event,
requirements and profile bundles), a `sources` map, a shared `facts` map and a
`tasks` map keyed by task ID. Obtain the pin from the read-only verification
preflight and the task state version and requirements from task inspection; a
stale pin or version rejects the transaction. Each source path/hash must be in
the original author's `reviewed_hashes` as well as the current hybrid capture.
The engine does not turn inventory discovery into an authored review.

Each task entry explicitly includes its `state_version`, `requirements_hash`,
`profile_bundle`, exact `subjects`, `types` and `facts` sets, `fact_reviews`
(shared-fact keys), task-specific `implementation_finding`, `profile_finding`,
source paths and limitations. Adapter tasks also require an
`adapter_binding_finding`. Each shared fact entry names the fact, the exact task
IDs that reference it, a finding, reviewed sources and limitations. The current
format requires a reviewed finding for every task fact; one explicit shared
finding can cover multiple tasks. A finding is an author's judgement, not an
inference or a manufactured approval. When a task subject has adapter-assigned
cases, its reviewed sources must include the source artifact in the actual
adapter description.

Only a fresh, complete managed hybrid run with all mandatory cases PASS can
publish these checks. The store checks the saved hybrid proof, captured source
inventory, task state/version, latest check, task events and selected profiles
again in one transaction with any other delivered work. The original Done
result and reviews remain intact. Appended evidence is labeled `INSPECTED +
actual native`; its report links the actual run and inspection artifact and
separately lists native cases mapped to facts and manual fact reviews. Neither
basis expands the finite coverage into a general proof.

`status` counts this evidence in the regular current/stale totals and also in
`inspected_current`/`inspected_stale`. These freshness counts compare current
source, specification and pinned requirements, not a newly executed runtime
identity check. Native status additionally requires a guarded publication's
run/task/requirements link. Inspected status requires that link and the exact
store-created check event and report, so a freely recorded or imported check is
not treated as native authority. The linked inspected run and artifact are held
by verification retention; no source, historical result, review or prior
verification is rewritten. On schema 6, the receipt index needed to establish
inspected-current status is absent: the lookup returns unknown, and retention
preserves linked inspected artifacts without implicitly migrating the database.
Schema 7 can verify the private check event and report binding. A source or spec
change makes the earlier binding stale; a Runner toolchain pin change also changes
the captured Host bundle inputs. Preserve and regenerate the bundle, review the
changed source pins and obtain a fresh complete hybrid cohort for the new inputs.

Compatibility correction: a Review task in complete verification now needs an
explicit structured `review` in the authored note for that review task's own
subject, even when it has no listed review obligations. An unrelated or general
note never supplies approval. If multiple task subjects have structured reviews,
all must approve without unresolved high findings, and all findings are retained.
Rejected reviews and reviews with unresolved high findings cannot be delivered as
approved; supply an actual reviewed decision instead of relying on the former
implicit approval fallback.

Newly merged authored notes retain the original actor and scope in each imported
subject report. When they are combined with stored source-bound reviews, the
report identifies the acting account as an aggregator and names the authored
source; when no stored subjects are present, the imported actor and scope remain
the acting identity. Previously recorded reports are left unchanged.
