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

The existing HTTP implementation remains the default. The additional Gin router reuses this application and its unchanged Cinema spec. See [framework setup](../../docs/site/src/reference/http-frameworks.md) for production selection, adapter configuration, optional profiles, tests and limits.
