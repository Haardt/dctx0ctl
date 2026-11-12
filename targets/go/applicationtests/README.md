# Go application tests (0.2.0)

Testing-only module, application protocol 2. The historical adapter SDK and
WorkerRequest remain separate and supported. Production does not import this
module. Publish the pinned local copy for Cinema with:

```
python3 scripts/publish_go_application_library.py --project examples/cinema-go
```

`Client.DecoderTest` passes raw bytes unchanged to the production decoder before
running a separate result projector. Integer/decimal wire digits remain text.
Decoder callbacks mark actual production parse/contract rejections with `Reject(reason)`. Other returned errors and panics are technical exceptions; projector errors remain binding errors. This prevents an I/O error from satisfying a rejection case.
`OperationTest`, `StateChangeTest` and `AssertionTest` own registration, central
case iteration and finalization. An explicit fixture factory provides one local
production closure, necessary setup/oracle continuations, actual state projection
and cleanup. Pure oracle calls have separate tapes and cannot fulfill observed
production calls. A nil trace means absent instrumentation; an empty slice means
an actual observer recorded zero events/calls/resources.

Call `Finish` and close the client from the native test's cleanup path. The engine
selects every case and latches failures independently of native error handling.
Native test outcomes must also pass; the central assessment alone is diagnostic,
not complete collection, source/build evidence or task adoption authority.

The Cinema pilot runner enumerates the Go parent tests and checks test2json
subcase identities/results against the central cases. Bun has a separate library
with explicit asynchronous contexts. ScenarioTest and LifecycleTest keep one fixture across ordered steps.
ContextFixtureFactory receives an explicit CaseContext with centrally selected
mock subjects and a context-bound Collaborate capability; mock replies are never
passed as invented production calls. IdentityTest derives its verdict from the
actual completed source operations in the same session. Managed collection and
native replay run through the engine; transactional task delivery is a separate
engine/store responsibility. Unsupported continuations remain UNKNOWN. Current native acceptance is Linux;
other platforms require their own real execution/provenance checks.

```
python3 scripts/memory_guard.py go test -C targets/go/applicationtests -coverprofile=../../../target/go-application-library.coverage ./...
python3 scripts/memory_guard.py python3 scripts/application_test_acceptance.py --form decoder
python3 scripts/memory_guard.py python3 scripts/application_test_acceptance.py --form operation
python3 scripts/memory_guard.py python3 scripts/application_test_acceptance.py --form state-change
python3 scripts/memory_guard.py python3 scripts/application_test_acceptance.py --form assertion
```

Reports retain time/RSS and separate helper coverage. These development runs are
unassessed against the accepted benchmark baseline; no script promotes it.

The complete Cinema suite uses generated typed calls for the existing production
API and direct application-owned HTTP handlers. Run it with `--form all`.
The group report alone never authorizes task adoption; release acceptance still
requires the complete workspace validation with coupled time/RSS measurements.


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
