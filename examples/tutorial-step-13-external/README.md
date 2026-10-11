# Step 13: An explicitly configured external HTTP service

This directory retains the complete specification and implementation from [step 12](../tutorial-step-12-lifecycle/README.md), then adds the features below.

## What this step adds

This step moves beyond the local clock to a real HTTP boundary. Directory.HealthApi returns a Health message; Connectivity.ready invokes it and returns its readiness flag.

## Specification extension

The following excerpts are copied from this step’s actual spec. Surrounding declarations are retained in the complete files: [workspace.dctx0](dctx0/workspace.dctx0).

```dctx0
message Health { ready: Boolean }
```

```dctx0
external Directory {
  intent """Query an explicitly configured local HTTP service."""
  surface HealthApi {
    operation status() -> Health
  }
}
```

```dctx0
surface Connectivity exposes requestReply {
  operation ready() -> Boolean {
    effects { observe Directory.HealthApi.status }
  }
}
```

```dctx0
example "configured service is ready" {
  when result = Connectivity.ready()
  expect result == true
}
```

The external declaration defines a contract, not a URL or automatic exposure. The example selects a concrete ready scenario. `observe Directory.HealthApi.status` identifies the external interaction; an explicit project adapter/configuration supplies the actual local service and its address.

## TypeScript implementation

[src/external.ts](src/external.ts) contains the real local handler, HealthClient and Connectivity consumer. Test sessions start the service on an available loopback port and stop it during cleanup. A supplied endpoint is validated rather than inferred from symbol names.

## What is tested

[src/external.test.ts](src/external.test.ts) sends actual HTTP requests, checks the returned Health data and rejects HTTP errors and malformed payloads. Native verification invokes the configured client/consumer against the real service. A consumer that ignores readiness must fail. This does not claim conformance of arbitrary deployed services.

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
./examples/tutorial/agent-step.sh 13
# Optionally execute an agent in that new exercise directory:
./examples/tutorial/agent-step.sh 13 -- codex exec --full-auto
```

The exercise supplies this step’s spec, testing adapter and acceptance tests, but starts production code at step 12. The full reference in Git is preserved. Previous SQLite history and inspection approvals are omitted; manual requirements need a fresh substantive review. See the [series guide](../tutorial/README.md) for full-series, mutation and replay commands.
