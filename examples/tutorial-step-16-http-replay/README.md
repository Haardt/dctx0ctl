# Step 16: Real HTTP routes, an interactive board and replay

This directory retains the complete specification and implementation from [step 15](../tutorial-step-15-reviews-policy/README.md), then adds the features below.

## What this step adds

This step adds http.dctx0, a local Bun server and a browser UI. Public list/find routes return summaries; an explicit administrative rows operation exposes stored status. Insert and replacement routes reuse the real writer semantics, allowing users to create tasks and switch between Open and Done.

## Specification extension

The following excerpts are copied from this step’s actual spec. Surrounding declarations are retained in the complete files: [copy.dctx0](dctx0/copy.dctx0) · [http.dctx0](dctx0/http.dctx0) · [verification.dctx0](dctx0/verification.dctx0) · [workspace.dctx0](dctx0/workspace.dctx0).

```dctx0
operation insert(request: Task) -> Task ! TaskConflict {
  effects {
    read Task
    write Task
  }
  state inserts Task from request
  ensures success implies result == request
  ensures failure is TaskConflict implies after.Task == before.Task
}
```

```dctx0
operation replace(request: Task) -> Task ! MissingTask {
  effects {
    read Task
    write Task
  }
  state replaces Task by request.id from request
  ensures success implies result == request
  ensures failure is MissingTask implies after.Task == before.Task
}
```

```dctx0
example "HTTP finds an actual stored task" {
  when result = Http.find(httpId)
  expect result == Some(TaskSummary { id = httpId title = "Read" })
}
```

The Http fragment explicitly declares operations, effect permissions, selected state and postconditions. It does not derive URLs from the spec: application code maps GET /tasks, GET /tasks/:id, GET /board/tasks, POST /tasks and PATCH /tasks/:id. The existing-key example exercises a real stored task. Replay reuses a saved case rather than regenerating different inputs; changed code is a new comparison with its own provenance.

## TypeScript implementation

[src/http.ts](src/http.ts) implements the actual handler/client. [server.ts](server.ts) starts it with three in-memory tasks. [src/ui.ts](src/ui.ts) renders the board, creates tasks, writes status changes, loads details and handles errors. Safe text rendering, failed-write restoration and protection against stale detail responses are part of the implementation. The external adapter starts fresh local HTTP sessions and correlates actual server accesses with invocations.

## What is tested

[src/http.test.ts](src/http.test.ts) exercises real routing, public/admin payloads, successful writes, duplicate/missing errors, malformed requests, path/body mismatches and client payload validation. [server.test.ts](server.test.ts) imports and stops the actual entry point on an ephemeral port. Native Http cases check results and real state. Separate [browser checks](../tutorial/ui-browser-check.ts) cover real UI interaction, retries, empty/error states, delayed-response races, persisted writes and narrow-screen layout. The series mutations detect insert/replace incorrectly implemented as upsert; saved HTTP counterexamples are replayed before and after repair.

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

To use the board:

```sh
bun run start
```

Open <http://127.0.0.1:4173/>. `PORT=4180 bun run start` selects another port. Create tasks with the form and use the checkbox to mark them Done or reopen them. Titles retain the 16 UTF-16-unit contract. Restarting restores the three seed tasks; this example stores changes in memory.

Browser acceptance is separate from Bun coverage of the HTML-generating module. Start a local Chromium debugger, then run the checker from the repository root:

```sh
chromium --headless --no-sandbox --disable-gpu --remote-debugging-port=9228 \
  --user-data-dir=/tmp/dctx0-tutorial-browser about:blank
# In a second terminal:
bun examples/tutorial/ui-browser-check.ts
```

Stop the debugger afterward. `BROWSER_DEBUG_URL` selects its address; `UI_SCREENSHOT` selects the screenshot path. The retained browser report includes actual V8 function/range coverage; this is distinct from Bun production coverage and is not full branch coverage.

## Reimplement this step with an agent

From the repository root:

```sh
./examples/tutorial/agent-step.sh 16
# Optionally execute an agent in that new exercise directory:
./examples/tutorial/agent-step.sh 16 -- codex exec --full-auto
```

The exercise supplies this step’s spec, testing adapter and acceptance tests, but starts production code at step 15. The full reference in Git is preserved. Previous SQLite history and inspection approvals are omitted; manual requirements need a fresh substantive review. See the [series guide](../tutorial/README.md) for full-series, mutation and replay commands.
