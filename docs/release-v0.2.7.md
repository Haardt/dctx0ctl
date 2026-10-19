## Shared verification values and recursive corpora

Large string values share immutable storage in Rust without changing their JSON
representation. Rust callers constructing `VerificationValue::String` now supply
`Arc<str>` (for example `text.into()`). Accepted native values reuse the matching
expected value. Message/evidence budgets are configurable, with defaults of
1 MiB/4 MiB and hard ceilings of 64 MiB/256 MiB. Workers support the message
ceiling while project-specific lower limits remain enforced.

The deterministic generator is now `dctx0-values-4`. Productive recursive
constructors are bounded by named-type visits, retaining terminal and nonempty
samples. Bounded selection remains non-exhaustive; unproductive recursion and
exhausted budgets never establish verification PASS.

## Qualified Go race execution

`scripts/go_race.py` first executes a clean race-instrumented program and then
requires detection of a deliberate data race. Only then does it run uncached
Cinema tests with `go test -race -count=1 -mod=readonly -p 1 ./...`.
`target/go-cinema/race.json` retains source hashes, toolchain, outputs and per-stage
elapsed time/peak RSS. The outer memory guard limits aggregate sampled process-tree
RSS to one third of startup available memory.

On this Linux ARM64 machine the clean probe fails before application code:
ThreadSanitizer reports 39-bit VMA, requiring 48 bits. This is a host limitation,
not a detected Cinema data race. It is reported as unavailable (exit 2), making
the aggregate validation incomplete and unsuccessful. There is no plain-test
fallback or release-gate exemption. Fresh release CI must pass the qualified
race run on a supported host and all five native distribution jobs.

See [Go's platform requirements](https://go.dev/src/internal/platform/supported.go)
and [upstream ARM64 VMA discussion](https://github.com/golang/go/issues/29948).
