# Native verification generators

Go, TypeScript/Bun and Rust share published requirements, deterministic case
planning and central Rust assessment. Generated code belongs in testing, never
in production. Generation supplies neither a production validator nor execution
evidence.

## Commands

Use a current published check and an initialized testing target:

```sh
dctx0 verify init SPEC --project APP --target go --subject Shop.Api
# Proposal only: no files, locks, migrations or task transitions.
dctx0 verify scaffold-codecs SPEC --project APP --target go --subject Shop \
  --dry-run --json
dctx0 verify scaffold-codecs SPEC --project APP --target go --subject Shop
dctx0 verify bindings scaffold SPEC --project APP --target go \
  --subject Shop.Api --dry-run --json
dctx0 verify bindings scaffold SPEC --project APP --target go --subject Shop.Api
```

Replace `go` with `typescript` or `rust` for those targets. Codec selection accepts
exact data declarations or namespaces and includes their type closure. Test
scaffolds accept subjects/tasks or the existing configured scope. They reuse the
configured fixture file, seed and budgets; an alternate `--fixtures` input is
rejected to prevent different inputs at generation and execution. Configure the
existing project fixture path before generating that scenario.

Existing files are always preserved. A report contains proposed contents even
when the existing file was preserved; it does not certify those files are equal.
Use `--candidate upgrade` to create inert `upgrade.<filename>.txt` proposals and
compare/merge them deliberately. They cannot add duplicate Go declarations or
activate Rust modules. `--output` on `scaffold-codecs` selects a separate basename
beside the existing native contract. There is no overwrite/force mode.

Codec generation retains TypeScript's `--integer`, `--option`, `--enum`, `--type`
and strict schema-1 `--config` options. CLI settings override configuration. The
same options are validated against each target's supported representations.

## Representations and authored boundaries

| Shape | Go | Rust | TypeScript |
| --- | --- | --- | --- |
| Record | `map[string]any` | JSON object | object |
| List | `[]any` | JSON array | array |
| Int / `number` | exact `json.Number` | exact `serde_json::Number` | checked safe integer |
| Int / `bigint` | `*big.Int` | unsupported; author a mapping | `bigint` |
| Option | nullable structural value | nullable JSON value | `null` or `undefined` |
| Payloadless enum | string or tagged map | string or tagged object | string or tagged object |
| Payload variant | `kind`/`payload` map | `kind`/`payload` object | `kind`/`payload` object |
| Decimal | exact decimal text | exact decimal text | exact decimal text |

Nested records, optional records and lists compose. UUID/Timestamp remain strings;
conversion does not validate their semantic format. Float/Duration require finite
numbers. Bytes use integer collections with byte bounds. Go and Rust require null
options; undefined is unsupported. Nullable options that collapse present values
into absence are explicitly unsupported. Builtins without a mapping, including
Go/Rust `Dctx0Result` in this increment, report conversion errors. No lossy
fallback exists.

Rust requires the existing testing manifest's `serde_json` arbitrary-precision
feature. Without it, an integer that cannot be preserved exactly returns an
error. Fixed-width application integers require an authored checked conversion;
never route through float64 or unchecked casts.

Go exports `Encode`, `Decode`, `DecodeInput`; Rust exports `encode`, `decode`,
`decode_input`; TypeScript retains `encode`, `decode`, `decodeInput`. Strict
conversion checks representation structure only. Input conversion retains
representable missing/extra fields and wrong primitive kinds. Unrepresentable
inputs remain unsupported, not production rejections.

For example, an authored Go validator binding can marshal `DecodeInput`'s
structural value and pass those bytes to the actual production decoder:

```go
native, err := DecodeInput(subject, supplied)
if err != nil { return ValueCheck{}, AdapterError(err) }
raw, err := json.Marshal(native)
if err != nil { return ValueCheck{}, AdapterError(err) }
_, err = protocol.DecodeContract(name, raw) // authored production binding
if err != nil { return ValueCheck{Kind: "rejected", Reason: pointerTo(err.Error())}, nil }
return ValueCheck{Kind: "accepted", Value: &supplied}, nil
```

Do not strictly decode a production struct before calling its parser for a
negative test. Do not catch conversion errors as business rejections. Nonstandard
native enum tags, union payload fields, exception types and integer types still
need explicit authored mappings. Production imports no generated helper.

## Outcome assertions

Go helpers return errors so their rejection behavior can be tested directly.
Generated `_test.go` wrappers use `testing.TB.Helper`/`Fatal`:

```go
// Before: bridgeFields requires a record and contradicts the failure assertion.
if observation.Value.Kind != "failure" { t.Fatal("expected failure") }
fields := bridgeFields(t, *observation.Value)

// After: the envelope and declaration are checked before reading its payload.
failure := requireDeclaredFailure(t, observation, err, prefix+"AccessDenied")
requireStringField(t, failure.Payload, "code", "PERMISSION_DENIED")
```

