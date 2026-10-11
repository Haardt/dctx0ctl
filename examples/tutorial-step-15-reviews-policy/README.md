# Step 15: Review, named suites and release policy

This directory retains the complete specification and implementation from [step 14](../tutorial-step-14-fragments/README.md), then adds the features below.

## What this step adds

This step adds verification.dctx0 with explicit review criteria, a question, all named proof/test suites and a Release policy. A separate Catalog fragment also records ownership of Note. These declarations connect behavioral evidence with reviewed architectural requirements.

## Specification extension

The following excerpts are copied from this step’s actual spec. Surrounding declarations are retained in the complete files: [copy.dctx0](dctx0/copy.dctx0) · [verification.dctx0](dctx0/verification.dctx0) · [workspace.dctx0](dctx0/workspace.dctx0).

```dctx0
fragment Tutorial.Board.Catalog {
  owns { entity Note }
}
fragment Tutorial.Board {
  review BoundaryReview {
    intent """Review actual boundaries alongside executable behavior evidence."""
    inspect {
      implementation
      dependencyGraph
      decisions
      surfaces
      ownership
      contracts
    }
    criteria {
      "Catalog depends on the normal reader interface and returns public messages."
      "Mocks replace collaborators while the actual consumer remains under test."
      "State and effect observations come from actual application sessions."
      "External endpoints are explicitly configured and test resources are cleaned up."
    }
    question BoundaryIntegrity {
      """Do any production modules import spec files, dctx0 runtime or generated testing artifacts?"""
    }
  }
  proof TypeConformance
  proof SurfaceConformance
  proof ArchitectureConformance
  proof OwnershipConformance
  proof EffectConformance
  test Examples
  test Properties
  test AdapterConformance
  test ExternalConformance
  policy Release {
    require TypeConformance == PASS
    require SurfaceConformance == PASS
    require ArchitectureConformance == PASS
    require OwnershipConformance == PASS
    require EffectConformance == PASS
    require Examples == PASS
    require Properties == PASS
    require AdapterConformance == PASS
    require ExternalConformance == PASS
    require BoundaryReview != REQUEST_CHANGES
    require BoundaryReview.highFindings == 0
  }
}
```

`inspect` names review inputs; `criteria` and `question` state what a reviewer must assess. The suite declarations give TypeConformance, SurfaceConformance, ArchitectureConformance, OwnershipConformance, EffectConformance, Examples, Properties, AdapterConformance and ExternalConformance explicit names. Release requires every suite to PASS, no REQUEST_CHANGES and zero high findings. A review approval cannot override a native test failure.

## TypeScript implementation

Production code stays unchanged. [inspection-notes.json](.dctx0/testing/typescript/inspection-notes.json) records authored source/spec-bound inspections and substantive findings. After code changes, a reviewer must inspect the new implementation before updating evidence; copying old approval hashes is not a review.

## What is tested

All earlier production and native tests remain required. [mutation-acceptance.ts](../tutorial/mutation-acceptance.ts) additionally rejects a requested-change review, a high finding, a missing review and stale source evidence. Native manual obligations remain separate from the effective assessment that binds valid inspections and checks the policy. Architecture prose is not automatically converted into assertions.

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
./examples/tutorial/agent-step.sh 15
# Optionally execute an agent in that new exercise directory:
./examples/tutorial/agent-step.sh 15 -- codex exec --full-auto
```

The exercise supplies this step’s spec, testing adapter and acceptance tests, but starts production code at step 14. The full reference in Git is preserved. Previous SQLite history and inspection approvals are omitted; manual requirements need a fresh substantive review. See the [series guide](../tutorial/README.md) for full-series, mutation and replay commands.
