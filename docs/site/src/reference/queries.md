# Compact verification and public SQL (view version 2)

SQLite schema 7 stores ordered case metadata, the complete required-case inventory
and independently compressed, content-addressed case bodies. Identical bodies are
shared across runs. New successful checks project pinned spec text transactionally;
new verification publications project cases in their publication transaction.
Run identities, canonical full report values, artifact hashes, task state and
adoption rules are unchanged. The newest run and newest FAIL/UNKNOWN run retain
complete details, with additional protection for pending task publications and
the immediate source runs of retained inspection/replay workflows.

`verify run`, `complete`, `inspect`, `attest` and `replay --json` return compact
receipts; run summaries identify report version 1.
`verify show SPEC RUN_ID` defaults to a summary. Outcome/subject filters, limit and
cursor select metadata pages; `--case CASE_ID` opens only that case's body.
The full-run result remains visible for every selection. Normal JSON responses
are limited to 1 MiB; `--max-output-bytes` raises the budget up to 64 MiB.
Oversized values are rejected without emitting partial JSON.

```sh
dctx0 verify run spec --project . --json --full
dctx0 verify export spec RUN_ID --output report.json
dctx0 verify export spec RUN_ID --case CASE_ID --output case.json
dctx0 query prepare spec
dctx0 query prepare spec --snapshot SNAPSHOT
dctx0 query schema spec --json
dctx0 query sql spec --sql 'SELECT task_id,title,ready FROM dctx0_tasks WHERE work=:work ORDER BY wave,task_id' --param 'work="Open"' --json
dctx0 query sql spec --file investigation.sql --json
```

This is an intentional default JSON compatibility change. Consumers of complete
observations/provenance must explicitly select `--full` or export saved evidence.
Exports use a synced temporary sibling and atomic replacement; failures preserve
the destination. Existing exported files are never automatically removed.
Existing schemas 1–6 remain readable. Schema 7 conversion uses `storage compact`. `query prepare` explicitly migrates writers,
converts retained runs individually, and indexes the head, current task snapshot
and retained run snapshots. Conversion commits per run and can be resumed.
Missing projections report `QUERY_INDEX_MISSING` and a preparation command.

## Public views

`query schema` is the executable column/type contract. View version 2 provides:

| Views | Scope and relationships |
| --- | --- |
| `dctx0_task_evidence` | Pinned task evidence metadata without full reports |
| `dctx0_tasks`, `dctx0_task_subjects` | Current/selected plan, joined by task_id |
| `dctx0_task_dependencies` | Prerequisite task IDs, condition and reason |
| `dctx0_task_links`, `dctx0_task_events` | Historical links and pinned events; links are not prerequisites |
| `dctx0_checks` | Project check occurrences, snapshots, status and time |
| `dctx0_runs`, `dctx0_run_issues` | Historical outcomes/counts and retained open obligations |
| `dctx0_run_links` | Recorded direct inspection/replay source runs, joined by run_id/parent_run_id |
| `dctx0_cases`, `dctx0_run_requirements` | Retained metadata and complete required-case inventory; join by run_id/case_id |
| `dctx0_artifacts` | Retained run artifact hashes, media types, byte counts and availability |
| `dctx0_subjects`, `dctx0_relations`, `dctx0_facts` | Pinned IR nodes, semantic edges, source excerpts; join snapshot/subject |
| `dctx0_source_refs` | Pinned spec spans and captured implementation file hashes |

`dctx0_facts` exposes the source text of IR nodes with their original kind and
owner; filter kind when looking for constraints, intents or decisions. Spec spans
use one-based lines. Implementation references have a run ID and no inferred
subject or line position. Artifact bytes and application code are not searchable
through these views. Run-level file captures do not prove subject coverage.

```sql
SELECT subject,outcome,COUNT(*) AS cases
FROM dctx0_cases
WHERE run_id=:run_id AND outcome IN ('FAIL','UNKNOWN')
GROUP BY subject,outcome ORDER BY subject,outcome;

SELECT t.task_id,t.title,r.file,r.start_line
FROM dctx0_tasks t JOIN dctx0_task_subjects s USING(task_id)
JOIN dctx0_source_refs r ON r.subject=s.subject
WHERE t.work='Open' AND t.ready=1 ORDER BY t.task_id,r.file;

SELECT subject,text FROM dctx0_facts
WHERE text LIKE :text ORDER BY subject;
```

`--param name=JSON` binds a scalar; duplicate, unused and missing parameters are
errors. `--sql`/`--file` are exclusive, and `--file -` reads stdin. SQL supports
joins, aggregates, NULLs and read-only CTEs. A separate query-only connection with
an authorizer exposes public views; internal tables, writes, ATTACH, PRAGMA and
file/extension functions are rejected. SQL intermediate values and expression
complexity are bounded. The default limits are 100 rows, 1 MiB serialized JSON and
five seconds, configurable with `--max-rows` (10,000), `--max-output-bytes` (64 MiB)
and `--timeout-seconds` (60). Row truncation is explicit as `complete:false`.
Use ORDER BY and WHERE/LIMIT to paginate; there is no generic SQL cursor.

