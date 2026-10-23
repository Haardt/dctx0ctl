## Compact verification, public SQL and smaller SQLite history

Verification commands return compact versioned JSON receipts by default. Consumers
that need the complete previous report must request `--full` or use atomic
`verify export`. Summaries and filtered case lists read metadata; an exact case
query opens only its selected body. Output limits reject oversized responses
without silently truncating evidence.

`query schema`, parameterized read-only `query sql`, and explicit `query prepare`
expose documented public views for tasks, evidence, cases and pinned spec sources.
Views are project-scoped and versioned, with historical plan/event/snapshot pins,
row/byte/time limits and explicit availability. Done, readiness and testing PASS
retain their existing rules.

SQLite schema 7 shares compressed immutable snapshot/task/report objects and exact
report chunks while retaining all task history and historical PASS. Verification
keeps the latest run, latest FAIL/UNKNOWN, pending publications and direct sources
needed by retained inspection/replay workflows. Historical metadata outlives
pruned bodies; missing detail is explicit. Conversion is opt-in through
`storage compact`; `storage install-copy` checks the comparison evidence, verifies
preserved values again and saves a backup before replacement. Older schemas remain
readable, and reading never migrates or repairs the database.

The isolated Company Manager comparison preserved 18,509 payload rows, all task
history and 1,261 historical PASS states. Its database shrank from 4.86 GB to
341 MB (93%). Five paired samples produced identical SQL results and byte-identical
full exports. The measured SQL workloads were 83–98% faster with 72–97% lower peak
RSS; full-export runtime was essentially unchanged. This is historical evidence,
not a new execution claim for the Company Manager application.

Local workspace/feature/native checks passed with 97.17% merged Rust source-line
coverage and 97.01% LLVM function coverage. All 81 coupled release workloads and
separate 10,000-case query workloads were recorded. The local aggregate remains
incomplete because Go Race is unavailable on the ARM64 host's VMA layout. General
baseline comparisons remain predominantly unassessed; the paired comparison's
20% speed/10% RAM gates passed. No thresholds, exclusions or accepted benchmark
baseline were relaxed. Detailed evidence is in
`docs/storage-compaction-validation.json`; those measurements precede this
release-only package version bump.

All three embedded skills share the query guide. Existing JSON exports are left
untouched. Normal tag CI and all five native distribution gates remain required
before publication; no new release exception is introduced.
