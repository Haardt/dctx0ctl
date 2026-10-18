# Actual call observation in examples

Native example call-count assertions evaluate the actual calls reported by the
adapter. `call-observation` is independent of `mock-injection`: observing calls
requires the former; prepared `given` replies require the latter. An example
without `given` does not require mock injection merely to count calls.

Each assertion counts the ordered prefix of completed `when` steps in its
example, including repeated identical calls. A subsequent empty trace does not
erase earlier calls. Only the resolved operation identity and matching argument
values count. Calls from later steps cannot affect an earlier assertion.

An available empty trace (`Some([])` in Rust, `calls: []` on the wire) can prove
`exactly 0`. A missing trace is not evidence of zero calls: the assertion remains
UNKNOWN with `TRACE_OBSERVER_REQUIRED`. If any earlier step lacks observation,
subsequent observed steps cannot repair that incomplete prefix. Assertions that
do not need calls continue to use their ordinary evidence.

Declared application failures retain their actual call observations. Prepared
mock replies and the central mock callbacks are not added to the actual trace;
otherwise an adapter that records a mock-backed call would count it twice.
Existing unused/unexpected mock checks remain in force.

The Rust `Scenario::deliver_observed(observation, events, calls)` API delivers
actual per-step calls. The existing `deliver(observation, events)` method remains
source-compatible and supplies a missing observer. The accumulator is private;
serialized scenario evidence, SDK protocol and SQLite schema remain unchanged.
The existing native session evidence retains the actual per-invocation traces.
State/resource observation and event assertion semantics are unchanged.

This intentionally corrects projected capability requirements and previously
UNKNOWN example assessments. Historical evidence is not rewritten, and a fresh
execution is required to produce new evidence. Opaque/unresolved trace labels
remain unsupported obligations.

Regression coverage uses a shared small specification, pure scenario tests,
real generated Bun workers, actual collaborator mutations and the complete
verification-run pipeline. Validation and Company Manager acceptance are
recorded in `docs/call-observation-validation.json`; performance measurements
remain separate from behavior claims and never promote a baseline implicitly.
