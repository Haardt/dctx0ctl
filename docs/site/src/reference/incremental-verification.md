# Incremental verification: implementation boundary

## Repository validation reuse

The repository's `python3 scripts/validate.py --coverage` now has a separate,
working local historical-result cache. A successful complete workspace validation
with coverage and release measurements publishes an immutable SQLite receipt.
An unchanged invocation verifies source/dependency/toolchain/environment/options
identity and evidence hashes, prints `REUSED` with the original timestamp and
paths, then returns without executing the suite or appending measurements.
`--force` bypasses lookup and can publish a fresh successful receipt;
`--no-reuse` disables both lookup and publication. Partial checks, failed gates,
missing evidence, source drift and interrupted runs do not publish receipts.
Baseline promotion never reads or writes this cache.

The cache is `target/validation-cache/receipts.sqlite3`; removing it only forces
validation again. The first complete successful run is still needed. Existing
old reports do not establish a receipt retrospectively. Installed dependency
bytes, linked file contents, Cargo configurations and tool binaries participate
in identity; unsupported linked directories disable reuse rather than silently
omitting inputs. Environment values are hashed rather than persisted.

This cache reuses a dated local validation result. It does not assert fresh
external network/service/system conditions or new performance samples; use
`--force` when those conditions need to be tested again, or apply
`--reuse-max-age-seconds`. It does not deliver Runner tasks or authorize reuse
of individual native application cases.

## Automatic native receipt capture and historical reuse

Native run and complete commands now default to `--reuse=auto`. They select the
latest same-scope native observation automatically, including FAIL/UNKNOWN or
unbound observations; they do not search backwards for a green result. An
explicit `--reuse-from RUN_ID` restricts the basis. `--reuse=off` forces fresh
selected cases and conflicts with `--reuse-from`. `--reuse=require` refuses
production case execution when an existing basis supplies no compatible unit.
A missing basis still permits establishing a fresh one. `--no-adopt` independently
prevents task delivery.

Every managed run captures available receipts even without a reuse flag. The
standard additional declaration is `.dctx0/testing/reuse-inputs.json`;
`--reuse-inputs PATH` selects another one. Without a declaration, the automatic
receipt binds the environment/runtime but asserts no deterministic units. Those
units execute freshly. This conservative default does not invent isolation.

New input receipts use version 2, independently of the version-1 reuse proof,
SDK, transport and store contracts. An `incomplete` list records missing runtime,
snapshot or live-revision observations by code and input name. Such receipts
cannot authorize reuse or task publication. Old complete version-1 receipts
remain readable. Invalid declarations still fail before production execution.
Input drift after execution retains the failed/unknown attempt and its original
receipt; copied historical observations are removed from that attempt rather
than being published under changed inputs.

When compatibility selection is reached, units receive machine-readable decisions and input-name reasons in
`incremental.decisions`, retained in the version-2 incremental summary artifact.
Digests and secret values are omitted from reasons. The publication transaction
also rejects an old green basis contradicted by later FAIL/UNKNOWN case metadata
in the same bound execution cohort. Current task/profile/publication guards
remain mandatory.

### Explicit controlled environments

Existing configurations keep inherited execution. A project can instead choose
an explicit child environment in `.dctx0/testing/verification.json`:

```json
"environment": {
  "mode": "controlled",
  "inherit": ["PATH", "HOME", "TMPDIR"],
  "values": {"FIXTURE_MODE": "isolated"}
}
```

Builds, native tests, compiler inspection and live revision commands use this
selection. Program discovery uses its selected PATH. Fixed values override
forwarding, absent forwarded names stay absent, and unselected variables such as
`CODEX_TOOL_CALL_ID` never reach the child. Receipts contain names and digests,
not environment values. Runtime identity version 3 records which parent names
must still be current; historical runtime identity versions 1 and 2 remain
readable. No SDK, transport or SQLite version is changed by this addition.
Do not put secrets in command arguments, which remain ordinary recipe evidence.

### Retention and status checkpoint

Schema-7 retention keeps a complete native case baseline for each target/scope,
ordered by widest inventory and then latest observation. A later incomplete or
narrow attempt does not remove that baseline. `status` exposes it separately as
`historical_complete_baseline`; this is historical coverage, not live validity or
current total PASS. Existing historical results and task events are unchanged.
The additive `verification_case_body_users(body_hash,run_id)` physical index
protects retention's inner lookup from full index scans; readers do not create it.

### Remaining implementation boundary

