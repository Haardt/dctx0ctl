# Cinema: real persistence, HTTP and lifecycle observations

The Cinema application now supplies real SQLite implementations of all thirteen
declared port operations. `src/app/session.ts` owns a Bun SQLite database and an
Awilix scope; it registers collaborators before resolving the actual application
service. `src/http/app.ts` explicitly wires twelve Hono routes. Production imports
no specification, generated harness, or dctx0 testing module.

The external adapter can select `mock` or `real` mode. Real mode requires an
explicit test database configuration. Each case gets a fresh session and database;
reviewed setup calls establish actual initial state. Optional configured IDs and
time replace only those normal dependency boundaries. Real port calls are recorded
after execution, and entity observations come from SQLite queries. Mocks still
prove consumer behavior only; they do not certify these production ports.

Rust evaluates keyed before/after state, existence and create/update/remove/preserve
predicates, actual call/event counts, and lifecycle source/target states and guards.
Missing observers remain UNKNOWN; an observed empty list means zero observations.
Duplicate complete entity identities prevent a state proof. Consecutive lifecycle
steps must share their delivered boundary state. Independent paths use fresh
sessions. Both Cinema paths have native integration tests, including a mutation
that leaves the poll Open after closing it.

The schema-1 JSON-lines invocation reply additively accepts an optional `calls`
array. Omission retains the old meaning of unobserved calls; an empty array means
observed zero calls. The Rust enum constructor now includes this optional field.
Unknown fields and worker success flags remain rejected. Existing profile
manifests, compiler IR and snapshot goldens are unchanged.

Native build/test commands use a separate bounded command receipt: stdout and
stderr are logs, never verification protocol messages. Nonzero exit is a build
failure. Timeouts, cancellation and output limits use the managed process-group
cleanup path. A successful command exit alone never grants task PASS or adoption.
The command audit's `stderr` buffer contains both output streams. Native compiler
RSS polling is explicitly a sampled lower bound where the final sample is missed;
the Bun test report separately captures the live process high-water mark.

## Validation checkpoint (2026-10-10)

The full coupled development run passes 217 Rust tests, eight Python tests, four
Bun transport tests and 36 independent native Cinema tests. Merged Rust coverage
is 15,332/15,332 lines and LLVM function coverage is 1,298/1,298. The runner records
all 84 release workloads with speed/RSS samples. Reduced development settings
remain unassessed against the accepted baseline, which is unchanged. Native
application coverage separately reports 210/210 lines and 131/131 functions.
Retained evidence (historical evidence removed during repository cleanup) keeps full measurement records,
raw LLVM totals, actual native observations and intermediate failed runs.

## Remaining evidence limits

This increment does not implement the complete verification CLI, immutable run
history, automatic task adoption, shrink or replay. Whole-plan operation clause
assessment and preservation of partial observations after transport failure remain
necessary. Native application coverage is separate from Rust coverage and from
coverage of the generic generated worker and adapter.

The application's current-poll choice is newest Open poll, otherwise newest poll;
the specification does not define this selection policy. SQLite scopes rating
identity by film/member and vote identity by poll/member. The entity declarations
currently name `memberId` as their key while surrounding requirements describe
per-film/per-poll records. This discrepancy remains an explicit open modeling
point, not a silently changed specification or a complete entity-identity proof.
Historical `dctx0.hono.bindings.ts` output is not used as execution evidence; the
new HTTP routes are explicit production application code.

Validation uses `PATH=/home/held/.cargo/bin:$PATH python3 scripts/validate.py --coverage`.
The narrower `--focus verification --samples 1 --iterations 1` development run
includes native tests and paired release measurements, but is neither full
workspace acceptance nor a comparable default-sample baseline assessment. No
baseline is promoted automatically.

## Implemented capabilities

| Capability | TypeScript/Bun | Java, Go, Rust |
| --- | --- | --- |
| Editable implementation profiles | Available | Available |
| Published spec-driven planning | Target-neutral library and `verify plan` | Same target-neutral planner |
| Structural Tree-sitter discovery | Planned | Unsupported execution increment |
| Generated JSON-lines worker | Available, SDK 1 | Unsupported |
| Managed native execution | Linux engine adapter | No executable target recipes |
| Pure Cinema winner property | Both assertions, 101 cases/201 calls | Unsupported |
| Prepared port mocks and emitted events | All 12 Cinema examples | Unsupported |
| Actual port/state observations | Explicit real Cinema adapter | Unsupported |
| Lifecycle paths | Both Cinema paths; fresh SQLite sessions | Unsupported |
| Whole-plan operation pre/postconditions | Pending orchestration | Unsupported |
| Run/show/replay CLI and immutable run history | Pending | Pending |
| Automatic task adoption | Pending; imported PASS is insufficient | Pending |
| Shrink and replay | Pending | Pending |

The application currently uses Hono 4.12.26 and Awilix 13.0.5, installed from the
committed Bun lockfile. Those framework versions describe this application and
its native tests, not a generic framework integration capability. Bun 1.4.2 runs
both the generated worker and the independent application tests. No unsupported
target is presented as having a passing harness.