`ReturnedRecord` and `DeclaredFailure` are disjoint. Technical errors are supplied
separately and never inferred from error messages. Application exceptions,
unavailable observations, wrong declarations and malformed payloads cannot pass a
declared-failure assertion. Rust and TypeScript expose equivalent checks.
Payloadless failures may have null payloads; fields cannot be read from them.

`RecordedCalls`/`recorded_calls`/`recordedCalls` require an explicitly available
actual trace. Observed empty is different from absent. Their expected counts are
caller supplied; generation does not invent exactly-one obligations from permitted
calls or mock setup. Helpers cannot authenticate an authored observer: bind it to
actual collaborators and retain normal source-bound review.

The runner specification names `AccessDenied` and modeled call obligations, but
only types its `code` field as String. The literal `PERMISSION_DENIED` remains an
authored supplemental assertion, not a generated business requirement.

## Central test scaffolds

Scaffolds contain editable native test entrypoints, separate support files and
source references to published binding requirements. The fixture hook initially
fails with an explicit unreviewed-binding error. Implement it only after connecting
actual production calls/validators/observers through the existing adapter.

The native test invokes `verify bindings test` in a child process, passing the
snapshot, check occurrence and full plan hash embedded by generation. These
arguments are generated; they are not manually maintained acceptance inventories.
The driver reuses managed capture/build/execution, central assertions, complete
mandatory cases and configured limits. Missing observations, stale pins, technical
failures, unsupported inputs and cleanup failures remain FAIL/UNKNOWN.

The driver returns a transient report with `published: false`, actual case
outcomes and incompleteness diagnostics. It does not migrate storage, publish run
history, prune runs or adopt tasks. Native compiler caches may still be populated.
For durable evidence use ordinary `verify run --no-adopt`; generation and scaffold
test success are not application acceptance.

Rust requires explicit `#[cfg(test)] mod contract_plan_tests;` in the authored
adapter; include `wire_codecs` explicitly if its conversion helpers are used.
Existing manifests, lockfiles and module declarations are preserved. Bun uses
`bun test contract-plan.test.ts`; Go uses its normal `go test` invocation. Set
`DCTX0_TEST_CLI` to an upgraded executable when testing an unreleased tool.
During development, use `target/release/dctx0` after an all-features release build;
the installed CLI is not replaced by this workflow.
The configured case/build/run budgets remain authoritative; generated tests do
not expand a consumer's test timeout.

## Fixture isolation

Fixture keys and resource choices remain authored. The tool contains no database,
container or cluster switch. A Go hook can return a child-only environment:

```go
configured, err := ConfiguredFixture("RUNNER_VERIFICATION_DATABASE")
if err != nil { return nil, err }
return map[string]*string{"RUNNER_VERIFICATION_DATABASE": configured}, nil
```

A required unavailable state is explicit:

```go
// Before: the scenario inherited the full gate's configured private database.
// A serial t.Setenv fixed it, but changes the parent process environment.
// After: only this scenario's child sees the empty project-chosen switch.
fixtureScenario(t, map[string]*string{
    "RUNNER_VERIFICATION_DATABASE": UnavailableFixture(),
}, func() {
    // Authored calls, ownership assertions and cleanup use the actual fixture.
})
```

Nil Go environment entries remove a key; a pointer to empty text clears it.
Rust uses `Command::env`/`env_remove`; Bun constructs a separate child environment.
Configured prerequisites require nonempty values. Diagnostics name keys, never
values. Parent environment is unchanged, including when tests run concurrently.
Go child scenarios retain test deadlines and coverage output settings. Cleanup
uses Go's test cleanups, Rust fixture Drop, or Bun finally; cleanup of resources
acquired during a partially failed setup remains the authored hook's responsibility.

## Validation and measurement

```sh
python3 scripts/memory_guard.py python3 scripts/generator_acceptance.py
python3 scripts/validate.py --coverage
```

The focused runner builds the release CLI, executes codec/publication/central
regressions and generated tests in all three languages. An unimplemented hook
fails; an actual independent production function passes; a production mutation
fails central postconditions. It preserves raw logs and speed/RSS samples under
`target/generator-acceptance`, with atomic latest-20 summaries under
`benchmarks/history/generators`. Measurements distinguish compiler/test commands
from release generation commands and retain explicit memory-method limitations.
New workloads remain unassessed against an accepted baseline.

Workspace validation invokes the scaffold portion after its normal native codec
and engine tests. Neither native helper coverage nor reduced handwritten code
implies full application acceptance or complete branch coverage.

The 2026-11-07 isolated workspace coverage run covers 42,031 of 43,291 merged
source lines (97.089%) and 3,410 of 3,512 LLVM functions (97.096%), satisfying the
unchanged 97% gates. Raw LLVM line coverage is separately 44,025 of 45,676
instrumented lines (96.385%); merging follows the existing source-line policy,
not a new exclusion. Branch coverage is unmeasured. Native helper tests execute
real Go, Rust and Bun programs; Rust coverage does not measure those helpers.

