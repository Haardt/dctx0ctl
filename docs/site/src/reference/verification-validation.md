# Verification foundation validation — 2026-10-10

The full requested roadmap is **not complete**. Architecture stage 0 is recorded;
part of stage 1 provides wire values, expression evaluation and explicit finite-plan
assessment. Full IR projection, refinements/type validation, input generation,
shrinking, native TypeScript/Bun harness, adaptive fixtures, verification CLI,
provenance/history/adoption, mocks and lifecycles remain outstanding. See
[the architecture](verification-architecture.md) and
[implemented API limits](verification-foundation.md).

## Checks and coverage

`PATH=/home/held/.cargo/bin:$PATH python3 scripts/validate.py --coverage`
completed the workspace check/build/fmt/Clippy, all-feature tests, separate CLI
feature configurations, doctests, Python tests and coverage stages successfully:
142 all-feature Rust tests, 8 Python tests, 11,329/11,329 merged production source
lines and 955/955 LLVM functions. Both complete example compiler/hash acceptance
cases pass; compiler IR and snapshot fixtures were not refreshed. These results
are not native application acceptance or complete branch coverage. Raw LLVM totals
are retained separately from the merged source-line summary in
validation evidence (historical evidence is available in Git history); full exports remain under
`target/coverage/`.

## Performance

The coupled run `20261009T225759.478233Z-434233.json` recorded all 78 workloads but
returned failure because eleven established timings exceeded the accepted speed
tolerance. Earlier failed attempts were still benchmarking concurrently. That
failed comparison is retained; no baseline or tolerance was changed.

An isolated release repeat `20261009T230546.739968Z-446733.json` used the project-owned
`scripts/validate.py` benchmark/save functions with the unchanged defaults:
20 parser iterations, at most 3 phase iterations, one warm-up and 5 samples;
speed tolerance 20%, RAM tolerance 10%. It reused the full checks only after
matching the exact validated source hash `e46ce4c4faf49be49cbf42cb44d323f1de10db41e8b6ff056b726d14d4e31ec9` before and
after measurements. Its `validation_evidence_run` pins the original full run;
tests were not represented as newly executed during the repeat.

The isolated repeat completed all 78 workloads. All 34 established workloads
passed speed and RAM thresholds against the explicitly accepted
`20261009T181143.296099Z-266218.json` baseline. The performance gate is **partial**:
40 profile workloads and 4 new verification workloads remain unassessed, pending
an explicitly authorized baseline promotion. No performance optimization or claim
that the earlier historical runtime deviation was fixed is made here.

Full raw samples, accepted baseline provenance and absolute/percentage comparisons
for every assessed workload are preserved in
validation evidence (historical evidence is available in Git history). The project runner retains
its last 20 runs and prints recent trends; the concurrent initial attempt should
not serve as an isolated trend baseline.

| Core-only workload | Median elapsed ms | Peak RSS bytes |
| --- | ---: | ---: |
| `cinema/verification-evaluate-unbound` | 0.016 | 7,163,904 |
| `cinema/verification-assess-unbound` | 0.097 | 7,081,984 |
| `runatron-next/verification-evaluate-unbound` | 1.175 | 62,570,496 |
| `runatron-next/verification-assess-unbound` | 4.041 | 64,262,144 |

Timing excludes compilation and synthetic plan preparation; RSS is Linux
post-exec process VmHWM and includes preparation/IR/plan allocations, not
incremental allocation. These new phases do not access SQLite or spawn a native
application worker. They evaluate expressions with empty variables/observations;
many results intentionally stay UNKNOWN. Their measurements make no claim of
application PASS. Native harness/helper coverage and execution costs are not yet
available.

The retained JSON includes both the failed initial comparison and isolated repeat;
benchmark history and the separately accepted baseline keep their normal lifecycle.
