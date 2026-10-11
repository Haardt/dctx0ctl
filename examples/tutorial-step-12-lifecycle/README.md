# Step 12: Connected lifecycle paths

This directory retains the complete specification and implementation from [step 11](../tutorial-step-11-state/README.md), then adds the features below.

## What this step adds

This step extends Status with Archived and models Open → Done → Archived. A second lifecycle models entity presence: Present → Removed. Both use actual writer operations and real keyed state.

## Specification extension

The following excerpts are copied from this step’s actual spec. Surrounding declarations are retained in the complete files: [workspace.dctx0](dctx0/workspace.dctx0).

```dctx0
type Status = Open | Done | Archived
```

```dctx0
lifecycle TaskLifecycle for Task using status {
  state Open initial
  state Done
  state Archived terminal
  transition Catalog.TaskWriter.replace
    from Open
    to Done
    when request.status == Done
  transition Catalog.TaskWriter.replace
    from Done
    to Archived
    when request.status == Archived
}
```

```dctx0
lifecycle TaskPresence for Task using presence {
  state Present initial
  state Removed terminal
  transition Catalog.TaskWriter.remove
    from Present
    to Removed
    when true
}
```

`initial` and `terminal` mark path endpoints. A transition names an operation, source/destination states and an executable `when` guard over its typed request. `using status` observes a field; `using presence` observes whether the keyed entity exists. [lifecycles.json](.dctx0/testing/typescript/lifecycles.json) supplies reviewed setup calls, keys and ordered transition inputs. Transitions within one path share a session; independent paths start fresh.

## TypeScript implementation

The existing Board insert/replace/remove implementation executes setup and transitions. Rust compares actual state at every transition and checks that each after-state connects to the next before-state.

## What is tested

[src/lifecycle.test.ts](src/lifecycle.test.ts) checks connected status changes, removal and session independence. Native verification executes all configured mandatory paths, including the multi-transition path. A mutation that leaves a replacement Open must fail. The series mutation/replay tool reruns the saved lifecycle counterexample and distinguishes repaired code through new provenance.

## Run this step

From this step directory, after building the Rust CLI as described in the [series guide](../tutorial/README.md):

```sh
bun install --frozen-lockfile
bun run typecheck
bun run test --coverage
bun test ./.dctx0/testing/typescript/resource-observer.test.ts
../../target/release/dctx0 verify complete dctx0 --project .
```

The checks include inherited tests and contracts. `verify complete` runs the configured finite native plan and binds valid source-bound inspections where needed. It does not automatically approve free-form architecture text. Measured line/function coverage and a passing finite plan do not prove every possible input or branch.

## Reimplement this step with an agent

From the repository root:

```sh
./examples/tutorial/agent-step.sh 12
# Optionally execute an agent in that new exercise directory:
./examples/tutorial/agent-step.sh 12 -- codex exec --full-auto
```

The exercise supplies this step’s spec, testing adapter and acceptance tests, but starts production code at step 11. The full reference in Git is preserved. Previous SQLite history and inspection approvals are omitted; manual requirements need a fresh substantive review. See the [series guide](../tutorial/README.md) for full-series, mutation and replay commands.
