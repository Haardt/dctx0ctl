# Step 2: Aliases, refinements and identity

This directory retains the complete specification and implementation from [step 1](../tutorial-step-1-values/README.md), then adds the features below.

## What this step adds

On top of the counter, this step adds validated text, identities and collections. `scalar` introduces a simple named string type. `value` adds restrictions; an alias such as `DisplayTitle = Title` inherits the complete contract instead of bypassing validation.

## Specification extension

The following excerpts are copied from this step’s actual spec. Surrounding declarations are retained in the complete files: [workspace.dctx0](dctx0/workspace.dctx0).

```dctx0
value Title = String {
  length 1..16
  where length(trim(value)) > 0
}
```

```dctx0
value DisplayTitle = Title
```

```dctx0
value Email = String {
  length 3..254
  format Email
}
```

```dctx0
value Identifier = String { format UUID }
```

```dctx0
value TaskId = UUID { stableId "tutorial.task-id.v1" }
```

```dctx0
value Link = String { format Uri }
```

```dctx0
value Day = String { format Date }
```

```dctx0
value Instant = String { format DateTime }
```

```dctx0
value ShortList = List<Int> { length 1..2 }
```

```dctx0
operation echo(title: DisplayTitle) -> DisplayTitle {
  pure
  ensures result == title
}
```

`length` bounds strings or collections, while `where` expresses an executable predicate. Title must contain non-whitespace text but is returned unchanged. String lengths follow UTF-16 units. `format` covers Email, UUID, Uri, Date and DateTime. `stableId` describes the TaskId data contract; it is not universal declaration identity. The formatted string Identifier and the UUID builtin TaskId deliberately have different validation rules; see the semantics notes.

## TypeScript implementation

[src/board.ts](src/board.ts) adds factories and the actual echo operation. Application code retains its own types and validators.

## What is tested

[src/refinements.test.ts](src/refinements.test.ts) checks blank and overlong titles, inherited validation, email/UUID boundaries, absolute URLs, calendar rollover, timestamp offsets and precision, list cardinality, element validity and copying. Native verification generates values through the declared contracts and checks echo against its alias result contract. A title-length mutation must fail.

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
./examples/tutorial/agent-step.sh 2
# Optionally execute an agent in that new exercise directory:
./examples/tutorial/agent-step.sh 2 -- codex exec --full-auto
```

The exercise supplies this step’s spec, testing adapter and acceptance tests, but starts production code at step 1. The full reference in Git is preserved. Previous SQLite history and inspection approvals are omitted; manual requirements need a fresh substantive review. See the [series guide](../tutorial/README.md) for full-series, mutation and replay commands.
