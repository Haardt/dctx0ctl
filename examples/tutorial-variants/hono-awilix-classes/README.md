# Hono + Awilix: production behavior in classes

This variant uses Hono and Awilix (the dependency injection library) to implement the same counter spec as [direct Bun](../bun-direct/README.md). Production has no standalone function declarations, function expressions or arrow functions. Classes, class methods, types, constants and bound method callbacks are allowed. Tests and the external adapter remain separate from this production style constraint.

## Specification and implementation

The complete [spec](dctx0/workspace.dctx0) is identical to tutorial step 1 and the direct variant:

```dctx0
value Count = Int {
  range 0..3
}
operation increment(count: Count) -> Count {
  pure
  requires count < 3
  ensures result == count + 1
}
```

The containing Actions surface, explicit example and forall invariant are retained. Architecture is selected in [AGENTS.md](AGENTS.md) and [editable profiles](.dctx0/profiles/typescript/common.md); the spec does not require a DI framework.

[src/board.ts](src/board.ts) defines CounterService.count/increment. [src/app.ts](src/app.ts) defines CounterController and Application. Application creates a strict Awilix container using CLASSIC injection, registers scoped services before resolving them and binds a class method to Hono POST /increment. Each request creates its own scope, resolves a controller with its CounterService and disposes that scope in finally. Stopping Application stops the Bun server and disposes the root container. [src/server.ts](src/server.ts) is the actual executable entry point.

The controller accepts `{"count":"1"}` and returns `{"result":"2"}`. Decimal integer strings preserve the bigint representation. Expected validation failures return 400. Hono returns 404 for routes/methods with no matching handler; the direct Bun variant explicitly uses 405 for an unsupported method. This transport difference does not alter the shared increment contract.

## What is tested

[src/app.test.ts](src/app.test.ts) checks the complete Count domain, real HTTP calls, malformed input, boundaries, routing, unexpected error propagation and server startup/cleanup. Additional tests check per-request service creation/disposal, identity within a scope and the absence of standalone functions/arrows in the three production sources. These inspect the actual source files and supplement the authored architecture inspection; free-form profile text is not automatically tested.

The native adapter calls the real CounterService validator and sends real HTTP requests through Hono and Awilix. It does not replace CounterService or return expected constants. The shared runner demands the same native plan as direct Bun and 100% measured production lines/functions.

## Run

```sh
cd examples/tutorial-variants/hono-awilix-classes
bun install --frozen-lockfile
bun run typecheck
bun run test --coverage
../../../target/release/dctx0 verify complete dctx0 --project .
PORT=4181 bun run start
```

In another terminal:

```sh
curl -H 'content-type: application/json' -d '{"count":"1"}' http://127.0.0.1:4181/increment
```

Dependencies are pinned in bun.lock. To compare both architectures, run `bun examples/tutorial/verify-variants.ts` from the repository root. [inspection-notes.json](.dctx0/testing/typescript/inspection-notes.json) records the reviewed captured sources. After changing code, inspect the new implementation before updating those notes.
