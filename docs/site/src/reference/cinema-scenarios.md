# Cinema examples through actual services

The central verifier now executes ordered example actions against the generated
Bun worker. The unchanged full Cinema spec projects twelve examples. All twelve
pass against the actual Catalog, Community and ScreeningPlanning methods in
`examples/cinema/src/application/movie-night.ts`, with 12 surface invocations,
16 observed collaborator requests, seven observed events and 19 expectations.

Production has application-owned TypeScript messages, errors and dependency
boundaries. It imports neither specs nor testing artifacts. The external reviewed
adapter at `.dctx0/testing/typescript/adapter.ts` supplies fresh services, value
translation and injected collaborators. It does not construct example answers:
the central Rust scenario prepares those from typed spec expressions and responds
only to actual collaborator requests. The testing-only `model.json` descriptor
and protocol artifacts are generated in the isolated acceptance workspace.
Public artifact setup/run commands are not implemented yet.

## Central semantics

`Scenario` is pure Rust state. It follows the pinned source action order, prepares
mock arguments and results, requests actual invocations, binds delivered results
and assesses success, declared failure payloads, assertions and emitted payloads.
Native results/events are checked against their declared contracts without
repairing incorrect representations. Each actual collaborator request and answer
is retained; unexpected targets/arguments or ambiguous prepared behavior cannot
produce a passing consumer proof.

`given` prepares behavior; it does not invent an exactly-one requirement.
Repeated matching requests are recorded and use the prepared response. An unused
prepared collaborator leaves the consumer proof UNKNOWN. This is a completeness
condition, not a fabricated runtime count. Explicit operation call-count clauses
still require separate operation-obligation execution. Missing or duplicated
mandatory scenario actions also prevent PASS.

Modeled numeric literals are projected through their retained types. Exact
Integers/Decimals may inhabit a Float field only when IEEE-754 conversion preserves
the exact value. Thus Cinema's 4.5 and 5 rating examples work without silently
rounding 0.1 or a large integer. This conversion applies to modeled values only;
native values still have to use the correct wire representation. Other Float
arithmetic remains an outstanding interpreter capability.

An adapter's rejected invocation promise is a technical failure. Applications
report an explicit observation: `invokeApplication` captures the actual call while
leaving translation/encoding outside its exception handler. The Cinema adapter
separately translates declared application failures, preserves unexpected
application exceptions and propagates its own translation failures technically.

## Managed execution

Each example starts a new session and ends it after assessment. A whole-case
deadline now bounds initialization, all requests and session cleanup together;
individual request limits and the worker's overall run deadline still apply.
Expiry aborts and reaps the managed process group. Tests demonstrate that several
individually short requests cannot evade the whole-case budget. Wrong begin,
invoke or end replies prevent successful execution.

The acceptance mutates only isolated production copies: wrong event payload,
removed event emission and wrong port wiring are detected. The reference spec and
application stay untouched. The regular Cinema TypeScript compiler check also
passes; its elapsed time and Linux wait4 peak RSS are retained separately. Compiler
RSS includes pre-exec process memory and is not incremental allocation.

## Scope and remaining work

This is consumer-example evidence with port mocks, not proof of production ports.
Actual persistence, Hono/Awilix composition, HTTP behavior, state/precondition/
postcondition obligations, explicit call counts, lifecycles and whole-plan
assessment remain required. The CLI still exposes planning only. Immutable native
run provenance/history, replay/shrink and guarded task adoption remain incomplete.
Transport failures currently return a technical error from the native example
API; the future whole-run report must also preserve partial scenario evidence.
Rust LLVM coverage does not measure the native/generated TypeScript helpers.

The complete scope and outstanding acceptance requirements remain tracked in
[the Cinema goal checklist](cinema-verification-progress.md).

## Retained validation

The complete coupled checkpoint passes 202 Rust/8 Python/4 Bun tests, with
14,526/14,526 merged Rust source lines and 1,232/1,232 LLVM functions. All 84
release speed/RSS workloads complete; reduced development settings and single
native measurements are unassessed against the accepted baseline. No baseline,
threshold or coverage exclusion changes were made.
Retained raw evidence (historical evidence removed during repository cleanup) includes native examples,
winner/transport samples, the native compiler check, reference source hashes and
earlier failed/incomplete runs. Rust instrumentation does not measure native
TypeScript coverage, and this checkpoint does not complete the Cinema goal.
