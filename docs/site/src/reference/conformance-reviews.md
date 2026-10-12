# Agent conformance reviews

Manual requirements are actionable task obligations, not permanently open tests.
The planner assigns each constraint, decision, explicit review, non-execution
conformance suite and policy to one Review task. Existing scoped reviews are reused;
uncovered scopes receive a conformance review task. Cinema has 25 assigned
obligations in four review tasks and 105 tasks in a fresh plan. Execution suites
such as Examples and Properties remain native obligations; an approval cannot
replace their execution.

Planner version 2 adds optional `review_obligations` to task candidates. Task
inspection exposes the assigned IDs, original source excerpts and acceptance
instructions. Existing definitions and Done results are immutable. Replanning
creates successors for changed review requirements, preserves historical tasks,
and adds the missing conformance task. Compiler IR and snapshot goldens do not
change. SQLite needs no schema migration: reviews use existing append-only events.

## Agent workflow

Use the ordinary task workflow. First check and publish the current plan, then
select an available review with delivered prerequisites:

```sh
dctx0 check examples/cinema/dctx0
dctx0 tasks plan examples/cinema/dctx0
dctx0 tasks list examples/cinema/dctx0 --kind review
dctx0 tasks show examples/cinema/dctx0 T123
dctx0 tasks start examples/cinema/dctx0 T123 --project examples/cinema --actor agent-1

dctx0 verify review-template examples/cinema/dctx0 \
  --project examples/cinema --task T123 > /tmp/review-T123.json
```

The template is read-only and deliberately incomplete: empty reviewer, summary
and per-subject reports, with `request-changes` as its initial judgement. It is
not an approval. The agent inspects the actual source and spec, runs suitable
checks, fixes deviations or reports findings, and fills every assigned subject's
`report` and `limitations`. Reports should name concrete files/symbols, relevant
checks and their observed results. A task-level summary alone is insufficient.
Use `approved` only when the evidence supports the requirements. Otherwise use
`request-changes` and structured actionable findings. Unknown or unobservable
requirements must remain unresolved.

Keep report files outside the application inventory, such as `/tmp`: writing a
new report into the application after creating its template changes the binding.
Claims and versions below are the guards returned by the preceding commands:

```sh
dctx0 tasks record-review examples/cinema/dctx0 T123 \
  --project examples/cinema --actor agent-1 --claim CLAIM --version VERSION \
  --file /tmp/review-T123.json

dctx0 tasks complete examples/cinema/dctx0 T123 \
  --project examples/cinema --actor agent-1 --claim CLAIM --version NEW_VERSION \
  --summary 'Reviewed all assigned requirements with concrete source and check evidence.'

dctx0 verify complete examples/cinema/dctx0 --project examples/cinema
```

Recording validates task requirements, the current spec snapshot/source hash,
complete application inventory and exact assigned subjects. Empty, partial,
misbound or stale reports are rejected. Completion remains separate from approval:
a request-changes report can finish the review activity, but cannot satisfy the
conformance obligation. A source-bound review can be Done/Unverified. It never
becomes testing PASS merely because an agent approved it.

`verify complete` uses the latest persisted report of each active review task.
It revalidates its sources, requirements and scope against the actual execution.
Missing/rejected/stale reports and actionable unresolved findings prevent full
acceptance. Earlier convenient approvals are not substituted for a later rejection.
Native observations remain immutable; native FAIL and missing execution cannot
be overridden. The complete report includes `reviewed_tasks` separately and
`non_pass_tasks` still counts tasks without testing PASS, including review tasks.
A successful report can therefore have four Done/Unverified Cinema review tasks.
Executable Release gates are always evaluated by Rust against actual outcomes;
a policy review cannot overrule a failed gate.

## Changed implementation and renewed reviews

A changed application inventory makes an old confirmation insufficient for current
acceptance, without rewriting the historical review. Explicitly create a successor:

```sh
dctx0 tasks renew-review examples/cinema/dctx0 T123 \
  --project examples/cinema --reason 'Reinspect the changed implementation'
```

The returned plan contains a new Open/Unverified review task with a historical
link. Dependent integration/review revisions follow existing follow-up rules.
Only delivered, unclaimed reviews can be renewed. Repeating the same renewal for
the same source binding and reason is idempotent. Use the new task ID for start,
review-template, record-review and complete. Replanning retains the renewal.
The initial inventory binding is deliberately conservative: any captured source,
configuration or dependency change can require renewed review. This does not
claim selective per-symbol review invalidation.

## Compatibility and evidence limits

Legacy `record-review` JSON remains accepted without `conformance`, but unbound
legacy approvals cannot discharge current program obligations. Existing
`inspection-notes.json` remains an import path. Stored source-bound task reports
are authoritative for their assigned subjects, including rejections and stale
bindings; imported notes cannot mask those reports. Old testing/check history is
preserved. No production architecture, registration or dctx0 dependency is added.

Agent reports are authored judgements, not independently proven facts. Source
hashes establish which code was reviewed; they cannot establish that an agent
actually performed a good review. Ordinary tests, mutation checks and substantive
review remain necessary evidence.

Contributor acceptance:

```sh
python3 scripts/validate.py --focus conformance-reviews --example cinema \
  --samples 1 --iterations 1 --coverage
```

This focus runs task/store/engine/CLI checks and the source-bound agent workflow
against TypeScript, Go and Rust Cinema. The Cinema workflow fixture reuses existing,
hash-validated authored inspection notes as test data; it does not generate new
architectural approvals. Coverage still executes the complete instrumented Rust
workspace. Release speed/RSS measurements run at the end; reduced samples remain
unassessed against the accepted baseline. Python is contributor tooling only;
the product workflow above uses the Rust executable directly.

## Acceptance (2026-10-12)

TypeScript/Bun, Go and Rust Cinema each pass 5,674 mandatory native cases and the
persisted agent workflow: 25 obligations, four Done/Unverified review tasks,
105 total tasks, no open tasks and an effective PASS without an inspection-notes
file. The ordinary CLI integration also covers later bound and legacy rejections,
source drift, real production mutation, explicit renewal, idempotence, replanning
and preserved historical reviews. Source-binding unit tests cover stale input
rejection, invalid review scope, renewal guards and retained finding severities.

The complete instrumented Rust workspace passes. Merged source lines are
30,350/31,070 (97.68%); raw LLVM functions are 2,454/2,529 (97.03%). No coverage
threshold or ordinary error-path exclusion was introduced. The first 96.95%
function checkpoint remains recorded as failed rather than relabelled.

Current stable task/store/engine/CLI checks and the release Cinema workflow pass.
The final 71 release speed/RSS workloads ran afterward and remain unassessed
against the accepted baseline, which was not changed. Concurrently edited
`scripts/install_mdbook.py` and `scripts/docs_site.py` are outside this feature's
verification and measurement source scope; neither is executed or embedded here.
All Rust and embedded-helper coverage inputs remain fully included and match the
current tree. See retained evidence (historical evidence removed during repository cleanup) and
`benchmarks/history/20261012T112024.971179Z-1115890.json`. The aggregate records
that encountered concurrent source changes retain their failed validation stamp;
independently valid coverage, stable checks and final measurements are linked
explicitly in the accepted scoped record.