Every response identifies the view version, plan/event/snapshot pins and data
availability. `--plan`, `--as-of` and `--snapshot` select historical pins.
Without explicit snapshot, the task plan's snapshot wins over the published head.
Run rows always preserve their own exact snapshot/check occurrence. Pruned runs
retain counts and available metadata, with explicit body availability and no replay
authority. Older already-pruned runs may have no case rows. Readiness,
delivery and blockers come from the existing task model, projected once per query
under one history read transaction. Reading never migrates or updates task state.

Run summary counts include PASS/FAIL/UNKNOWN, missing cases and open obligations.
For older pruned summaries which never stored an open-obligation count,
`open_obligation_count_available` (SQL: `open_obligations_available`) is false;
the numeric placeholder is not a claim of zero obligations. CTEs cannot shadow
public view names to access private storage; both statement validation and the
SQLite authorizer enforce that boundary.

## Validation and measurements (2026-10-22)

Run the coupled workspace gate with
`python3 scripts/validate.py --coverage --samples 1 --iterations 1 --example cinema`.
It includes `scripts/query_acceptance.py` against the uninstrumented release CLI.
For focused development, `python3 scripts/query_acceptance.py --samples 1` couples
store/CLI tests with the same query measurements. Large synthetic runs use
`python3 scripts/query_acceptance.py --measure-only --binary target/release/dctx0 --cases 10000 --samples 1`;
functional CLI assertions still run. No large fixture is committed.

The final workspace run passes checks, formatting, Clippy, tests, feature matrices
and Cinema native acceptance. Coverage reaches 36,722/37,746 merged source lines
(97.2871%) and 2,986/3,078 raw LLVM functions (97.0110%), with no new exclusions.
All 81 release workloads and 14 query workloads complete on stable source inputs.
Query measurements separately cover preparation, summary, task aggregation,
filtered cases, one case, source joins and full export for Cinema and Cinema-Go.
Records include database/index/body sizes, compression costs and child RSS from
a native wait4 launcher, excluding Python fixture preparation.

Local aggregate validation is incomplete because qualified Go Race cannot run
on this ARM64 host (`RACE_HOST_VMA_UNSUPPORTED`). Measurements use one sample and
remain unassessed against the incompatible baseline. The baseline is unchanged.
See [retained validation evidence](../../../query-validation.json) for exact
commands, raw coverage metrics, workload records and intermediate failed attempts.

The additional stress run passes 55 checks and all 14 measurements with 10,000
cases per workspace (169,060,747 raw corpus bytes). Summary child RSS is about
9.3 MB; full export RSS is about 21 MB while writing 169,062,244 bytes. These
synthetic, highly compressible payloads measure bounded read/export behavior,
not representative compression ratios or a new accepted performance baseline.

## Shared storage (schema 7, view version 2)

Schema 7 removes repeated snapshot/graph and plan dependency copies. Immutable
JSON objects and large exact report strings share content-addressed compressed
storage. Original public snapshot, plan, task and retained run values and hashes
are reconstructed without recompilation. All historical task/event pins remain.

Version 2 adds `dctx0_run_links` (run/source-run relationships), `dctx0_task_evidence` (task ID, requirements hash, implementation,
status, verifier/version, timestamps, report hash, detail availability) and
`dctx0_cases.body_available`. Case metadata, mandatory-case inventories, source
captures and artifact metadata can outlive their bodies. The newest run and newest
FAIL/UNKNOWN run retain bodies; pending task publications are protected by their
actual task IDs. Their direct inspection/replay sources are also protected so
retrying an active workflow remains possible. This is one hop, not unlimited
ancestral retention. Tiny runtime metrics and identity artifacts are retained
as measurement metadata. Missing older bodies produce `DETAILS_PRUNED`. Previously pruned
metadata is not invented, and published task PASS is not revoked by body retention.

Readiness is still computed by the pure task model. Metadata sessions avoid
execution bodies; task context hydrates the selected task and its prerequisite
closure. SQL discovers needed views, prepares typed indexed metadata and applies
its timeout to preparation and execution. No full snapshot is loaded for SQL.

```sh
dctx0 storage inspect spec --json
dctx0 storage compact spec --dry-run --json
dctx0 storage compact spec --output compact.sqlite3 --json
dctx0 storage verify-copy spec --copy compact.sqlite3 --json
```

Compaction uses a consistent backup and resumable transactions, validates every
converted JSON value, checks SQLite integrity and removes unreferenced objects.
It leaves the source unchanged. An interrupted staging copy can be resumed with
the same command while its source signature remains unchanged. A completed or
unrelated destination is never overwritten.

Compare before installing with the release measurement runner:

```sh
python3 scripts/storage_compaction_acceptance.py --spec /path/to/spec \
  --copy /path/to/compact.sqlite3 --old-binary /path/to/previous/dctx0 \
  --new-binary target/release/dctx0 --samples 5
dctx0 storage install-copy spec --copy compact.sqlite3 \
  --evidence benchmarks/history/compaction/COMPARISON.json --json
```

Installation verifies the successful comparison and binds its binary and copy
hashes, rechecks all snapshot/task/evidence data, excludes concurrent store
connections and keeps a synced backup under `.dctx0ctl/backups/`. The returned
report names it. The installation journal is separate from read paths. A failed
or interrupted installation never authorizes a reader to recover files.

Resolve an interrupted installation explicitly with `dctx0 storage recover spec`.
Recovery preserves an existing intact database; if the source is missing, it
restores the recorded backup. Read commands never recover or remove journals.
