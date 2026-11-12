# Connect native application tests

## Choose the backend and scope

New Go and TypeScript/Bun connections use application-owned native tests with
central Rust assessment. Keep existing adapters configured and running; move
only the explicitly requested subjects. Rust's existing adapter is executable,
but a Rust application-test library is not shipped. Java retains profiles only.
Library version, application protocol, worker SDK and storage schema are separate.

Identify SPEC, APP, target, production boundary and a usable native fixture.
Read the current config and editable profiles; `verify discover` inventories
structure, not behavior. `check SPEC` compiles the spec; it does not test production.
Inspect the selected binary's subcommand help, especially in pinned consumers.
For a new project, `verify init SPEC --project APP --target go` (or `typescript`)
creates the technical config and an explicitly unavailable legacy scaffold.
It preserves existing files and does not supply an application-test connection.
Keep that scaffold out of the assigned native scope; any remaining adapter cases
need a real adapter. Configure the intended full scope and budgets explicitly.

## Build the production connection

1. Publish the pinned testing-only library:
   `dctx0 verify application-tests library --project APP --target go`
   (or `--target typescript`). Owned library files and authored tests are separate;
   publication preserves existing tests and refuses edited owned files.
   Files land in `.dctx0/application-tests/library` with `library-pin.json`.
   Go uses `github.com/Haardt/dctx0ctl/applicationtests/go v0.2.0` with a local
   `replace ... => ./.dctx0/application-tests/library`; Bun can import the local
   library directly from authored test files. Configure testing dependencies and frozen
   dependencies explicitly. Production never imports this testing package.
2. Author a native root test and typed fixtures. Go uses `testing.T` and
   `Client.DecoderTest`, `OperationTest`, `StateChangeTest`, `AssertionTest`,
   `ScenarioTest`, `LifecycleTest`, `IdentityTest`. Bun uses `AssessmentClient`
   and the corresponding lower-camel-case functions, including `constructorTest`.
   The library owns registration, central case iteration and finalization.
   Test code supplies actual production calls, fixtures and projections, not a
   second case generator, copied domain rules or expected-result dispatcher.
3. Connect the client using `DCTX0_APPLICATION_SOCKET`; read the central bundle
   at `DCTX0_APPLICATION_PLAN` when mapping types. Keep explicit case/session
   contexts, separate resources for parallel cases and awaited background work.
   Always finish and close in cleanup; preserve both central and native failures.
4. Decoder callbacks pass raw bytes unchanged to the actual production decoder,
   including invalid JSON/UTF-8. Go marks known production rejection with
   `Reject(reason)`; other errors/panics remain technical exceptions. Constructors
   witness the actual converted native input. Codec, witness and projector errors
   are binding failures, never evidence of production rejection. Preserve exact
   integer/decimal digits, Option and variant identity; report unsupported mappings.
5. Operation fixtures make direct typed production calls and project actual
   results or declared domain errors. State-change fixtures observe before →
   production call → after, with explicit scope, keys and completeness. Missing
   observers differ from observed empty state/trace. Observe real calls, resources
   and effects through actual dependency boundaries; injecting an observer must
   affect the production instance under test.
6. Supply typed local setup/oracle continuations needed by the selected cases;
   preserve resolved setup values. Pure oracle calls cannot satisfy production
   call obligations. Connected scenario/lifecycle steps share their fixture and
   real persistence. Identity checks need their actual source operations.
   Unsupported continuations remain incomplete; do not invent a string registry
   or synthetic successful observations to conceal them.

Use the shipped Go/Bun library API and Cinema examples as executable references:
`examples/cinema-go/contracttests` and
`examples/cinema/.dctx0/application-tests`. The Go fixture's local facade generator
is example-specific, not a general production-code generator supplied by the CLI.

## Prepare and execute current requirements

Prepare derives the mandatory inventory from the current spec; it checks and
publishes a snapshot but does not execute or deliver tasks. Use a new plan path
when retaining previous runs. GROUP and ROOT must match actual native registrations.

```sh
dctx0 verify application-tests prepare SPEC --project APP \
  --plan .dctx0/application-tests/current.json --group native \
  --test-name ROOT --form all --boundary json-decoder --group-seconds 300
```

