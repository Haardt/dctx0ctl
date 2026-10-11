# Step 9: Declared failures and emitted events

This directory retains the complete specification and implementation from [step 8](../tutorial-step-8-mocks/README.md), then adds the features below.

## What this step adds

This step adds a MissingTask business failure, a TaskViewed event and Catalog.Public.requireTask. A successful call publishes the event and returns a summary; a missing task throws the declared failure without publishing an event.

## Specification extension

The following excerpts are copied from this step’s actual spec. Surrounding declarations are retained in the complete files: [workspace.dctx0](dctx0/workspace.dctx0).

```dctx0
failure MissingTask { taskId: TaskId }
```

```dctx0
event TaskViewed { taskId: TaskId }
```

```dctx0
operation requireTask(id: TaskId) -> TaskSummary ! MissingTask {
  state observes Task by id
  effects {
    call TaskReader.lookup
    emit TaskViewed(taskId: id)
  }
  ensures success implies result == single(map(TaskSummary { id = task.id title = task.title } for task in filter(task in before.Task where task.id == id)))
  ensures failure is MissingTask implies first(filter(task in before.Task where task.id == id)) == None
  ensures calls TaskReader.lookup(id) exactly 1
  ensures success implies emits TaskViewed(TaskViewed { taskId = id }) exactly 1
  ensures failure is MissingTask implies emits TaskViewed() exactly 0
}
```

```dctx0
example "require an existing task" {
  given TaskReader.lookup(foundId) returns Some(TaskSummary { id = foundId title = "Read" })
  when result = Public.requireTask(foundId)
  expect Success
  expect result == TaskSummary { id = foundId title = "Read" }
  expect emits TaskViewed { taskId = foundId }
}
```

```dctx0
example "require a missing task" {
  given TaskReader.lookup(missingId) returns None
  when result = Public.requireTask(missingId)
  expect result fails MissingTask
}
```

`-> TaskSummary ! MissingTask` declares both outcomes. `success` and `failure is MissingTask` select conditional postconditions. `expect Success`, `expect result fails` and `expect emits` express scenario outcomes. Exact trace clauses require one lookup, one matching event after success and zero events after the declared failure. A failure payload is checked as data independently of the transport outcome envelope.

## TypeScript implementation

[src/events.ts](src/events.ts) defines the real error/event factories; [src/catalog.ts](src/catalog.ts) throws and publishes through an ordinary callback. The adapter translates the caught error and observes the actual callback.

## What is tested

[src/events.test.ts](src/events.test.ts) validates payloads, actual error type, success publication and the absence of publication on failure. Native scenarios compare failure/event payloads and counts. Removing the real publication must fail the executable event requirement.

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
./examples/tutorial/agent-step.sh 9
# Optionally execute an agent in that new exercise directory:
./examples/tutorial/agent-step.sh 9 -- codex exec --full-auto
```

The exercise supplies this step’s spec, testing adapter and acceptance tests, but starts production code at step 8. The full reference in Git is preserved. Previous SQLite history and inspection approvals are omitted; manual requirements need a fresh substantive review. See the [series guide](../tutorial/README.md) for full-series, mutation and replay commands.
