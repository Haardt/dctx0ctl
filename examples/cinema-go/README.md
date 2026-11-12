# Cinema in Go

This application implements the same complete Cinema specification as the
TypeScript example. It uses ordinary Go interfaces, `net/http`, and real SQLite
through `database/sql` and the pinned CGO-free `modernc.org/sqlite` driver.
Production code imports neither dctx0 nor generated testing contracts.

Go 1.25 or newer is required. Prepare pinned dependencies explicitly:

```sh
go mod download
```

Run the application:

```sh
go run ./cmd/cinema
```

The defaults are `127.0.0.1:4181` and `cinema.sqlite3`. Override them using
`CINEMA_ADDRESS` and `CINEMA_DATABASE`. The database is owned by the application.
HTTP routes are explicitly implemented in `httpapi`, independently of the spec.

Verification uses `.dctx0/testing/go/adapter.go`, with fresh SQLite sessions,
actual HTTP requests, declared collaborator replacements, and real state/event
observations. The adapter translates values using generated type metadata; it
contains no expected business results. Exact integers use `math/big.Int`.

```sh
dctx0 check dctx0
dctx0 verify init dctx0 --project . --target go
dctx0 verify discover dctx0 --project . --target go
dctx0 verify run dctx0 --project . --no-adopt
dctx0 verify complete dctx0 --project .
```

`complete` also requires current source-bound authored inspection notes for the
manual requirements. Native failures cannot be approved away. PASS applies to
the displayed finite plan; it does not establish correctness for every input.
The Go build and worker run in managed Linux processes. The optional collective
systemd RAM cap is disabled with `DCTX0_MEMORY_GUARD=0`.

Local checks are `go test ./...`, `go vet ./...`, and `go test -race ./...`.
The race detector needs its normal native toolchain; the verification worker
itself is compiled with CGO disabled. Build and execution costs are reported
separately in final release measurements.

## Additional Gin HTTP variant

The existing HTTP implementation remains the default. The additional Gin router reuses this application and its unchanged Cinema spec. See [framework setup](../../docs/site/src/docs/http-frameworks.md) for production selection, adapter configuration, optional profiles, tests and limits.


The application-test suite lives in `contracttests/`. Its local testing-only
module is pinned to 0.2.0/application protocol 2. `scripts/generate_application_facade.py`
derives direct typed calls from a centrally prepared bundle; authored fixtures
provide SQLite, HTTP and actual observations. Production imports no test library.
From the repository root, run:

```sh
python3 scripts/application_test_acceptance.py --form all
python3 scripts/application_test_managed_acceptance.py --target go --replay --report target/cinema-go-managed/result.json
```

The second command builds from captured sources, collects native test results
independently and verifies original, mutated and repaired native replay. Reports
include time/process-tree RSS. Diagnostic and replay reports alone grant no
task-adoption authority. The complete
release gate remains `python3 scripts/validate.py --coverage`.


For a fresh managed application-test acceptance, run from this directory:

```sh
dctx0 verify application-tests library --project . --target go
dctx0 verify application-tests prepare dctx0 --project . --plan .dctx0/application-tests/plan.json --group cinema-native --test-name TestCinemaSuite --form all --boundary json-decoder --group-seconds 300
dctx0 verify application-tests complete dctx0 --project . --plan .dctx0/application-tests/plan.json --group cinema-native --recipe .dctx0/application-tests/native-run.json --report .dctx0ctl/application-tests/receipt.json
```

`complete` uses the prepared check occurrence, current editable profiles and
source-bound authored notes in `.dctx0/testing/go/inspection-notes.json` (or
`--notes PATH`). It executes the native suite and delivers eligible tasks through
the store's transactional guards. A confirmed fresh Go run covers all 5,766 cases
and leaves all 105 tasks Done/PASS, including four authored review attestations.
An explicit native-backed attestation supplies that testing evidence; review
approval or manual completion alone does not. Existing Done/Unverified reviews
are preserved. Final workspace coverage and release validation remain pending.

`verify application-tests run` stores fresh evidence and attempts guarded task
delivery by default. Add `--no-adopt` for diagnostics without task delivery.
`prepare`, `serve`, imported reports and replay cannot independently authorize
adoption. The JSON report exposes `evidence`, `adopted_tasks`, `adoption_issues`
and `adoption_proposal`; `adoption_authority` is true only after actual task
delivery. See the [workflow](../../docs/site/src/docs/application-tests.md).
