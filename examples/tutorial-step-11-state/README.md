# Step 11: Writes and before/after state contracts

This directory retains the complete specification and implementation from [step 10](../tutorial-step-10-effects/README.md), then adds the features below.

## What this step adds

This step introduces a TaskWriter port with distinct insert, replace, upsert and remove semantics. It also writes composite-keyed notes. Successful returns are insufficient: verification must observe the correct stored changes and preservation on failure.

## Specification extension

The following excerpts are copied from this step’s actual spec. Surrounding declarations are retained in the complete files: [workspace.dctx0](dctx0/workspace.dctx0).

```dctx0
operation insert(request: Task) -> Task ! TaskConflict {
  effects {
    read Task
    write Task
  }
  state inserts Task from request
  ensures success implies creates Task(request.id)
  ensures success implies exists Task(request.id)
  ensures success implies after.Task(request.id).status == request.status
  ensures failure is TaskConflict implies preserves Task(request.id)
  ensures success implies result == request
  ensures failure is TaskConflict implies after.Task == before.Task
}
```

```dctx0
operation replace(request: Task) -> Task ! MissingTask {
  effects {
    read Task
    write Task
  }
  state replaces Task by request.id from request
  ensures success implies (updates Task(request.id) or preserves Task(request.id))
  ensures success implies after.Task(request.id).title == request.title
  ensures success implies result == request
  ensures failure is MissingTask implies after.Task == before.Task
}
```

```dctx0
operation save(request: Task) -> Task {
  effects {
    read Task
    write Task
  }
  state upserts Task from request
  ensures exists Task(request.id)
  ensures creates Task(request.id) or updates Task(request.id) or preserves Task(request.id)
  ensures result == request
}
```

```dctx0
operation remove(id: TaskId) -> Unit {
  effects {
    read Task
    write Task
  }
  state removes Task by id
  ensures removes Task(id) or preserves Task(id)
  ensures after.Task == filter(task in before.Task where task.id != id)
}
```

```dctx0
operation saveNote(request: Note) -> Note {
  effects {
    read Note
    write Note
  }
  state upserts Note from request
  ensures exists Note(request.taskId, request.sequence)
  ensures after.Note(request.taskId, request.sequence).text == request.text
  ensures result == request
}
```

Insert rejects an existing key with TaskConflict; replace rejects a missing key with MissingTask; upsert supports both creation and replacement; remove is idempotent. The executable predicates `exists`, `creates`, `updates`, `preserves` and `removes` compare real snapshots. `after.Task(request.id)` selects a single key; `after.Note(request.taskId, request.sequence)` selects a composite key. Failure clauses require the collection to remain unchanged.

## TypeScript implementation

[src/entities.ts](src/entities.ts) performs real storage updates and returns copies. The adapter observes writes and complete before/after state, including existing-key and new-key scenarios.

## What is tested

[src/state.test.ts](src/state.test.ts) checks duplicate insertion, missing replacement, unchanged state after failure, exactly one changed row, returned-value isolation, repeated removal and composite-key separation. Native verification checks all state modes/predicates and field values. A replacement that returns the requested value without storing it must fail.

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
./examples/tutorial/agent-step.sh 11
# Optionally execute an agent in that new exercise directory:
./examples/tutorial/agent-step.sh 11 -- codex exec --full-auto
```

The exercise supplies this step’s spec, testing adapter and acceptance tests, but starts production code at step 10. The full reference in Git is preserved. Previous SQLite history and inspection approvals are omitted; manual requirements need a fresh substantive review. See the [series guide](../tutorial/README.md) for full-series, mutation and replay commands.
