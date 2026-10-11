# Step 5: Collections, strings and exact decimals

This directory retains the complete specification and implementation from [step 4](../tutorial-step-4-expressions/README.md), then adds the features below.

## What this step adds

This step adds the registered collection and string functions, conditional option construction, and exact decimal arithmetic. Collection operations are pure and preserve their input; cardinality-dependent operations state their preconditions explicitly.

## Specification extension

The following excerpts are copied from this step’s actual spec. Surrounding declarations are retained in the complete files: [workspace.dctx0](dctx0/workspace.dctx0).

```dctx0
operation positive(values: List<Count>) -> List<Count> {
  pure
  ensures result == filter(value in values where value > 0)
}
```

```dctx0
operation doubled(values: List<Count>) -> List<Int> {
  pure
  ensures result == map(value * 2 for value in values)
}
```

```dctx0
operation only(values: List<Count>) -> Count {
  pure
  requires count(values) == 1
  ensures result == single(values)
}
```

```dctx0
operation fallback(values: List<Count>) -> Int {
  pure
  ensures result == getOrElse(first(values), -1)
}
```

```dctx0
operation optional(value: Count, present: Boolean) -> Option<Count> {
  pure
  ensures result == ifElse(present, Some(value), None())
}
```

```dctx0
operation halfEven(amount: ExactAmount) -> ExactAmount {
  pure
  ensures result == round(amount, 2, HalfEven)
}
```

```dctx0
example "signed decimal ties remain exact" {
  when even = Actions.halfEven(2.345)
  when away = Actions.halfUp(-2.345)
  when third = Actions.third(1.0)
  expect even == 2.34
  expect away == -2.35
  expect third == 0.33
}
```

The full source also covers count, sum, all, any, contains, distinct, min, max, sorting in both directions, first, mean, indexOf, reverse and slice. String contracts cover lowercase, uppercase, trim, length, startsWith, endsWith and includesText. `ifElse`, `Some` and `None()` construct options. Decimal round supports Down, Up, HalfUp and HalfEven; divide makes its scale and rounding mode explicit. Exact finite quotients are retained; repeating quotients are rounded.

## TypeScript implementation

[src/collections.ts](src/collections.ts) implements lists and strings. [src/decimal.ts](src/decimal.ts) uses exact integer arithmetic rather than floating-point approximations for decimal results.

## What is tested

[src/collections.test.ts](src/collections.test.ts) checks empty/nonempty inputs, order, distinctness, selection, missing indexes, text semantics and optional values. [src/decimal.test.ts](src/decimal.test.ts) checks positive/negative ties, every rounding mode, digits beyond JavaScript safe integers and finite/repeating division. Native clauses compare the actual output with the Rust evaluator. Mutating doubled values must fail.

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
./examples/tutorial/agent-step.sh 5
# Optionally execute an agent in that new exercise directory:
./examples/tutorial/agent-step.sh 5 -- codex exec --full-auto
```

The exercise supplies this step’s spec, testing adapter and acceptance tests, but starts production code at step 4. The full reference in Git is preserved. Previous SQLite history and inspection approvals are omitted; manual requirements need a fresh substantive review. See the [series guide](../tutorial/README.md) for full-series, mutation and replay commands.