For a Bun constructor boundary use `--boundary value-constructor`. In mixed
projects, repeated `--subject QUALIFIED_NAME` assign those subjects' supported
cases to the native group; the remaining central cases retain the adapter group.
Inspect the generated assignment for full, unique coverage, connected paths and
matching test forms. Backend selection must not change IDs, inputs, seeds or budgets.
An ordinary new plan need not equal a historical reference plan. Preserve bindings,
regenerate from the current spec and execute; report actual incompatibilities.
For an explicitly frozen execution retain its requested SHA-256 check.

Author a schema-1 native recipe with runtime, root_test, test_directory,
prepare/build/catalog/run commands and junit (Bun). Recipes execute inside captured
sources, not ambient dependency directories. `{plan}` and `{artifacts}` are engine
placeholders. Go recipes prepare/vendor dependencies, build `go test -c`, collect
with `-test.list=.` and run through `go tool test2json`. Bun recipes use a frozen
offline install, typecheck, the shipped `library/catalog.ts` and native JUnit.
Use the Cinema recipes for complete schemas; include all transitive helpers,
fixtures, production sources, toolchain/build inputs and explicit external inputs.
Provision actual database/container fixtures with bounded startup and owned cleanup.
Keep their directories private and alive for the entire run; do not sweep shared
temporary directories while verification is active.
Use `verify preflight SPEC --project APP` with the same scope and delivery intent
before execution; `--diagnostic` explicitly plans a diagnostic run. It checks
declared bindings and task prerequisites, not live fixture readiness. Inspect
the prepared mixed assignment as well; a legacy preflight is not proof that a
native group was collected or executed.

```sh
dctx0 verify application-tests run SPEC --project APP \
  --plan .dctx0/application-tests/current.json --group native \
  --recipe .dctx0/application-tests/native-run.json \
  --report .dctx0ctl/application-tests/UNIQUE.json --no-adopt
```

For a mixed cohort substitute `hybrid` for `run`. Without reuse flags it executes
both assigned backends freshly. `adapter-group` diagnoses one assigned adapter group only and
cannot certify the complete cohort. The current managed hybrid supports one
adapter and one application-test group of the same target. Do not silently
switch the whole project to legacy or narrow the scope to get a green result.

`--no-adopt` is diagnostic, not task delivery. For delivery, use `run` / `hybrid`
without that flag once the actual task/profile prerequisites are ready.
`complete` / `hybrid-complete` additionally require current source-bound authored
inspections; hybrid-complete requires `--notes PATH` and has no `--no-adopt`.
Do not generate review approval to bypass missing execution.

## Read results and maintain compatibility

Report mandatory, recorded PASS/FAIL/UNKNOWN and missing cases separately, plus
open authored obligations, source freshness, task deliveries and time/RSS.
A green native parent alone is insufficient: collection, journal, finalization,
native exit/assertions, source/build binding and full group coverage must agree.
Environment aborts leave unexecuted cases open, not failed and not passed.
Save the run ID and export needed evidence before retention can remove details.
Replay uses the owning native test and saved inputs; it does not deliver tasks.

The legacy backend continues using existing configuration and `verify run SPEC
--project APP` / `verify complete SPEC --project APP`. `--legacy` belongs only to
wrappers which explicitly provide it. Upgrading dctx0 never requires migrating
all adapters, changing business specs, rewriting old configs or erasing history.
Old PASS remains evidence for its old inputs; a new task ID alone does not imply
missing production behavior. Validate existing code before changing it.
Incremental reuse requires the tool's matching current evidence bindings; do not
infer reuse or task authority from a cached PASS or matching case count.
`--reuse-inputs PATH` captures an explicit
deterministic unit and external-input declaration; add `--reuse-from RUN_ID`
to select a same-project parent. These flags work with `run` and `hybrid` as
well as legacy `verify run`. Read `.incremental` for actual reused and executed
counts. Non-contract cases form one coarse unit requiring an explicit snapshot
or live revision binding. Hybrid still executes the application-test group.
Eligible complete passing evidence can deliver tasks only after lineage and the
current task guards are verified; add `--no-adopt` for diagnostic runs. Historical
reuse is not a fresh external observation, and the input declaration does not
sandbox live services or the process. Authored inspection and review completion
flows continue to require fresh evidence.
Follow the repository's `docs/site/src/reference/incremental-verification.md`
for declaration fields, compatibility conditions and limits.
