# Two architectures, one specification

These examples implement the same finite Count/increment specification from [tutorial step 1](../tutorial-step-1-values/README.md). They illustrate an explicit implementation-profile choice without changing the language or its behavioral expectations. They are small architecture alternatives, not duplicate 16-step series.

| Variant | Production architecture | Actual native verification entry point |
| --- | --- | --- |
| [Direct Bun](bun-direct/README.md) | Ordinary counter functions and a Bun Request/Response handler | Real HTTP POST /increment |
| [Hono + Awilix, classes](hono-awilix-classes/README.md) | Class methods, Hono routing and Awilix per-request scopes | Real HTTP POST /increment through a resolved controller/service |

Both use bigint internally and decimal integer strings over HTTP. Both retain Count 0..3, the increment precondition, its postcondition, the concrete example and the quantified invariant. The adapters invoke the actual HTTP path and call actual production factories for contract validation; they contain no alternative implementation of increment.

```sh
# From the repository root, with the Rust release CLI already built:
bun examples/tutorial/verify-variants.ts
```

The runner performs frozen installation, strict type checking, independent tests with 100% measured production line/function coverage and native complete verification. It compares the snapshots/plans, exercises the tutorial reset/agent helpers and requires a broken increment implementation in each isolated variant to produce an actual native FAIL through HTTP. It measures speed and RSS last. [validation.json](validation.json) retains the captured evidence, mutation counterexamples and measurement scope. New workloads remain unassessed against the accepted repository baseline.

The classes variant explicitly selects Hono/Awilix in AGENTS.md and editable target profiles. dctx0 does not impose these frameworks on other applications. See [agent guidance and profiles](../tutorial/README.md#agent-guidance-and-editable-profiles) before using a profile to steer an exercise.
