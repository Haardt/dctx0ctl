# Verification history storage

Schema 7 retains the newest published verification run and newest FAIL/UNKNOWN
run with complete case details. Pending publications are protected by their task
IDs; the direct sources of retained inspection/replay workflows are also protected.
The protection does not recursively retain an unlimited chain of ancestors.
Native and imported runs share this policy, regardless of scope. Schemas 5/6
retain their previous three-run policy until explicit conversion.
Saving the same run again does not extend its lifetime or change its publication
order. Different executions have different execution times and identities.

Older runs retain a compact historical outcome, scope, case counts, provenance
digest and existing task/adoption links. Their details are explicitly marked
pruned. A historical PASS describes the original result; it cannot authorize a
new adoption or inspection after its execution evidence has been removed. Replay
requires one of the retained complete runs. Exact retries of an already published
task batch can still read its original receipt; they create no new evidence or
task events. Task states, results and append-only
events are preserved. Task detail queries add `verification_evidence` availability
metadata for referenced local runs; Markdown and the Site identify pruned proof
details beside the historical check. This metadata does not rewrite saved tasks.

## Stored evidence

Schemas 6/7 separate run headers, ordered case metadata, mandatory case inventories
and independently compressed, content-addressed case bodies. Runs derived by
source inspection share unchanged case bodies. Schema 5 whole-corpus gzip storage
remains readable until explicit preparation converts it. Public full-run
JSON remains schema 1; run IDs still hash that original JSON representation.
Artifact hashes and lengths refer to original, uncompressed contents. Artifacts
of at least 4 KiB are compressed when compression saves space.

Complete cases retain the inputs and observations needed for replay and task
checks. Additional session journals and individual operation, example and
lifecycle assessment diagnostics are persisted only for FAIL, UNKNOWN and
incomplete execution. Successful cases are still assessed normally. Plans,
mandatory inventories, provenance and proof-relevant central assessments remain.

Schema 5 introduced retention metadata; schema 6 adds addressable case storage and
public query projections. Schema 7 adds shared snapshot/task/report storage. Read-only access supports schemas 1–7 without migration.
Writers migrate additively; `dctx0 query prepare SPEC` explicitly converts existing
retained bodies and prepares pinned spec projections. The CLI must support the stored schema; schema-6 binaries cannot
open schema 7. Snapshot, compiler-check, rename and task history
are not pruned.

## Inspect and maintain history

```sh
dctx0 verify history path/to/spec --limit 50 --offset 0 --json
dctx0 verify show path/to/spec RUN_ID --json
dctx0 verify gc path/to/spec --dry-run --json
dctx0 verify gc path/to/spec --compact --json
```

History pages contain at most 200 summaries and do not decode case bodies. Rust
callers use `verification_run_summaries` for paginated history; `verification_runs`
returns the retained complete runs, including protected pending publications. Show defaults to a summary; filters select
metadata pages and `--case` opens one body. `--full` preserves the earlier complete
JSON. `verify export SPEC RUN_ID --output report.json` streams an atomic full export.
The Site includes historical summaries and reconstructs complete corpora through
the shared storage abstraction. See [compact reports and SQL](queries.md) for the
intentional JSON compatibility change, preparation and public view contracts.

Normal publication atomically prunes obsolete detail and deletes unreferenced
artifacts. Small runtime-identity and runtime-metrics records remain as measurement
metadata, even after execution bodies expire. Shared execution artifacts remain
while another complete run needs them. SQLite
can reuse freed pages; reducing the physical file requires `gc --compact`.
Compaction builds and checks a separate replacement while preserving the source,
then replaces it atomically under the project lock. Source changes or unavailable
exclusive access prevent replacement. Interrupted temporary builds are restarted
using the maintenance journal.

Maintenance creates **no backup**. Removed detail cannot be recovered after the
replacement. Compaction needs space for the reduced database and its temporary
transaction files, rather than a second copy of the full original database.

To remove duplicate embedded run bodies from explicitly selected JSON exports:

```sh
dctx0 verify gc path/to/spec --compact --exports path/to/.artifacts/verification
```

Recognized embedded reports become references with historical summaries; the
database remains the source of complete retained evidence. Unrecognized files
are preserved and reported. Symlinks are not traversed. Explicitly exporting a
new full JSON report can still create another copy outside the database.

## Validation and measurements

```sh
python3 scripts/storage_acceptance.py
python3 scripts/validate.py --coverage
```

The storage runner couples store, engine and CLI checks with release speed/RSS
measurements for both Cinema specification roots. Its synthetic storage corpus
contains 10,000 cases with 24,000 payload bytes per case. It measures publication,
detail reads and summary reads separately; process VmHWM includes fixture
preparation. Native compiler and harness costs are outside those storage phases.
New workload baselines remain unassessed until explicitly accepted. The latest
20 storage reports are retained under `benchmarks/history/storage/`.

## Company Manager maintenance on 17 October 2026

The existing database shrank from 19,740,737,536 to 1,764,896,768 bytes
(91.06%). All 409 run identities remain: three have complete details and 406
have pruned historical summaries. Referenced artifacts fell from 94,813 to 246.
The measured run/body/artifact payload total fell from 17,164,614,483 to 3,687,162
stored bytes; this subtotal excludes summary metadata, relational indexes and
other history. Physical database size includes all tables and SQLite overhead.

