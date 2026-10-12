# Bun transport and target SDK

`dctx0-target-sdk` now supplies the separate version-1 execution transport contract.
`TargetProfileProvider` and the editable profile manifests are unchanged. The
TypeScript target's `BunHarness` produces testing-only `contract.ts` and `worker.ts`,
an explicit Bun recipe and a nonfunctional adapter scaffold. Targets return data;
they do not write files or spawn processes.

## Adapter boundary

The developer adapter exports `adapter: ProjectAdapter` with `describe` and
`begin`. Its description names actual entry points, source artifacts, transport
capabilities and representation choices. A fresh testing session supplies actual
`invoke`, optional production factory/decoder `validate`, optional state `observe`
and mandatory `close` methods. Production code imports none of the generated
contract, worker or specification files.

`begin` receives a testing context with `collaborator` and `emit`. Collaborator
calls produce a `mock-call` message and wait for the external controller's
`mock-reply`. The worker does not compare arguments with expected calls, select
mock answers, interpret expressions or decide PASS/FAIL/UNKNOWN. Emitted events
come from the application's actual testing observation access. State responses
must come from the adapter's actual observer.

The independent counter fixture has its own application class and decoder; its
adapter constructs that application and translates values. The native conformance
test also copies and invokes Cinema's real `candidateOrderWinner`, `filmId` and
their existing type dependency. It does not reconstruct expected example answers.
This transport fixture is not either of the still-required full direct/Hono/Awilix
adaptation acceptance applications.

## Protocol

Requests have `protocol_version`, a positive safe integer `id` and a tagged
`request`. Replies, collaborator requests and technical errors are distinct
message variants. Describe, begin, invoke, validate, observe, end, shutdown and
resource-metrics requests are supported. Mock replies are responses to outstanding
collaborator IDs, not new verification results.

Integers/decimals cross the boundary as exact text in the core's tagged values.
The adapter must state any native representation limits. The worker transports
these values and never silently converts them into JavaScript numbers. Actual
value semantics and operation contract checking remain the Rust interpreter's
responsibility. Invocation rejections currently become thrown observations;
adapter setup/observation/protocol and cleanup failures become technical errors.
Separating translation failures inside `invoke` from actual application exceptions
is still required before native verification/adoption. Neither kind is a worker
PASS flag.

The SDK checks version, framing size, safe IDs and the complete deserialized
shape. Comparing that shape with the original JSON catches fields Serde would
otherwise ignore on unit variants, including injected success flags. Nullable
wire fields are explicit. Capabilities preserve their declared array order;
ordinary record/map keys remain data. The JSON payload limit is 1 MiB, excluding
the final newline. The worker checks payload size at its line parser and output
boundary. **Its stream assembler, elapsed-time, log-output and process-group
limits are not yet the required managed engine executor.** Native environment
and cleanup cannot be considered completely verified by this transport alone.

Application console output goes to stderr; stdout contains JSON-Lines messages.
Arbitrary direct application writes to stdout can still break the protocol and
must produce an incomplete/unknown controller outcome. Separate processes are
not a security sandbox.

## Executable evidence and measurements

The Rust target conformance test generates the actual worker and runs four Bun
tests. They cover actual calls, events/state, a collaborator round-trip, fresh
sessions, initialization and cleanup failures, scaffolds, missing sessions,
unselected subjects, corrupt/versioned frames, extra success flags, invalid mock
replies and EOF while awaiting a collaborator. Actual positive worker packets are
also decoded by the Rust SDK, keeping the two wire definitions in conformance.

The native report records initialization, operation, mock-transport and Cinema
pure-call timings separately from Rust artifact-generation measurements. Native
RSS is the worker's live post-exec Linux `VmHWM`; it includes application/session
preparation. Timing includes transport and the test controller, rather than only
the inner business function. These are single conformance samples, not a native
performance baseline or complete Cinema execution costs.

```sh
PATH=/home/held/.cargo/bin:$PATH python3 scripts/validate.py \
  --focus native-transport --samples 1 --iterations 1
PATH=/home/held/.cargo/bin:$PATH python3 scripts/validate.py --coverage
```

The first command is explicitly partial development validation. Both commands
pair tests with release speed/RSS workloads, and only fresh successful native
reports enter that run. Workspace focus remains the full acceptance path. Two
new `verification-harness` workloads bring the release runner to 84; their
baselines remain unassessed. Embedded TypeScript files participate in release
source provenance. Native helper line/branch coverage is still outstanding and
is not measured by Rust LLVM coverage.

The full coupled checkpoint passes 177 Rust tests, 8 Python tests and four Bun
tests. It measures 13,244/13,244 merged Rust source lines and 1,108/1,108 LLVM
functions. All 84 release workloads complete. Raw LLVM totals, earlier failed or
incomplete runs, actual protocol samples and separate native timing/RSS samples
are retained in checkpoint evidence (historical evidence is available in Git history). Development
settings remain unassessed; the accepted baseline is unchanged.

CI installs the locally exercised Bun 1.4.2 and uploads native conformance reports.
The versioned setup uses the [official setup-bun action](https://github.com/oven-sh/setup-bun).

## Remaining integration

The generated transport is not yet an application verification run. Managed Linux
processes, deadlines/cancellation/log limits, pinned application inventory,
TypeScript/TSX discovery, technical initialization, full CLI run/show/replay,
central scenario/mock/state/lifecycle evaluation, schema-4 immutable history and
guarded automatic adoption remain required. Full Cinema implementation and both
adaptive production fixtures are still missing. See the
[completion checklist](cinema-verification-progress.md).
