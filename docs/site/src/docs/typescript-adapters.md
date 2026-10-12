# TypeScript adapters

The connection to TypeScript is an external **testing adapter**, separate from the production application. The application owns its types, dependencies, composition root, and framework integration.

Technical configuration lives in `.dctx0/testing/verification.json`. The generated adapter contract defines the supported test interface.

## Testing area

```text
.dctx0/testing/
├── verification.json
└── typescript/
    ├── adapter.ts
    ├── contract.ts
    ├── examples.json
    └── worker.ts             # generated testing transport
```

`verify init --target typescript` creates missing artifacts while preserving authored files. Its unavailable scaffold deliberately cannot certify an application.

## Adapter responsibilities

The adapter describes its capabilities, starts isolated sessions, calls real production entry points, translates values, and exposes actual results and observations through the SDK contract. Read the generated `contract.ts` and the [Cinema adapter source](https://github.com/Haardt/dctx0ctl/blob/main/examples/cinema/.dctx0/testing/typescript/adapter.ts) for the current versioned types.

Independent cases receive fresh sessions. Lifecycle steps share a session. Declared collaborator replies can be injected through real dependency boundaries. Calls, events, resources, and states must come from actual observers; imports alone do not establish runtime behavior.

Do not reconstruct the expected business result in the adapter. That would test a second implementation of the specification rather than the application.

## Wire values and failures

Translate application-owned values to the language-neutral SDK representation. Preserve exact numbers and tagged outcomes. Unsupported native representations should be reported explicitly rather than rounded or guessed. Application exceptions and adapter/translation failures have different evidence meanings.

Examples require concrete typed values for symbolic inputs. These fixtures are separate from expected behavior: the specification supplies the assertions.

## Structural discovery

```sh
dctx0 verify discover <spec-root> --project <app-root> --target typescript
```

Tree-sitter helps inventory exports, imports, and candidate boundaries. It does not bind them automatically or prove semantic type conformance. Review candidates and wire the actual entry points in your adapter.

## Configuration and execution

```sh
dctx0 verify init <spec-root> --project <app-root> --target typescript \
  --subject <qualified-name>
dctx0 verify plan <spec-root> --project <app-root>
dctx0 verify run <spec-root> --project <app-root> --no-adopt
```

The configuration selects explicit subjects/tasks and deterministic budgets. An empty scope is not whole-application verification. Prepare Bun dependencies before execution.

For real storage and transport assurance, include persistence/HTTP checks. Mock replies establish consumer behavior under those replies, not the production provider's correctness. See [evidence](evidence.md), [workflow](workflow.md), and the [SDK/harness guide](https://github.com/Haardt/dctx0ctl/blob/main/docs/site/src/reference/bun-harness.md).
