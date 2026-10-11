# Step 14: Multiple files and domain fragments

This directory retains the complete specification and implementation from [step 13](../tutorial-step-13-external/README.md), then adds the features below.

## What this step adds

This step leaves the existing workspace intact and adds copy.dctx0. A fragment extends the existing Tutorial.Board domain instead of creating another workspace or duplicating its declarations.

## Specification extension

The following excerpts are copied from this step’s actual spec. Surrounding declarations are retained in the complete files: [copy.dctx0](dctx0/copy.dctx0) · [workspace.dctx0](dctx0/workspace.dctx0).

```dctx0
// This file extends the existing domain rather than declaring a second copy.
fragment Tutorial.Board {
  value CopyTitle = Tutorial.Board.Title
  surface Copy exposes requestReply {
    operation title(value: CopyTitle) -> CopyTitle {
      pure
      ensures result == value
    }
  }
  example "copy a reviewed title" {
    when result = Copy.title("Read")
    expect result == "Read"
  }
}
```

`fragment Tutorial.Board` selects the existing qualified owner. CopyTitle uses the qualified reference `Tutorial.Board.Title`, so its refinements remain inherited across files. The added Copy surface exposes a pure identity operation and a concrete example. The compiler composes all .dctx0 sources before resolving names and checking contracts.

## TypeScript implementation

[src/copy.ts](src/copy.ts) implements the operation using the existing production title validator. The adapter adds its entry point without replacing any earlier operation.

## What is tested

[src/copy.test.ts](src/copy.test.ts) checks valid titles and inherited rejection of invalid titles. Native verification compiles the combined workspace and executes the fragment example and operation cases. Returning a constant title must fail.

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
./examples/tutorial/agent-step.sh 14
# Optionally execute an agent in that new exercise directory:
./examples/tutorial/agent-step.sh 14 -- codex exec --full-auto
```

The exercise supplies this step’s spec, testing adapter and acceptance tests, but starts production code at step 13. The full reference in Git is preserved. Previous SQLite history and inspection approvals are omitted; manual requirements need a fresh substantive review. See the [series guide](../tutorial/README.md) for full-series, mutation and replay commands.
