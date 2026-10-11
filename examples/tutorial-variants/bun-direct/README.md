# Direct Bun: ordinary functions and explicit HTTP

This variant adds a real HTTP boundary to the finite counter from tutorial step 1. It keeps normal exported functions for the business operation and uses Bun directly, with no Hono or dependency injection package.

## Specification and implementation

[dctx0/workspace.dctx0](dctx0/workspace.dctx0) is unchanged from step 1:

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

The operation belongs to Tutorial.Board.Actions. The full source also retains the example and monotonicity invariant. [src/board.ts](src/board.ts) contains the actual count/increment functions. [src/app.ts](src/app.ts) explicitly binds POST /increment to increment. [src/server.ts](src/server.ts) starts the server. URLs and HTTP status behavior are application decisions; they are not inferred from a spec name.

Send `{"count":"1"}` and receive `{"result":"2"}`. Integer strings avoid JSON bigint limitations and preserve exact values. Invalid JSON/types, out-of-range counts and incrementing 3 return HTTP 400. Unknown paths return 404 and unsupported methods return 405.

## What is tested

[src/app.test.ts](src/app.test.ts) covers every finite Count value, legal increment calls, invalid native values, real HTTP payloads/errors/routes, unexpected exception propagation and actual entry-point startup/cleanup. Native verification runs the shared spec through HTTP, including the example and invariant. The adapter calls the real factory for validation; Rust evaluates expectations. The shared runner requires 100% measured lines/functions for all three production files.

## Run

```sh
cd examples/tutorial-variants/bun-direct
bun install --frozen-lockfile
bun run typecheck
bun run test --coverage
../../../target/release/dctx0 verify complete dctx0 --project .
bun run start
```

POST to <http://127.0.0.1:4180/increment>. For example, in another terminal:

```sh
curl -H 'content-type: application/json' -d '{"count":"1"}' http://127.0.0.1:4180/increment
```

Use `PORT=4181 bun run start` to select another port. Stop the server before reusing its port. [AGENTS.md](AGENTS.md) and [common.md](.dctx0/profiles/typescript/common.md) select this architecture as agent guidance. They do not change the behavioral spec or guarantee testing PASS.
