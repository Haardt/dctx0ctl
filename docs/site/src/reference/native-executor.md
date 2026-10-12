# Managed native transport and pure cases

The engine now provides a Linux-only `NativeWorker`. The target SDK still supplies
artifacts and recipes; filesystem/process effects remain in the engine. Replies
are observations, not verification verdicts.

## Process ownership and limits

Every worker gets its own process group. The engine enables Linux child-subreaper
behavior so terminated workers' descendants can be adopted and reaped. This is a
process-wide host setting; embedding callers must account for it. Cleanup waits
only for the owned process group, not arbitrary host children.

Default limits are 30 seconds for describe/initialization, 5 seconds for a case
request and 120 seconds for the native worker's execution lifetime. Cleanup has a
separate 5-second limit. Stderr is capped at 256 KiB and a JSON payload at 1 MiB.
Protocol streams are read in bounded chunks and at most two complete frames are
queued. A partial over-limit frame fails without waiting for its newline.
Canonical SDK schema/version/ID checking happens before a packet can be used.

`NativeCancellation` wakes waiting workers, including cancellation that precedes
the request. Timeout, cancellation, failed callbacks, malformed packets, unexpected
IDs, duplicate collaborator IDs, technical errors and stream limits abort the
worker and reap its managed group. Original failure and cleanup failure are kept
separate. Application logs retain their bounded prefix; overflow prevents success.

Call `finish` after a shutdown reply. The reply alone is insufficient: finish
waits for actual process exit, kills remaining managed descendants, reaps them,
drains streams within a bound and rejects unsolicited messages or late faults.
Exit observation uses Linux `waitid(..., NOWAIT)` before group cleanup, preserving
the leader PID until it can safely be reaped. Repeated abort is idempotent and
does not signal a stale PID. Drop is a last-resort group kill with background
reaping; a verification report must use explicit completion/abort instead.

Process groups are not a security sandbox. Detached descendants, external services,
databases and persistent test resources need explicit ownership/configuration.
Streams held open outside the managed group prevent clean completion. The native
deadline bounds asynchronous transport; a supplied Rust mock callback must itself
be pure and budgeted, rather than blocking indefinitely.

## Pure expression execution

`execute_pure_case` starts a fresh adapter session and drives the interpreter's
ordered invocation continuation against the actual worker. Returned values are
checked centrally against the operation's declared result type. Executed contract
violations and unexpected emitted events from a pure operation are FAIL; missing
capabilities and unsupported semantics remain UNKNOWN. Rust assesses the final
assertion, then closes the session. Native success flags cannot replace this
assessment.

The API evaluates one explicit finite case from supplied IR/plan data. It does not
authenticate snapshots, constitute a whole-plan result or authorize adoption.
The engine's published planning API remains the source of pinned user-facing
requirements. A complete native run still needs inventory/provenance, complete
mandatory coverage and publication guards.

The acceptance test projects the unchanged Cinema spec, runs both modeled winner
assertions and their generated inputs through the actual Bun worker/application
function, and checks a deliberately incorrect isolated application copy. It never
changes the source spec or reference application. This proves the winner invariant
path, not Catalog/Community/VotingStore, examples, mocks or lifecycles.

## Validation

```sh
PATH=/home/held/.cargo/bin:$PATH python3 scripts/validate.py \
  --focus native-executor --samples 1 --iterations 1
PATH=/home/held/.cargo/bin:$PATH python3 scripts/validate.py --coverage
```

The focus mode is partial development validation and still pairs tests with all
release speed/RSS measurements. Fresh native-executor reports retain process costs
separately. Python protocol fixtures exercise process failure/cleanup behavior;
the Cinema property fixture uses real Bun and central Rust evaluation. Native RSS
is live Linux process VmHWM, including setup, and is not incremental allocation.
Single development measurements are not an accepted native performance baseline.

Full Cinema still requires central example/mock/state/lifecycle orchestration,
the complete application, immutable run history, guarded task adoption and the
remaining verification CLI. See [the complete goal checklist](cinema-verification-progress.md).

The retained full checkpoint passes 192 Rust/8 Python/4 Bun tests, 13,979 merged
Rust lines and 1,183 LLVM functions at 100%. All 84 paired release workloads
complete; development settings remain unassessed.
Raw evidence (historical evidence removed during repository cleanup) preserves raw LLVM metrics, native
process samples and earlier failed checks. No baseline or coverage exclusions
were changed. The next runner must distinguish adapter failures from application
observations and enforce a whole-case deadline across requests; existing request
timeouts alone do not establish that stronger guarantee.