This checkpoint still uses whole-program and whole-capture compatibility.
Compiler/build/catalog/native assertion processes may still run in a warm check;
it only skips eligible production cases. Per-unit dependency receipts, indexed
multi-origin lookup, true independent-spec invalidation, output/input inventory
separation, migration of old missing receipts, durable in-use references and
shared live/static incremental planning remain open. Therefore this is **not**
completion of the durable incremental plan or Runatron acceptance.
The dated [implementation checkpoint](../../../durable-incremental-progress.md)
and [validation record](../../../durable-incremental-validation.json) separate
the failed/interrupted broad attempt from the successful final focused checks.

## Additional deterministic input declarations

Native verification can reuse historical passing case units when an explicit
input declaration and the complete execution identity match. Eligible complete
passing incremental evidence can deliver tasks after its lineage and the current
task guards are validated. Reused cases are historical evidence; they do not
establish a fresh external observation. Existing Done/PASS records keep their
original meaning. Without a deterministic declaration, verification records available inputs but executes cases freshly.

Create a JSON file under the application root, for example
`.dctx0/testing/reuse-inputs.json`:

```json
{
  "schema_version": 1,
  "deterministic_units": ["contract:W.A", "other-cases"],
  "files": ["fixtures/snapshot.json"],
  "environment": ["FIXTURE_MODE"],
  "revisions": {
    "database": {
      "executable": "/usr/local/bin/read-fixture-revision",
      "args": [],
      "timeout_ms": 5000
    }
  }
}
```

Use actual qualified contract names and paths. `files`, `environment`, and
`revisions` may be omitted when not needed; the deterministic unit list is
required. The optional `other-cases` unit groups every non-contract case and
requires at least one declared file snapshot or live revision query. File
entries are regular snapshot files, not directories. Revision
commands must return a nonempty revision, exit successfully, and finish within
the configured timeout (default 5 seconds, maximum 30 seconds). They should be
read-only and return an identity that changes whenever the fixture state relevant
to the declared units changes. An unavailable, failed, timed-out, or changed
binding prevents publication of a passing reuse result. Available native
observations remain recorded with explicit incomplete provenance.

The engine hashes the declaration, file contents, revision outputs and query
executable identities before and after execution. It also binds the parent
process environment, declared absent environment names, configured runtime and
verifier identity. Stored receipts contain digests rather than file, revision or
environment values. A revision token is only as complete as the source that
produces it: callers must declare every external input that can affect the
selected units. The executor does not sandbox filesystem, network, clock,
databases or clusters, and matching declarations do not prove isolation. Use
fresh execution when those dependencies cannot be reliably bound.

An explicit basis remains supported; automatic lookup is the default:

```sh
dctx0 verify run SPEC --project APP \
  --reuse-inputs .dctx0/testing/reuse-inputs.json
dctx0 verify run SPEC --project APP \
  --reuse-inputs .dctx0/testing/reuse-inputs.json --reuse-from RUN_ID
```

The same flags apply to `verify application-tests run` and
`verify application-tests hybrid`, alongside their required `--plan`, `--group`,
`--recipe` and unique `--report` arguments. `--reuse-from` uses the standard declaration unless `--reuse-inputs` selects another one. Add `--no-adopt` to retain diagnostic-only behavior. A run with no parent records the
input receipt while executing normally. The `.incremental` result reports the
parent, reused, executed and required case counts, and adoption authority. A
missing or incompatible parent receipt causes fresh execution; a changed binding
during execution is rejected.

The same reuse options apply to `verify complete`, `application-tests complete`
and `application-tests hybrid-complete`. Source-bound inspection and review
requirements retain their existing freshness and task guards.

Rust callers must opt into delivery with the incremental API variants ending in
`_with_adoption`. The original incremental entry points remain diagnostic-only.

The central manifest supplies all case IDs. Explicitly declared contract
acceptance and rejection units can be reused only when every parent case in the
unit passed. All operations, connected scenarios, lifecycle and identity cases
form the single conservative `other-cases` unit; they are reusable only when it
is explicitly selected, has a snapshot or revision binding, and every case in
the entire unit passed. Unselected units execute freshly. A mixed assignment
cannot reuse `other-cases` across different group owners. Reuse requires the same normalized complete program,
requirements, generated inputs and budgets, plus matching full source and
dependency inventories, configuration, harness, runtime, evaluator and generator
identities. This is not function-level dependency analysis. The store rederives
bindings and units from pinned evidence, checks copied records, and preserves the
same-project parent chain. A reused record is identified as historical evidence;
matching inputs are not a fresh execution of that record.

Group completion fails closed: only separately assessed manual/suite obligations
may remain open. Unknown group-error codes, incomplete execution provenance and
native verdicts with no per-case failure assignment block reuse. A later bound
group failure contradicts the whole original cohort, including when an abort
recorded no cases. Explicitly selecting an older run cannot bypass that check.
Decisions include `PARENT_GROUP_INCOMPLETE` with the original issue code.

