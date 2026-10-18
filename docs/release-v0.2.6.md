## Actual call observation in native examples

Call-count assertions now receive the adapter's actual calls and count the
completed `when` prefix in order. Available empty traces can prove `exactly 0`;
missing observation stays UNKNOWN. Declared application failures retain calls,
repeated matching calls count separately, and mock callbacks do not double count
actual calls. Call observation requires `call-observation`, with `mock-injection`
required only for prepared mock replies. No SDK protocol or SQLite schema change.

The Company Manager outage regression now passes with COMPLETE native provenance.
All 7,315 current cases pass, including all 7,314 original cases; ten manual
constraints remain open. This is targeted scope evidence, not a full consumer
gate PASS.

## Memory-bounded QA and validation

The Linux QA runner now monitors command/descendant RSS at one third of startup
available RAM and cleans detached children on abort or cancellation. It also
works without a memory controller; 50 ms RSS sampling is not a hard kernel
allocation limit. Builds and tests default to one worker.

Local validation passed 25 Python tests, 562 regular Rust executions and 563
instrumented executions, plus complete Go/Rust Cinema mutation/replay acceptance.
Fresh merged source-line coverage is 97.53% and LLVM function coverage is 97.02%;
raw LLVM line coverage is 96.89%, retained separately. The bounded workspace run
peaked at 1.47 GiB within a 4.0 GiB budget. All 73 release workloads completed;
baseline comparison remains unassessed and the baseline was not promoted.
The local aggregate gate failed only because Go Race's ThreadSanitizer requires
a 48-bit VMA while this host provides 39 bits. These local results precede the
package version bump; fresh CI validation and all five native distribution jobs
remain publication gates. See `docs/call-observation-validation.json` for the
exact evidence and limitations.
