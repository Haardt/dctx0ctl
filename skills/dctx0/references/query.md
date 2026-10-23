# Query saved evidence

Start with `dctx0 query schema SPEC --json` before writing SQL. Existing history
needs explicit `dctx0 query prepare SPEC`; read commands never migrate or index.
Use filtered task lists or SQL aggregates to discover work. Do not enumerate the
inventory through repeated `tasks show` calls. Fetch individual details afterward.

```sh
dctx0 query sql SPEC --sql 'SELECT task_id,title,ready,check_status FROM dctx0_tasks WHERE work = :work ORDER BY wave,task_id' --param 'work="Open"' --json
dctx0 query sql SPEC --sql 'SELECT subject,outcome,COUNT(*) AS cases FROM dctx0_cases WHERE run_id = :run_id GROUP BY subject,outcome ORDER BY subject,outcome' --param 'run_id="RUN_ID"' --json
dctx0 query sql SPEC --sql 'SELECT t.task_id,s.subject,r.file,r.start_line FROM dctx0_tasks t JOIN dctx0_task_subjects s USING(task_id) JOIN dctx0_source_refs r ON r.subject=s.subject WHERE t.ready=1 ORDER BY t.task_id,s.subject' --json
dctx0 verify show SPEC RUN_ID --outcome FAIL,UNKNOWN --limit 20 --json
dctx0 verify show SPEC RUN_ID --case CASE_ID --json
dctx0 verify export SPEC RUN_ID --output report.json
```

Keep compact command receipts and run IDs. Request `--full` only for a consumer
that needs the legacy complete report; export large cases explicitly. SQL defaults
to 100 rows, 1 MiB and five seconds. `complete:false` means more rows exist; use
an explicit deterministic ORDER BY and WHERE/LIMIT for the next selection.

Task views use the current plan/event pin, or explicit `--plan`/`--as-of`.
Spec views use that plan's snapshot, or the published head; `--snapshot` overrides
it. Run rows retain their own snapshot/check occurrence. Pruned runs retain counts
but have no case details. Historical sources do not establish working-tree
freshness. Captured run-level implementation files do not prove a particular
subject implementation. Missing source positions remain NULL.

Done, testing PASS and readiness are separate. Filtered PASS cases do not change
the full-run result. SQL is read-only and does not claim tasks. Parallel CLI reads
remain allowed; use atomic claims/reservations and coordinate shared file edits.

## Compact storage and retained evidence

Schema 7 shares immutable snapshots, task bodies and exact report strings. SQL
and list queries use metadata; fetch a task or case body only for the current
investigation. `dctx0_task_evidence` exposes requirement/implementation pins,
verifier identity, status and report hashes without loading reports. Join it to
`dctx0_tasks` by `task_id`; PASS remains scoped to its recorded requirements and
implementation, and Done does not imply PASS.

All task plans and event pins remain queryable. Run metadata can outlive execution
bodies: check `dctx0_runs.details_available`, `dctx0_cases.body_available` and
`dctx0_artifacts.available`. The latest run and latest FAIL/UNKNOWN run retain
full details, as do pending task publications and the direct source runs of the
currently retained inspection/replay workflows. `dctx0_run_links` exposes these
source relationships; they grant no adoption authority. Tiny runtime metrics and
runtime-identity artifacts remain available as historical measurement metadata. Older already-pruned runs may lack
case metadata too. `DETAILS_PRUNED` requires a fresh execution or an explicitly
retained export; it does not revoke an already published historical task result.

Use `storage inspect` for sizes. `storage compact` builds a separate verified copy;
it does not install it. Installing a copy requires the successful paired comparison
report through `storage install-copy --copy ... --evidence ...`. Reads never perform
migration or recovery. Existing exported JSON files are outside automatic retention.
