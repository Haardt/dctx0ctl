# Agent evidence and reviews

Agents can evaluate requirements that do not have a complete automatic checker. DCTX0 records their judgment as separate, source-bound evidence. It does not turn a review into an automatic proof.

## Assigned conformance obligations

Planner version 2 assigns manual constraints/decisions, explicit reviews, non-execution suites, and policies to Review tasks. Existing scoped reviews are reused; uncovered scopes receive a review owner. Executable examples and properties remain native obligations.

A fresh Cinema plan has four review owners for 25 obligations and 105 tasks overall. Historical task definitions/results remain immutable.

## Review the actual requirements

Check the specification and publish the current plan. Select a ready review and start it under ordinary actor/claim/version guards:

```sh
dctx0 tasks list <spec-root> --kind review
dctx0 tasks show <spec-root> T42
dctx0 tasks start <spec-root> T42 --project <app-root> --actor agent-1
dctx0 verify review-template <spec-root> --project <app-root> --task T42 > /tmp/review-T42.json
```

Use a real assigned ID. The template is read-only and deliberately incomplete: an empty reviewer/summary and per-subject reports, initially requesting changes. It is not approval.

Inspect actual sources, contracts, and relevant test results. Report concrete files/symbols, evidence, limitations, verdicts, and actionable findings. A task-level summary does not replace reports for all assigned subjects. Store report files outside the captured application inventory so writing a report does not change its own binding.

```sh
dctx0 tasks record-review <spec-root> T42 --project <app-root> \
  --actor agent-1 --claim CLAIM --version VERSION --file /tmp/review-T42.json
dctx0 tasks complete <spec-root> T42 --project <app-root> \
  --actor agent-1 --claim CLAIM --version NEW_VERSION --summary "Reviewed assigned obligations."
```

Replace guard placeholders with returned values. Empty, partial, misbound, or stale reports are rejected. Completing a request-changes review finishes the activity, but does not satisfy the requirement. An approved bound Review task can remain Done/Unverified.

## Changed implementation

```sh
dctx0 tasks renew-review <spec-root> T42 --project <app-root> \
  --reason "Reinspect the changed implementation"
```

Renewal creates a new Open/Unverified successor and preserves history. Repeating the same renewal for the same binding/reason is idempotent. Current review binding is conservative across the captured application inventory; this is not selective per-symbol invalidation.

`verify complete` consumes current reports and revalidates their sources and requirements. Imported inspection notes cannot mask current assigned reports or supply a missing current review.

## Explicit inspection publication

```sh
dctx0 verify inspect <spec-root> <run-id> --project <app-root> --file <program-reports.json>
dctx0 verify attest <spec-root> <run-id> --project <app-root> --file <task-reports.json>
```

`inspect` stores effective suite/review/policy assessments without executing or adopting tasks. `attest` delivers eligible task reports through guarded transitions. Both require exact source and requirement bindings. Native failures and omitted mandatory execution remain authoritative.

Authored inspection notes can also be supplied explicitly. Hashes establish which inputs were reviewed, not whether the review was substantive. Meaningful tests and genuine inspection remain necessary.

See the [complete conformance-review contract](https://github.com/Haardt/dctx0ctl/blob/main/docs/conformance-reviews.md).
