# Step 8: Collaborator mocks and exact call counts

This directory retains the complete specification and implementation from [step 7](../tutorial-step-7-boundaries/README.md), then adds the features below.

## What this step adds

This step keeps the Catalog consumer real while preparing two collaborator scenarios: the reader returns a summary or returns None. It also upgrades the reader boundary to support asynchronous answers and requires exactly one lookup with the supplied identity.

## Specification extension

The following excerpts are copied from this step’s actual spec. Surrounding declarations are retained in the complete files: [workspace.dctx0](dctx0/workspace.dctx0).

```dctx0
example "reader returns a summary" {
  given TaskReader.lookup(foundId) returns Some(TaskSummary { id = foundId title = "Read" })
  when result = Public.find(foundId)
  expect result == Some(TaskSummary { id = foundId title = "Read" })
}
```

```dctx0
example "reader returns no task" {
  given TaskReader.lookup(missingId) returns None
  when result = Public.find(missingId)
  expect result == None
}
```

```dctx0
operation find(id: TaskId) -> Option<TaskSummary> {
  effects {
    call TaskReader.lookup
  }
  state observes Task by id
  ensures result == first(map(TaskSummary { id = task.id title = task.title } for task in filter(task in before.Task where task.id == id)))
  ensures calls TaskReader.lookup(id) exactly 1
}
```

The new postcondition on `Catalog.Public.find` is `ensures calls TaskReader.lookup(id) exactly 1`. `given` prepares collaborator behavior; `when` invokes the actual consumer; `expect` describes its result. Symbolic foundId/missingId inputs are supplied explicitly in the testing examples configuration. A mock proves the consumer interaction, while separate real-port cases continue to test BoardReader.

## TypeScript implementation

[src/catalog.ts](src/catalog.ts) awaits its reader. The [testing adapter](.dctx0/testing/typescript/adapter.ts) injects a reader only for configured mock scenarios, forwards requests to the Rust mock plan and records actual calls. It does not manufacture business results.

## What is tested

[src/catalog.test.ts](src/catalog.test.ts) checks actual composition, injected readers, asynchronous results and propagation of the same rejected promise error. Native scenarios verify the prepared response plus actual call arguments/count. A consumer mutation that skips its collaborator must fail even if a canned return value looks plausible.

## Run this step

From this step directory, after building the Rust CLI as described in the [series guide](../tutorial/README.md):

```sh
bun install --frozen-lockfile
bun run typecheck
bun run test --coverage
../../target/release/dctx0 verify complete dctx0 --project .
```

The checks include inherited tests and contracts. `verify complete` runs the configured finite native plan and binds valid source-bound inspections where needed. It does not automatically approve free-form architecture text. Measured line/function coverage and a passing finite plan do not prove every possible input or branch.

## Reimplement this step with an agent

From the repository root:

```sh
./examples/tutorial/agent-step.sh 8
# Optionally execute an agent in that new exercise directory:
./examples/tutorial/agent-step.sh 8 -- codex exec --full-auto
```

The exercise supplies this step’s spec, testing adapter and acceptance tests, but starts production code at step 7. The full reference in Git is preserved. Previous SQLite history and inspection approvals are omitted; manual requirements need a fresh substantive review. See the [series guide](../tutorial/README.md) for full-series, mutation and replay commands.