The coupled workspace run passes all 34 validation commands and records 88
release workloads. It takes 4,409,832.866 ms; the outer guard samples a
2,376,720,384-byte peak process-tree RSS sum at 50 ms intervals (shared pages
count per process). Performance comparison is unassessed against the existing
baseline; the baseline is unchanged.

After that run, the small authored adapter examples and acceptance runner were
refined to preserve wrong primitive shapes. The final focused run passes 40
measured phases in 42,118.483 ms with 459,476,992 bytes sampled process-tree peak
RSS. It executes generated code in all three targets and rejects an additional
production mutation that incorrectly accepts strings. Rust compilation inputs
for the coverage report are unchanged by this example-only refinement. The full
workspace and final focused run have separate source hashes; see
`docs/generator-validation.json` and its referenced measurement reports.

The minimal production boundaries and authored adapters are checked in under
`fixtures/conformance/generator/{go,rust,typescript}`. For integer validation,
the adapters call the independent `ValidateCount`/`validate_count`/`validateCount`
function. Wrong primitive inputs retain their primitive shape. Foreign options,
variants and failures use an explicitly authored tagged envelope in these
examples, preserving distinctions that nullable options cannot represent.
This is a manual mapping, not a new generated representation or business rule.

Representative final samples (warm tool caches, fresh isolated projects):

| Command phase | Elapsed ms | Peak RSS bytes |
| --- | ---: | ---: |
| Go scaffold generation | 30.549 | 67,268,608 |
| TypeScript scaffold generation | 30.718 | 67,268,608 |
| Rust scaffold generation | 30.981 | 67,268,608 |
| Go generated test with actual production | 1,132.870 | 104,673,280 |
| TypeScript generated test with actual production | 242.584 | 67,268,608 |
| Rust generated test with actual production | 3,466.017 | 217,362,432 |

Individual command RSS uses wait4's child/reaped-descendant high-water mark; it
may include inherited pre-exec memory and is not collective concurrent RSS.
These samples are not an optimization comparison or an application acceptance.

## Isolated runner comparison

The application repository is kept free of task-owned changes at the maintainer's
request. The adoption experiment lives in an isolated source/dependency copy at
`target/runatron-generator-source-copy`; it is not an application migration or a
commit in runatron-business. An earlier attempt copied runtime artifacts and was
abandoned; the successful copy uses Git's source inventory and separate dependency
copies, without hardlinks to application source.

The copied snapshot includes the application's local work at the time of capture.
Its source inventory and origin revision are retained in
`generator-copy-provenance.json` inside that copy. The copied Host bundle is built
from those sources with the application's existing `runner-prepare.ts`.

Eight focused Go tests pass against the real copied production decoder and
private Host fixture. They cover nested Assignment/VersionedReference conversion,
optional AttemptRecord representation, exact production integer parsing, explicit
fixed-width overflow, wrong failure declarations/payloads, the actual returned
upgrade record and actual `Authorization.verify(AssignRuntime)` calls. The two
negative fixture tests pass with a configured database inherited by their parent;
only their children see the explicitly cleared switch. A positive denial test
with a missing configured fixture fails with a prerequisite diagnostic.

The existing handwritten converter remains available and is compared independently
for the selected nested records. Native Host/enum mappings and actual resource
ownership remain authored. The complete central scaffold is retained as an inert,
unimplemented comparison candidate; it is not silently installed in the consumer's
two-second ordinary test lane.

Counting nonblank, non-comment source lines across the three selected existing test
files gives 276 before and 243 after: 33 fewer authored lines. The comparison also
adds 115 authored regression lines and reuses 676 generated codec lines plus 142
generated support lines. This small adoption does not establish a net reduction in
all application code; its reusable failure decoding replaces the repeated block,
while fixture declarations, production calls and ownership assertions remain authored.

The failure helper rejects the contradictory record decoder. The two inherited
environment defects are prevented when the author explicitly declares the fixture
unavailable; generation does not infer that choice from test names or prose.

The copied focused run takes 4,924.688 ms and records 400,863,232 bytes peak RSS
using wait4's child/reaped-descendant high-water mark, which may include inherited
pre-exec memory and is not collective concurrent RSS. It observes the unchanged
per-test two-second deadline. Full application acceptance and coverage were not
claimed or run in the original application repository.

`docs/generator-reference-adoption.patch` retains the authored comparison and an
owned-fixture runner. Apply it only to a source copy. Generate the selected Go
codecs there, obtain `contract_testing_test.go` from a `bindings scaffold` dry-run
proposal, build the copied Host bundle, then run
`bun packages/runner/.dctx0/testing/generator-adoption.ts`. The generated reference
selection is AccessDenied, Assignment, AttemptRecord, RuntimeAction,
InvocationAuthority and VersionedReference below `RunatronBusiness.RunnerRuntime`.
Retained scope, source digests and measurements are in
`docs/generator-reference-validation.json`; these are focused experiment receipts,
not task delivery or application acceptance evidence.
