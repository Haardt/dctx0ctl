# Step 4: Expressions, operators and construction

This directory retains the complete specification and implementation from [step 3](../tutorial-step-3-data/README.md), then adds the features below.

## What this step adds

This step adds arithmetic and Boolean operations, range/list membership, string concatenation and variant construction. Preconditions define legal inputs; several postconditions describe the same result from different angles.

## Specification extension

The following excerpts are copied from this step’s actual spec. Surrounding declarations are retained in the complete files: [workspace.dctx0](dctx0/workspace.dctx0).

```dctx0
operation score(a: Count, b: Count) -> Int {
  pure
  requires b > 0
  ensures result == (a + b) * 2 - a
  ensures result > 0 and result / result == 1
  ensures result % 2 == a % 2
  ensures -result < 0
}
```

```dctx0
operation eligible(count: Count) -> Boolean {
  pure
  ensures result == (not (count == 0) and (count <= 2 or count >= 3))
  ensures result == (count in 1..3)
  ensures result == (count in [1, 2, 3])
  ensures result implies count != 0
}
```

```dctx0
operation named(title: Title) -> Choice {
  pure
  ensures result == Named { value = title }
}
```

```dctx0
example "arithmetic and construction" {
  when score = Actions.score(2, 3)
  when label = Actions.describe("Read")
  when choice = Actions.named("Read")
  expect score == 8
  expect label == "Task: Read"
  expect choice is Named { value = "Read" }
}
```

The score contract exercises arithmetic precedence, division, remainder, comparisons and unary negation. Eligibility combines `not`, `and`, `or`, `implies` and membership. `Named { value = title }` constructs a payload alternative. The example uses sequential `when` bindings and checks values as well as a variant pattern with `is`. The original invariant continues to demonstrate operation calls inside expressions.

## TypeScript implementation

[src/expressions.ts](src/expressions.ts) supplies the real score, eligibility, label and constructor functions, using the existing production factories.

## What is tested

[src/expressions.test.ts](src/expressions.test.ts) enumerates valid finite score inputs, rejects invalid counts and checks eligibility, string output and construction. Native verification evaluates every executable clause and the multi-call example in Rust. Changing the score calculation must produce FAIL.

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
./examples/tutorial/agent-step.sh 4
# Optionally execute an agent in that new exercise directory:
./examples/tutorial/agent-step.sh 4 -- codex exec --full-auto
```

The exercise supplies this step’s spec, testing adapter and acceptance tests, but starts production code at step 3. The full reference in Git is preserved. Previous SQLite history and inspection approvals are omitted; manual requirements need a fresh substantive review. See the [series guide](../tutorial/README.md) for full-series, mutation and replay commands.