Application-test reuse has been exercised with actual Go and Bun harnesses,
including an unchanged specification check followed by re-preparation. The full
application bundle and its verified inventory entry are normalized only for the
publication occurrence; assignment, resource budgets, lifecycle inputs and all
other execution inputs remain bound. Native assertions still run. Moving the
bundle or changing meaningful inputs forces fresh execution. In hybrid runs,
adapter contract units may be reused while the application-test group still runs.
An incremental hybrid cannot claim that its entire cohort was freshly executed.
Automatic delivery requires complete passing evidence, validated same-project
lineage and the current task, profile, claim, event and source guards. The store
revalidates each ancestor and copied record in the delivery transaction, up to
256 parent links, and requires a fresh managed native proof at the root. Missing,
pruned, imported, incomplete or failing ancestors cannot authorize delivery.
Native assertions and the application-test group still run; reuse does not turn
an unexecuted or failed native assertion into PASS. `--no-adopt` prevents delivery.

`dctx0 verify incremental-plan SPEC RUN_ID --project APP --json` is a separate
read-only comparison. It does not run tests, publish a check, change task state or
establish PASS. Its comparison alone does not authorize reuse.

## Validation and remaining work

### Exact continuation of a diagnostic run

`dctx0 verify continue SPEC RUN_ID --project APP --plan-only --json` selects
missing and nonpassing case IDs from the immutable original plan. It does not
expand their specification dependency closure into additional executions.
Remove `--plan-only` to execute the selected sessions; `--case ID` selects exact
IDs and `--max-cases N` is a hard ceiling checked before building or execution
(default 256). A selected lifecycle remains its complete path and cleanup.
Completed operations used for central entity-key derivation are referenced as
historical inputs; they are not executed again or called fresh observations.

Continuations can themselves be continued. Their ancestry is bounded to 64
links, validates the original concrete plan and source inventory, and gives
newer observations precedence, including FAIL/UNKNOWN and unexecuted selections
after an abort. `--with-evidence RUN_ID` adds an explicit single-case native
replay to this diagnostic ledger. The replay must have the same original parent,
concrete input projection, source inventory and assessment versions, and no later
contradictory observation. Case ID equality alone is insufficient. A ledger with
zero pending cases starts no worker. A non-planning zero-pending call also checks
the current source/spec hashes. `current_bindings_validated: false` remains
explicit: this does not establish live environment or external-state compatibility.

For expired temporary lifecycle resources, `--lifecycle-fixtures FILE` accepts a
`VerificationLifecycleFixtures` JSON object (`paths`). Only selected argument
values can change; keys, setup and argument counts remain pinned. Production
arguments are still centrally validated and assertions are unchanged. The new
concrete inputs are retained under their own content hash and artifact version.
The application must independently provide and retain the actual owned resources
for the execution lifetime.

This command is diagnostic. Its ledger is not current validated reuse, a complete
native cohort or task-delivery authority. Native observations and historical
failures remain immutable. Every continued run carries
`CONTINUATION_COMPOSITION_REQUIRED`; existing inspection, adoption and attestation
guards still require a separately validated complete provenance chain. No
unbound historical execution environment or external revision is invented.

The coupled runner provides `python3 scripts/validate.py --focus incremental`
for focused checks and release speed/RAM measurements of incremental workloads.
It is partial development validation; full coverage acceptance still requires
workspace focus. Focused measurements do not replace or promote the accepted
workspace baseline.

Focused debug tests cover the pure planner and store proof checks, declared input
capture, the legacy adapter, application harness and hybrid paths. Relevant
entry points include:

```sh
python3 scripts/memory_guard.py cargo test -p dctx0-verification --lib incremental
python3 scripts/memory_guard.py cargo test -p dctx0-store --lib incremental
python3 scripts/memory_guard.py cargo test -p dctx0-engine --test incremental_native
python3 scripts/memory_guard.py cargo test -p dctx0-engine --test hybrid_done application_incremental
python3 scripts/memory_guard.py cargo test -p dctx0-engine --test incremental_hybrid
```

The Go and Bun application reuse tests passed in a combined 16,256 ms with
832,663,552 bytes peak sampled process-tree RSS; these measurements include
the test command and descendants. These are development checks, not complete
release acceptance. Full validation remains
`python3 scripts/validate.py --coverage`, including the release performance
suite and its baseline comparison; the accepted baseline has not been promoted
for this increment. Function-precision invalidation, wider execution reuse and
stronger external isolation remain separate work. Test-analysis predictions
remain nonauthoritative.
