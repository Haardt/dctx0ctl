# Step 1: Values and a pure operation

This is the starting point: a complete small TypeScript application, its specification and an external verification adapter.

## What this step adds

The board starts with a counter rather than stored tasks. This step introduces the `workspace` and `domain` containers, an explanatory `intent`, a refined `value`, and a public `surface` containing a pure operation. `Count` has exactly four legal values: 0, 1, 2 and 3. The precondition excludes incrementing 3; the postcondition defines the result for every valid call.

## Specification extension

The following excerpts are copied from this step’s actual spec. Surrounding declarations are retained in the complete files: [workspace.dctx0](dctx0/workspace.dctx0).

```dctx0
value Count = Int {
  range 0..3
}
```

```dctx0
operation increment(count: Count) -> Count {
  pure
  requires count < 3
  ensures result == count + 1
}
```

```dctx0
example "increment one" {
  when result = Actions.increment(1)
  expect result == 2
}
```

```dctx0
invariant IncrementIsMonotone {
  forall count: Count
  assert count < 3 implies Actions.increment(count) > count
}
```

An `example` supplies one concrete call and expected result. The `forall` invariant ranges over the finite Count contract. Its `implies` guard prevents calling `increment` when its precondition is false. Intent text explains the design; it adds no hidden assertions.

## TypeScript implementation

[src/board.ts](src/board.ts) implements the counter using `bigint`. The external adapter translates values and invokes that function; Rust evaluates the specification expectations.

## What is tested

[src/board.test.ts](src/board.test.ts) accepts all four legal counts, rejects out-of-range counts, checks the real increment result and rejects a full board. Native verification checks the example, generated valid calls, result contracts and the quantified invariant. The series mutation test changes increment and requires an actual failing case.

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
./examples/tutorial/agent-step.sh 1
# Optionally execute an agent in that new exercise directory:
./examples/tutorial/agent-step.sh 1 -- codex exec --full-auto
```

The exercise supplies this step’s spec, testing adapter and acceptance tests, but starts production code at an empty implementation. The full reference in Git is preserved. Previous SQLite history and inspection approvals are omitted; manual requirements need a fresh substantive review. See the [series guide](../tutorial/README.md) for full-series, mutation and replay commands.
