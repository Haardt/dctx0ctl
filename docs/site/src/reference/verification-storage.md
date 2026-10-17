# Verification history storage

Each project retains the latest three published verification runs with complete
case details. Native and imported runs share this limit, regardless of scope.
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

The store separates run headers from a content-addressed gzip case corpus. Runs
derived by source inspection can share their unchanged cases. Public full-run
JSON remains schema 1; run IDs still hash that original JSON representation.
Artifact hashes and lengths refer to original, uncompressed contents. Artifacts
of at least 4 KiB are compressed when compression saves space.

Complete cases retain the inputs and observations needed for replay and task
checks. Additional session journals and individual operation, example and
lifecycle assessment diagnostics are persisted only for FAIL, UNKNOWN and
incomplete execution. Successful cases are still assessed normally. Plans,
mandatory inventories, provenance and proof-relevant central assessments remain.

SQLite schema 5 adds the storage representation and retention metadata. Read-only
access supports schemas 1–5 without migration. Writers migrate the schema
additively; existing bodies are converted by explicit maintenance. Older CLI
versions cannot open schema 5. Snapshot, compiler-check, rename and task history
are not pruned.

## Inspect and maintain history

```sh
dctx0 verify history path/to/spec --limit 50 --offset 0 --json
dctx0 verify show path/to/spec RUN_ID --json
dctx0 verify gc path/to/spec --dry-run --json
dctx0 verify gc path/to/spec --compact --json
```

History pages contain at most 200 summaries and do not decode schema-5 case
corpora. Rust callers use `verification_run_summaries` for paginated history;
`verification_runs` now returns at most three complete runs. Show returns the established full report for retained runs and an
explicit summary for pruned runs. The Site includes historical summaries and
processes complete compressed corpora one run at a time.

Normal publication atomically prunes obsolete detail and deletes unreferenced
artifacts. Shared artifacts remain while another complete run needs them. SQLite
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
