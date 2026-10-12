# Workflow and CLI

All commands below use the Rust `dctx0` CLI. `<spec-root>` is the reviewed specification directory; `<app-root>` is the independent application directory. Use `--json` for machine-readable output. Task queries default to Markdown.

## 1. Compile and publish requirements

```sh
dctx0 parse <spec-root>
dctx0 check <spec-root>
dctx0 history <spec-root> --json
```

`parse` also accepts a single file. `check` publishes a valid snapshot transactionally. Failed compilation records diagnostics while preserving the previous valid head.

## 2. Plan focused work

```sh
dctx0 profile typescript create --project <app-root> --default --agents
dctx0 tasks preview <spec-root>
dctx0 tasks plan <spec-root>
dctx0 tasks waves <spec-root>
dctx0 tasks next <spec-root>
dctx0 tasks show <spec-root> T42 --target typescript
```

Use actual IDs from your plan. A task normally implements one contract or operation. Context includes required type definitions, constraints, examples, prerequisites, and relevant prior results. Large contexts expose missing required sections and pinned pagination cursors; load them before implementation.

## 3. Claim work and deliver results

```sh
dctx0 tasks claim-next <spec-root> --actor agent-1
dctx0 tasks context <spec-root> T42 --section implementation
```

`next` is read-only; `claim-next` is atomic. Explicit start, release, and complete transitions require actor, claim, and version guards. Use `tasks slice` / `claim-slice` for dependency-closed work bundles. Reserving a later task does not mark its prerequisites delivered.

Work (`Open`, `Progress`, `Done`) and testing (`Unverified`, `PASS`, `FAIL`) are separate. Manual completion and review approval do not manufacture testing PASS. Use `dctx0 tasks --help` and the individual transition commands' help for exact arguments.

## 4. Prepare testing artifacts

```sh
dctx0 verify discover <spec-root> --project <app-root> --target typescript
dctx0 verify init <spec-root> --project <app-root> --target typescript \
  --subject <qualified-name>
dctx0 verify plan <spec-root> --project <app-root>
```

The same commands select `go` or `rust`. A scaffold is explicitly unavailable until implemented. Configure a nonempty subject/task scope, reviewed example inputs, and deterministic budgets. The default generated-case budget is 100 per obligation. Explicit examples remain mandatory.

## 5. Execute, inspect, and replay

```sh
dctx0 verify run <spec-root> --project <app-root> --no-adopt
dctx0 verify show <spec-root> <run-id> --json
dctx0 verify replay <spec-root> <run-id> --case <case-id> --project <app-root>
```

The adapter invokes actual production behavior and supplies observations. The core assesses the selected mandatory finite plan. Replay creates a new comparison and preserves the original report.

Omit `--no-adopt` only when task delivery is intended. Complete task-specific native evidence can deliver eligible open, unclaimed, unreserved tasks. Missing provenance, changed inputs, unmet prerequisites, and protected task kinds block adoption.

## 6. Review non-executable obligations

Constraints, decisions, explicit reviews, non-execution suites, and policies receive assigned Review tasks. Agents inspect actual requirements and implementation; source-bound reports preserve their judgment separately from native results.

```sh
dctx0 tasks list <spec-root> --kind review
dctx0 verify review-template <spec-root> --project <app-root> --task T42
```

Templates are incomplete, not approvals. Use ordinary guarded review recording and completion. Changed implementation may require `tasks renew-review`. See [agent evidence and reviews](agent-evidence.md).

## 7. Require complete configured acceptance

```sh
dctx0 verify complete <spec-root> --project <app-root> --json
```

This explicitly checks the spec, publishes the current task plan, executes the configured native scope, validates authored inspections and current task reports, and requires a nonempty delivered active plan with appropriate evidence.

A delivered, approved, source-bound Review task can remain Done/Unverified. `reviewed_tasks` lists those tasks separately; `non_pass_tasks` can include them even in a successful summary. Executable work still needs PASS. Running/reserved work, incomplete execution, actual failures, and missing/stale reviews prevent completion.

This command evaluates the configured application scope. Repository lint, coverage, and release performance acceptance are [separate contributor checks](contributing.md).