Export-directory contents fell from 3,533,094,515 to 262,330,742 bytes (92.58%).
Fifty-six recognized JSON reports were reduced. Unrecognized JSON and all other
files were retained; one malformed JSON file was reported and preserved.

All 20 non-verification tables, containing 1,139,345 records, have identical
before/after content digests. The latest three run IDs are unchanged. SQLite
integrity and foreign-key checks pass. No backup was created, as requested.
This one-off maintenance took 302,511 ms and peaked at 560,459,776 RSS bytes,
measured by Linux `wait4`; it invoked no native compiler or application harness.
The actual development binary hash, table digests and complete size/measurement
scope are retained in `docs/verification-storage-company-manager.json`.

## Validation results

The final focused storage acceptance report
`benchmarks/history/storage/20261017T085811.566463Z-3987.json` passes four command
stages and all 12 release speed/RSS workloads with unchanged sources. For a
240 MB synthetic case payload, publication takes 2,256–5,241 ms, detail reads
3,008–3,053 ms, and summary queries 0.361–0.381 ms. Peaks are 294–300 MB,
including the fully constructed input fixture. These single-sample development
measurements overlapped other validation; they establish no baseline comparison.

All-feature workspace Rust tests pass, including all 41 native engine tests.
Final formatting, workspace Clippy with warnings denied and all 20 Python tests
pass. Site/browser acceptance passes with unchanged sources in
`benchmarks/history/site/20261017T082659.978646Z-29808.json`. Complete Go and Rust
Cinema release acceptance each pass 5,674 cases, 105 tasks, five actual mutations
and replay; their separate native performance reports each contain 11 phases.

Merged coverage reaches **97.45% source lines and 97.01% functions**. This combines
a complete instrumented workspace execution with supplemental CLI-export and
store-boundary tests. Function identities use the source filename and complete
first-region span, combining template instantiations exactly as LLVM does; the
identity aggregation reproduces the original LLVM totals before merging. The
original raw workspace result (96.76% functions), supplemental raw JSON/LCOV,
content hashes and seven additionally covered functions remain separately
recorded in `docs/verification-storage-coverage.json`. No exclusions were added.
Production Rust remained unchanged between these executions; only tests were
added. This is merged execution evidence, not a claim that the original raw
workspace gate passed. Fresh CI continues to enforce the unchanged 97% gates.

The aggregate validation report remains explicitly failed/incomplete: Go Race
rejects this ARM host's 39-bit VMA layout, and source-change guards detected fixes
made during the run. Its 73 measurements are retained but cannot become a
baseline. Corrected compressed-artifact readers, final boundary tests and native
acceptance were repeated separately. No accepted baseline was changed.

Debug/test builds optimize the external SHA-256 dependency for large compiler
images; first-party debug code remains unoptimized. All performance workloads
use release builds without coverage instrumentation. See
`docs/verification-storage-validation.json` for precise scope, commands, raw
reports and remaining aggregate limitations.

## Minimum retained history (schema 7)

All task plans, definitions, states, events, requirements and pinned snapshots stay
readable, including historical PASS provenance. Large immutable JSON values share
compressed content-addressed objects; long report strings additionally share
exact-byte chunks. This removes repetition without changing public values,
ordering, IDs or artifact hashes. Compact event/evidence metadata and indexed spec
projections let agents list, aggregate and join without loading execution bodies.
Redundant internal graph and dependency copies are reconstructed from their
canonical snapshot/plan when needed. No application-code full-text index is added.

`storage compact` prepares a resumable verified copy; it does not replace the live
file. `storage verify-copy` compares preserved logical rows and validates retained
body hashes. `storage install-copy` accepts a successful comparison for the exact
copy and CLI binary, verifies the source again, and retains a synced backup under
`.dctx0ctl/backups/`. `storage recover` resolves an interrupted installation under
the project lock. These storage commands are separate from `verify gc`, whose
legacy export-reduction/maintenance behavior is described above. Existing JSON
exports are untouched by storage conversion.

## Schema 7 acceptance (2026-10-23)

Company Manager's checked copy was installed with a schema-5 backup after comparing
18,509 preserved payload rows across nine tables. The live database is
340,631,552 bytes instead of 4,863,143,936 bytes (93% smaller). All task and snapshot
history remains, including 1,261 historical PASS states. Five paired samples show
identical SQL results, case details and canonical full exports. SQL workloads are
83–98% faster with 72–97% less peak RSS; full-export time is effectively unchanged.
The accepted baseline and existing JSON exports were not changed.

The 10,000-case synthetic workloads separately measure preparation, summary,
aggregation, filtered cases, one case, source joins and streaming export. Their
169 MB raw corpora occupy about 17 MB each; summaries take 8–9 ms and 9–11 MB RSS.
These compressible imported fixtures are storage tests, not native execution.

Final workspace coverage is 97.17% merged source lines and 97.01% LLVM functions,
with raw LLVM totals retained separately and no new exclusions. Executable native
and feature checks pass. The overall gate remains incomplete because Go Race is
unavailable on this ARM64 host; general baseline comparisons remain predominantly
unassessed. See `docs/storage-compaction-validation.json` in the repository for
exact commands, raw-sample report links, hashes, earlier failed attempts and backup
locations.
