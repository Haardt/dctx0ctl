# Bun application tests 0.2.0

This testing-only package uses application assessment protocol 2, independently
of target SDK 2 and worker protocol 1. Explicit asynchronous case handles bind
central iteration, observations, continuations and cleanup to one Rust session.
Production imports none of this package.

Supported library forms are `decoderTest`, `constructorTest`, `operationTest`,
`stateChangeTest`, `assertionTest`, `scenarioTest`, `lifecycleTest`, and the
central-only `identityTest`. Fixtures own direct typed production closures,
optional scoped setup/oracle calls, real state observers and cleanup. Connected
steps retain the same fixture. Central mock capabilities are updated from each
Case/Action and invoked only through the explicit `context.collaborate` handle.
Pure oracle calls never become production collaborator observations.

`constructorTest(client, testName, inputCodec, productionConstructor,
resultProjector, inputWitness)` requires a witness of the actual converted
native input before production. Codec/witness/projector errors are binding
errors, never production rejection. `projectInput` reconstructs actual native
values while using original Option/variant metadata only as representation
annotations. Cycles, accessors, unknown classes, unsafe rounded integer numbers
and unsupported representations remain incomplete. Safe integers and bigint
retain exact integer digits; Float spelling is preserved only for the same
actual finite f64 (`Object.is`). Actual operation arguments are witnessed too.
`decoderTest` passes raw bytes unchanged, including invalid UTF8 or JSON, to the
real production decoder; it does not decode test JSON first.

Cinema retains one explicitly lossy malformed-input mapping: for 46 non-Option
constructor rejection cases, Some(Unit) and None both become native undefined.
The actual Unit is reconstructed from undefined while the requested invalid
form remains an annotation. This is not native Option-payload distinguishability.
Receipts list these cases. An actual Option target whose present payload
collapses to undefined requires tagged options and otherwise remains UNKNOWN.

`catalog.ts` performs standalone Bun root registration collection by replacing
`bun:test` with recording functions before importing captured roots. It does
not execute test callbacks and is not an official `bun --list-tests` API.
Skipped/focused/duplicate roots and unsupported describe/each/hooks/concurrent
forms fail collection. Only registrations completed during the awaited import
are supported; deferred background registration is outside this collector's
contract. Central cases are journaled separately, not enumerated as individual
Bun tests. Independent native exit and JUnit must pass even after central PASS.
The bounded JUnit validator is engine-owned.

Focused library validation with elapsed time and process-tree RSS:

```
python3 scripts/memory_guard.py bun test --coverage --coverage-reporter=text --coverage-reporter=lcov --coverage-dir target/typescript-application-tests/v2-coverage targets/typescript/applicationtests
```

Bun's in-process helper coverage does not aggregate `catalog.ts` executed in
child processes. Its ten subprocess conformance checks are reported separately;
no source-line coverage is claimed for that script. The generated Cinema facade,
fixtures and HTTP checks likewise have native acceptance evidence, separate from
these library line metrics and from Rust coverage. No automatic adoption or
cross-group transactional authority follows from a library finish response.


Managed task delivery is separate from library finalization. Fresh
`dctx0 verify application-tests run` builds from captured sources, checks native
collection and provenance, stores evidence and attempts guarded task delivery
by default. Use `--no-adopt` for diagnostics without task delivery.
`verify application-tests complete` also checks source-bound authored inspection
notes against the prepared check and current profiles, using the established
completion and transactional task guards. A fresh Cinema complete acceptance
can deliver 105 Done/PASS tasks, including four explicit native-backed authored
review attestations. Plain review approval or manual completion alone does not
set PASS; already Done/Unverified reviews remain unchanged.

Only actual task delivery sets report `adoption_authority`. Preparation, standalone
service reports, saved/imported diagnostics, replay and a library finish response
cannot independently authorize adoption. See
[the workflow](../../../docs/site/src/docs/application-tests.md) for exact commands
and validation status; final workspace coverage and release acceptance are separate.
