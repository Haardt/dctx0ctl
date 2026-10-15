# Native verification portability — acceptance pending

The executor uses the same protocol, size budgets and finite-plan assessment on
Linux, macOS and Windows. Auto retains Linux process-tree supervision; other
hosts use direct-child kill/wait and bounded stream draining. Rust callers can
select `NativeSupervision::Child` to exercise that backend on Linux. A stream
that remains open after cleanup produces incomplete execution, with bounded
reader cancellation. A missing mandatory reply or native result still prevents
PASS. Optional process-tree supervision and unavailable worker RSS do not.

`verify run --json` adds optional `supervision` capability metadata; saved runs
retain `application/vnd.dctx0.native-supervision+json`. Successful audits also
report capabilities. No SQLite migration is involved. Replay keeps its separate
run identity and never adopts tasks. Review completion remains distinct from
native testing PASS.

The JSON additions are optional on read. Rust callers constructing
`NativeLimits` or `NativeVerificationReport` with exhaustive struct literals must
provide the new `supervision` member (or use the limits' default update syntax).
This is an intentional source API change; existing stored reports and identities
remain readable.

Runtime identity schema 1 remains readable and identifies Linux procfs live-image
observations. Schema 2 explicitly identifies `captured-launch`: the resolved
program path is launched directly, and its bytes, arguments, canonical working
directory and effective inherited/overridden environment digest are retained.
Environment names/values are not written to evidence. Digests use lossless native
OsStr bytes, sorted NUL-delimited entries; Windows override names are compared
without ASCII case. Ephemeral supervisor ownership tokens are excluded. The
executable and parent environment are checked again before publication, alongside
existing captured sources, compiler inputs and Go/Rust build-receipt guards.
This method is a launch receipt, not a kernel-observed executable image.

Go uses host executable suffixes and tool paths; compiler peak RSS uses Unix
Rusage when available (Linux KiB, macOS bytes), otherwise null. Rust captures host
Rust tools and standard libraries, GNU/Linux headers, macOS Xcode SDK/compiler
headers or MSVC binaries/INCLUDE roots. Windows needs the Visual Studio developer
environment; native CI initializes it. Dependencies must be prepared before
locked/offline builds. Windows checkouts retain LF bytes for authored reviews.

Run on each native host after building an uninstrumented all-feature release CLI:

```sh
python scripts/native_portability.py --binary target/release/dctx0 --coverage
# Windows: --binary target/release/dctx0.exe
```

The runner requires Bun, Go, Rust, Python and the host C toolchain. It passes an
absolute Python interpreter to portable process fixtures; production has no
Python dependency. `--transport-only` explicitly records partial development
validation. For direct Cargo process tests, set `DCTX0_TEST_PYTHON` to an absolute
interpreter path; the project-owned validation runners supply it automatically.
Each validation run is still paired with release measurements. Each run retains
logs, raw samples,
toolchains, platform, binary/source hashes and revision. Native build audits and
worker metrics are retained separately in Cinema reports. Peak memory uses the
portable distribution runner's named wait4/PeakWorkingSetSize methods; null means
unavailable. These are not collective simultaneous RSS or Rust-only allocation
measurements. There is no accepted portability baseline. Existing baselines are
unchanged. History uses atomic writes/locking and keeps the latest 20 runs.

The five existing native release jobs now run on relevant PRs and workflow
dispatch, and gate new releases. Their final check requires one complete report
from each named target. Failed logs and native platform coverage are archived.
Native coverage is scoped to portable transport and compiled-target tests; Linux
workspace coverage separately retains the approved 97% source-line/function gate
via `PATH=/home/held/.cargo/bin:$PATH python3 scripts/validate.py --coverage`.
Cross-compilation or Linux coverage does not establish macOS/Windows acceptance.

Cinema completion reuses source-validated authored inspections and persisted task
reviews: 105 tasks, four review tasks Done/Unverified, 5,674 mandatory native cases
per target. Older scripts that required every task PASS now use that workflow.
No new architectural approvals are invented. Historical validation remains
historical; final native matrix evidence is still required before support claims
or release notes are broadened.

## Local checkpoint (2026-10-15)

The complete Linux native runner passed all 20 stages in
`benchmarks/history/native-portability/20261015T160818.585550Z-205801.json`,
including all three applications, real Rust application tests, failure/repaired
replay and task/review guards. Captured inputs were unchanged during that run.
The subsequent change only restricts a process-group cleanup test to Linux;
the current native matrix must accept that exact revision separately.

This checkpoint's uninstrumented release samples were 31.44 ms for specification
check, 21.33 ms for generation, 223.57 ms for first execution and 213.49 ms for
warm execution. Each reported wait4 peak was 58,425,344 bytes, with the scope
limitations above. They are individual development samples, with no accepted
portability baseline or cross-machine comparison.

The earlier complete workspace coverage instrumented run passed 32,256/33,041
merged source lines (97.624%) and 2,629/2,710 LLVM functions (97.011%). Its enclosing
validation/measurement record remains incomplete because earlier test failures
and source changes occurred in that long-running invocation. These coverage
figures precede the final test-fixture changes and do not certify the final tree.
The fresh full Linux CI validation is still required. No baseline was promoted.

The first five-host matrix found a real Rust Cinema HTTP transport issue on
macOS Intel and Windows. Accepted sockets now explicitly select blocking mode
before bounded reads. See the separate [authored delta inspection](native-portability-rust-http-review.md).
The earlier Linux x64/ARM64 and macOS ARM64 successes are retained historical
evidence; they are not combined with later jobs to claim a successful matrix.


## Explicit adapter cleanup follow-up

Resource inspection found a Rust worker gap: terminal protocol/read/write errors
could bypass the active session's `close()` hook. Adapter errors/panics could
also retain that session until process exit. The worker now attempts close on
all Result exits and before reporting adapter errors, takes the session before
cleanup, catches cleanup panic and finishes the context. Existing errors remain
errors; failed cleanup cannot emit a successful shutdown reply.

Nine real compiled-worker probes verify exactly-once close hooks, including
EOF, malformed protocol, repeated IDs, normal end/shutdown, adapter error/panic
followed by a new session and cleanup error/panic. A marker proves the hook ran;
OS handle release at exit alone would not satisfy the test. The complete Rust
mutation/replay regression, Clippy and coupled release measurements pass locally.
See `docs/native-portability-rust-cleanup-validation.json`. This artifact records
earlier fixture/measurement mistakes as well as the corrected results. The
shared native test stage executes these probes on each host; a new five-host
matrix is required for this helper change. Existing Cinema source reviews are
unchanged because application source did not change.


Delayed-close probes also reproduced a Bun/Go reporting race: a technical frame
could reach the executor before the adapter finished releasing its resources.
Both helpers now complete session cleanup before fault publication. Protocol
fault cleanup runs in the same serialized execution path; pending collaborator
calls are released before it. Go catches cleanup panic. The delayed probes fail
before these changes and pass afterward, alongside existing transport tests.
Bun/Go cleanup probes are mandatory in each native host job, and Bun helper
coverage/packet reports are archived. See
`docs/native-portability-sdk-cleanup-validation.json`. These helper deltas require
a new complete matrix; results from preceding revisions remain historical.
