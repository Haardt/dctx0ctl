# Functional-core operations

Use a direct `pure` clause on a module-owned surface operation when the outcome must depend only on explicit inputs. The algorithm remains in application code. The specification declares its boundary and properties.

<!-- dctx0:check -->
```dctx0
workspace Calculations {
  domain Numbers {
    module Arithmetic {
      intent "Compute deterministic values from explicit inputs."
      surface Rules {
        operation double(value: Int) -> Int { pure }
      }
      invariant DoublesTheInput {
        forall value: Int
        assert Rules.double(value) == value * 2
      }
    }
    test Properties
  }
}
```

The external adapter binds `double` to a normal application operation. Evaluation can suspend at a pure application call; the managed worker executes it and returns observations to the Rust evaluator. A declared failure or invalid result is not silently accepted as a value.

## Purity is an obligation

Pure behavior must avoid hidden I/O, clock access, randomness, environment dependence, mutable globals, and input mutation. Compiler rules restrict modeled effects and calls. A declaration alone cannot prove every transitive library is pure; structural inspection and native finite cases have explicit limits.

A missing behavioral observer or unsupported projection remains a gap, not probable success. Source-bound review can assess a qualitative obligation within its current inputs, while preserving the origin of that evidence.

## Finite execution

Core generation is deterministic and target-neutral. Seeds, case budgets, constraints, and unsupported obligations are pinned in the plan. Target workers execute actual calls and report observations; central Rust code validates result contracts and invariant assertions.

Shrinking and replay semantics are centrally owned. The current CLI supports `verify replay <spec-root> <run-id> --case <case-id>` with optional `--minimized`. Replay never adopts tasks.

Use pure invariants for value rules. State-dependent behavior belongs in operation contracts and lifecycle paths. [Native targets](../targets.md) include TypeScript/Bun, Go, and Rust; Java currently supplies profiles only.
