# Step 7: Modules, ownership, ports and decisions

This directory retains the complete specification and implementation from [step 6](../tutorial-step-6-entities/README.md), then adds the features below.

## What this step adds

This step introduces a Catalog module with explicit ownership and a required reader port. Its Public surface exposes summaries through a replaceable collaborator. A decision documents the public message boundary, and a constraint documents the dependency direction.

## Specification extension

The following excerpts are copied from this step’s actual spec. Surrounding declarations are retained in the complete files: [workspace.dctx0](dctx0/workspace.dctx0).

```dctx0
decision PublicMessages {
  choose """Return TaskSummary at the catalog boundary."""
  because """Internal status belongs to the task entity."""
  avoid """Do not expose stored entities as public messages."""
}
```

```dctx0
port TaskReader {
  operation lookup(id: TaskId) -> Option<TaskSummary> {
    state observes Task by id
    ensures result == first(map(TaskSummary { id = task.id title = task.title } for task in filter(task in before.Task where task.id == id)))
  }
}
```

```dctx0
surface Public exposes requestReply {
  operation find(id: TaskId) -> Option<TaskSummary> {
    effects {
      call TaskReader.lookup
    }
    state observes Task by id
    ensures result == first(map(TaskSummary { id = task.id title = task.title } for task in filter(task in before.Task where task.id == id)))
  }
}
```

The containing `module Catalog` declares `owns { entity Task message TaskSummary }` and `requires { TaskReader }`. The port describes the reader contract independently of its implementation. `effects { call TaskReader.lookup }` declares the collaborator boundary; it does not prove a runtime call count. `choose`, `because`, `avoid` and free-text constraints require substantive inspection rather than invented behavioral assertions.

## TypeScript implementation

[src/catalog.ts](src/catalog.ts) contains a normal TaskReader interface, a BoardReader backed by real storage and a Catalog consumer. Constructor injection is ordinary application code; no DI framework or generated production types are required.

## What is tested

[src/catalog.test.ts](src/catalog.test.ts) exercises real BoardReader/Catalog composition and an alternative injected reader. Native verification checks port and surface results against actual stored entities. Separate source-bound notes inspect the constraint and decision. A mutation that ignores the real reader must fail.

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
./examples/tutorial/agent-step.sh 7
# Optionally execute an agent in that new exercise directory:
./examples/tutorial/agent-step.sh 7 -- codex exec --full-auto
```

The exercise supplies this step’s spec, testing adapter and acceptance tests, but starts production code at step 6. The full reference in Git is preserved. Previous SQLite history and inspection approvals are omitted; manual requirements need a fresh substantive review. See the [series guide](../tutorial/README.md) for full-series, mutation and replay commands.
