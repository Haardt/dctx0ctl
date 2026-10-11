# Step 6: Entities, keys and public messages

This directory retains the complete specification and implementation from [step 5](../tutorial-step-5-collections/README.md), then adds the features below.

## What this step adds

The counter example becomes a stored task board. Task has a single identity key; Note has a composite key. TaskSummary is a public message that deliberately omits internal task status. Read operations now relate their results to actual application state.

## Specification extension

The following excerpts are copied from this step’s actual spec. Surrounding declarations are retained in the complete files: [workspace.dctx0](dctx0/workspace.dctx0).

```dctx0
entity Task {
  id: TaskId
  title: Title
  status: Status
  key id
}
```

```dctx0
entity Note {
  taskId: TaskId
  sequence: Count
  text: Title
  key taskId, sequence
}
```

```dctx0
message TaskSummary {
  id: TaskId
  title: Title
}
```

```dctx0
operation list() -> List<TaskSummary> {
  state lists Task
  ensures result == map(TaskSummary { id = task.id title = task.title } for task in before.Task)
}
```

```dctx0
operation find(id: TaskId) -> Option<TaskSummary> {
  state observes Task by id
  ensures result == first(map(TaskSummary { id = task.id title = task.title } for task in filter(task in before.Task where task.id == id)))
}
```

```dctx0
operation notes(id: TaskId) -> List<Note> {
  state lists Note
  ensures result == filter(note in before.Note where note.taskId == id)
}
```

`key id` identifies a Task; `key taskId, sequence` identifies a Note. `state lists` selects a complete collection, while `state observes ... by` selects a keyed observation. `before.Task` and `before.Note` refer to captured application state before invocation. The list operation maps entities to messages; find returns an option; notes filters by task identity.

## TypeScript implementation

[src/entities.ts](src/entities.ts) implements entity/message factories, storage and snapshot access. The testing adapter supplies real session snapshots, rather than constructing expected state from the spec.

## What is tested

[src/entities.test.ts](src/entities.test.ts) checks message boundaries, present/missing reads, snapshot isolation and uniqueness of simple/composite keys. Native verification compares returned collections/options with actual before-state. Explicit existing-key inputs complement generated identifiers. A mutation that returns an empty list must fail.

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
./examples/tutorial/agent-step.sh 6
# Optionally execute an agent in that new exercise directory:
./examples/tutorial/agent-step.sh 6 -- codex exec --full-auto
```

The exercise supplies this step’s spec, testing adapter and acceptance tests, but starts production code at step 5. The full reference in Git is preserved. Previous SQLite history and inspection approvals are omitted; manual requirements need a fresh substantive review. See the [series guide](../tutorial/README.md) for full-series, mutation and replay commands.
