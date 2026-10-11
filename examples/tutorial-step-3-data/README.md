# Step 3: Records, variants and builtin representations

This directory retains the complete specification and implementation from [step 2](../tutorial-step-2-refinements/README.md), then adds the features below.

## What this step adds

This step adds structured data. Status is a variant without payloads; Choice has an Empty alternative and a Named alternative carrying a Title. Details is a record whose fields compose existing contracts. Options, results and the remaining builtin types demonstrate how the external adapter can bridge ordinary TypeScript representations.

## Specification extension

The following excerpts are copied from this step’s actual spec. Surrounding declarations are retained in the complete files: [workspace.dctx0](dctx0/workspace.dctx0).

```dctx0
type Status = Open | Done
```

```dctx0
type Choice = Empty | Named(Title)
```

```dctx0
value Measure = Float { range 0..10 }
```

```dctx0
value Estimate = Duration { range 0..3600 }
```

```dctx0
value MaybeTitle = Option<Title>
```

```dctx0
value Attempt = Dctx0Result<Count, Title>
```

```dctx0
type Details {
  id: TaskId
  title: Title
  status: Status
  choice: Choice
  active: Flag
  progress: Measure
  estimate: Estimate
  amount: ExactAmount
  attachment: Attachment
  note: MaybeTitle
  labels: List<Title>
  attempt: Attempt
}
```

```dctx0
operation keepDetails(details: Details) -> Details {
  pure
  ensures result == details
}
```

```dctx0
operation keepAttempt(attempt: Attempt) -> Attempt {
  pure
  ensures result == attempt
}
```

`type Name { ... }` declares fields; `type Name = A | B(...)` declares alternatives. The complete source also introduces Timestamp, Int, Boolean, Decimal, Bytes, Unit, List, Option, Dctx0Option, Some, None and Dctx0Result aliases. `keepDetails` and `keepAttempt` require structural equality after the real operation returns.

## TypeScript implementation

[src/data.ts](src/data.ts) validates records and alternatives. Integers use `bigint`, decimals use exact strings, bytes use `Uint8Array`, and options use explicit tagged alternatives. Bytes are validated before conversion so an invalid element such as 256 cannot silently wrap to zero.

## What is tested

[src/data.test.ts](src/data.test.ts) checks exact numeric representations, invalid primitives, byte validation/copying, both option/result alternatives and every record field. Native verification exercises generated records and alternatives, applies refinements recursively and checks returned values. The Float-range mutation confirms that weakening a production validator is detected.

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
./examples/tutorial/agent-step.sh 3
# Optionally execute an agent in that new exercise directory:
./examples/tutorial/agent-step.sh 3 -- codex exec --full-auto
```

The exercise supplies this step’s spec, testing adapter and acceptance tests, but starts production code at step 2. The full reference in Git is preserved. Previous SQLite history and inspection approvals are omitted; manual requirements need a fresh substantive review. See the [series guide](../tutorial/README.md) for full-series, mutation and replay commands.
