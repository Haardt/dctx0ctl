# MiniBoard: 16 working applications

WorkshopHub teaches contract modeling. MiniBoard complements it with sixteen cumulative TypeScript applications, independent production tests, and native verification adapters.

Each step adds a language concept while preserving a working application:

| Step | Focus |
| --- | --- |
| 1 | Values and a first operation |
| 2 | Refinements |
| 3 | Records and variants |
| 4 | Expressions |
| 5 | Collections |
| 6 | Entities and identity |
| 7 | Boundaries |
| 8 | Declared collaborators and mocks |
| 9 | Failures and events |
| 10 | Effects |
| 11 | State |
| 12 | Lifecycles |
| 13 | External capabilities |
| 14 | Multi-file fragments |
| 15 | Reviews and policies |
| 16 | HTTP, task administration, and replay |


## Open the final application

From the checkout root:

```sh
cd examples/tutorial-step-16-http-replay
bun install --frozen-lockfile
bun run start
```

Open <http://127.0.0.1:4173/>. The page creates and updates actual tasks through native HTTP endpoints.

## Give your agent a fresh exercise

From the checkout root:

```sh
./examples/tutorial/agent-step.sh 1 -- codex exec --full-auto
```

Exercises are isolated under `target/tutorial-exercises`. Reference solutions remain intact. Read the [complete tutorial guide](https://github.com/Haardt/dctx0ctl/blob/main/examples/tutorial/README.md) for resets, empty mode, agent instructions, and per-step specifications/tests.

## Framework variants

Direct Bun and Hono/Awilix class-based HTTP examples implement the same finite step-1 contract. They demonstrate that architecture choices belong to implementation profiles and production composition, while the semantic plan stays shared.

[Variant sources and evidence](https://github.com/Haardt/dctx0ctl/tree/main/examples/tutorial-variants) include production mutations and performance records. They are two architecture examples, not another complete sixteen-step series.
