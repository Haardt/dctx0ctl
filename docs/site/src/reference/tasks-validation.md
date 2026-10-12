# Task implementation validation

Validated on 2026-10-09 with `python3 scripts/validate.py --coverage`.
The complete run passed **104 Rust tests and 8 Python tests**, rustfmt, Clippy with warnings denied, and the strict coverage gate. Both unchanged mandatory workspaces compile and produce target-neutral task plans. Existing parser/semantic reference fixtures and complete snapshot hashes remain acceptance gates.

The latest task display makes `show` and `next` self-contained: specific instructions and acceptance, exact entity/message/event/operation declarations, required types and compact prerequisite artifact/result references. Duplicate own types and initial change blocks are omitted. Regression assertions cover complete specs and typed JSON, recursive groups, refinements/stableId, source reconstruction, paginated continuation, old-layout cursor rejection and unchanged event sequence. Saved Cinema tasks T42 (MemberId) and T12 (Comment entity) display correctly without replanning.

## Behavior and coverage

Tests cover type closures/refinements, intents/rules/examples/reviews, recursive contracts, missing producers and hard cycles, deterministic waves, filtered/pinned queries, paginated context, concurrent claims, guarded transitions, atomic slice reservations, plan-change invalidation, and explicit check/review follow-up. Identity cases include renames, A→B→A, deletion/name reuse between plans, SCC splits/merges and renewed integration when producer identity changes despite identical contract text. Injected SQLite failures verify full rollback of publication, remediation, work, results, evidence and reservations; schema-1 reads and additive schema-2 migration preserve old history.

Coverage: **8,591/8,591 merged production source lines and 806/806 LLVM functions**. Raw LLVM instances report 9,073/9,092 lines (99.79%) and 96.91% regions; stable instrumentation supplies no branch counters. No production files or ordinary error paths are excluded. See [coverage measurement](coverage.md); machine evidence is exported under `target/coverage/` and uploaded by CI.

## Release performance

20261009T181143.296099Z-266218.json (historical reference removed during repository cleanup) records all 34 workloads, raw samples, hardware/toolchain, input hashes and source provenance. Settings: one warm-up, five fresh-process samples, 20 parser iterations and three iterations per other phase. Speed is median milliseconds per iteration; RAM is maximum post-exec peak RSS in MiB and includes prepared inputs/IR/SQLite. It is not incremental allocation. Task publication measures actual new plans; protocol 2 prevents comparison with earlier no-op samples.

| Workload | Cinema ms | Cinema MiB | runatron-next ms | runatron-next MiB |
| --- | ---: | ---: | ---: | ---: |
| Parse | 0.830 | 3.97 | 18.971 | 9.38 |
| Compile | 3.261 | 6.66 | 231.005 | 55.86 |
| Graph | 0.865 | 6.73 | 18.373 | 63.94 |
| Snapshot | 6.879 | 9.73 | 244.440 | 140.26 |
| Unchanged diff | 0.200 | 8.34 | 5.421 | 94.96 |
| Changed diff | 1.003 | 10.37 | 19.353 | 157.05 |
| Persistence | 13.060 | 18.40 | 250.014 | 278.14 |
| Unchanged check | 21.059 | 20.54 | 730.210 | 339.61 |
| Changed check | 18.587 | 24.40 | 760.571 | 405.88 |
| Task candidates | 6.110 | 16.95 | 313.130 | 285.84 |
| Task reconciliation | 1.054 | 16.97 | 179.402 | 284.94 |
| Task publication | 20.277 | 28.24 | 821.568 | 514.09 |
| Task list | 0.241 | 16.98 | 7.635 | 285.75 |
| Markdown context | 2.413 | 16.99 | 104.077 | 285.19 |
| JSON context | 2.674 | 17.01 | 97.610 | 286.26 |
| Task slice | 0.308 | 17.00 | 12.651 | 284.78 |
| Task claim/release | 5.188 | 17.83 | 144.658 | 285.60 |

The original performance comparison to the previous baseline failed on parser speed; that finding remains in the recorded run. After successful functional validation and complete speed/RAM measurement, the user explicitly authorized the new baseline. `python3 scripts/validate.py --accept-baseline 20261009T181143.296099Z-266218.json` promoted this complete run into [baseline.json](https://github.com/Haardt/dctx0ctl/blob/main/benchmarks/baseline.json). All 34 workloads now have accepted reference values for future comparisons; no tolerances were relaxed. The history still retains the latest 20 runs.

Source content hash: `681697167832dd1397dfe783e9f7201b8f744de310f7a406b40f77a581a5955a`. Toolchain: `rustc 1.97.1 (8bab26f4f 2026-07-14)`. Record timestamp: `2026-10-09T18:08:01.333208+00:00`. Full task commands and formats are documented in [tasks.md](tasks.md). Interpreter, target/native execution and agent execution remain deferred; imported reports do not execute or validate current files.

## Editable profile increment (2026-10-09)

The subsequent profile increment passes 130 workspace Rust tests and 8 Python
tests, plus CLI tests without default features and with only the Rust target.
It preserves both complete reference snapshot hashes. Coverage is 10,567 merged
source lines and 910 LLVM functions, both 100%; raw metrics remain distinct.
Tests cover all four provider contracts, repeatable creation, exact AGENTS text
preservation, dry-run, malformed manifests and paths, required/optional/empty
profiles, semantic roles and real call boundaries, complete example selection,
context budget/cursor guards, immutable claims and reservations, release/retry,
historical texts after file deletion, concurrent claims, selected adaptations,
transitive integration/review effects, stale previews, idempotence, replanning,
legacy adoption, schema-1/2 readers and writer migration 3. Injected table/column
write failures and missing snapshots verify rollback and explicit errors.

See [editable profiles](profiles.md) for the implemented commands and compatibility
changes. Release measurements now include 74 workloads. The accepted 34-workload
baseline remains unchanged; new profile workloads are unassessed until explicitly
accepted.

The final paired run used
`PATH=/home/held/.cargo/bin:$PATH python3 scripts/validate.py --coverage`.
Full measurement (historical reference removed during repository cleanup)
records all 74 workloads, successful functional validation and matching source
hashes before/after measurement
(`3363d79b7a39b9e3897f51fa65aa3c1db6f87efc6c94e0c027f7dc777956ff5c`).
The overall command exits 1 because the performance gate fails: 16 of the 34
comparable workloads exceed the unchanged 20% runtime tolerance. All comparable
RAM measurements satisfy the 10% tolerance; the largest RAM increase is 5.75%.
The 40 new profile workloads have measurements but no accepted baseline.

| Largest runtime regressions | Current ms | Absolute change ms | Change |
|---|---:|---:|---:|
| Cinema task reconciliation | 1.690 | +0.636 | +60.37% |
| runatron-next unchanged diff | 8.637 | +3.215 | +59.31% |
| runatron-next compilation | 333.212 | +102.208 | +44.24% |

These results remain an unresolved performance limitation; passing functional
checks does not imply passing the performance gate. The report retains raw
samples, peak RSS, per-workload comparisons and recent trends. No baseline
promotion or tolerance change was made. The accepted baseline SHA-256 remains
`85c6efc7052fd3bef04e582a828b417c3488c86841e7a64b2f638ef236a09444`.
History contains the latest 20 runs, including failed/incomplete measurements.
