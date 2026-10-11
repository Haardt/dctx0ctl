# Step 10: Observable resources and effect permissions

This directory retains the complete specification and implementation from [step 9](../tutorial-step-9-failures-events/README.md), then adds the features below.

## What this step adds

This step adds explicit read permissions to existing stored-state operations and an external clock boundary. State snapshots describe values; resource observations additionally record what the application actually accessed during invocation.

## Specification extension

The following excerpts are copied from this step’s actual spec. Surrounding declarations are retained in the complete files: [workspace.dctx0](dctx0/workspace.dctx0).

```dctx0
external Signals {
  intent """Read the local system clock through an explicit boundary."""
  surface Clock {
    operation now() -> Timestamp
  }
}
```

```dctx0
surface Diagnostics exposes requestReply {
  operation timestamp() -> Timestamp {
    effects { observe Signals.Clock.now }
  }
}
```

```dctx0
operation notes(id: TaskId) -> List<Note> {
  effects { read Note }
  state lists Note
  ensures result == filter(note in before.Note where note.taskId == id)
}
```

`effects { read Note }` permits accesses to Note; `effects { observe Signals.Clock.now }` declares the external observation boundary. Read, write and observe clauses are permissions, not minimum call counts. Exact calls and emits use the trace clauses from previous steps. The clock is a local dependency here; this declaration does not imply a remote endpoint.

## TypeScript implementation

[src/diagnostics.ts](src/diagnostics.ts) reads a real Date through an injected clock. The [resource observer](.dctx0/testing/typescript/resource-observer.ts) watches actual storage arrays and uses an async invocation scope. Harness snapshots run outside that scope; cleanup restores property descriptors. Unobserved I/O cannot be claimed as verified.

## What is tested

[src/diagnostics.test.ts](src/diagnostics.test.ts) checks actual clock sampling, a supplied Date and rejection of an invalid sample. [resource-observer.test.ts](.dctx0/testing/typescript/resource-observer.test.ts) checks real reads/writes/removals, async scope, rejected attempts, cleanup and missing-storage rejection. Rust assesses recorded effects against permissions; a broken clock result must fail.

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
./examples/tutorial/agent-step.sh 10
# Optionally execute an agent in that new exercise directory:
./examples/tutorial/agent-step.sh 10 -- codex exec --full-auto
```

The exercise supplies this step’s spec, testing adapter and acceptance tests, but starts production code at step 9. The full reference in Git is preserved. Previous SQLite history and inspection approvals are omitted; manual requirements need a fresh substantive review. See the [series guide](../tutorial/README.md) for full-series, mutation and replay commands.
