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

## Opt-in native case reuse

Native verification can reuse historical passing case units when an explicit
input declaration and the complete execution identity match. Eligible complete
passing incremental evidence can deliver tasks after its lineage and the current
task guards are validated. Reused cases are historical evidence; they do not
establish a fresh external observation. Existing Done/PASS records keep their
original meaning. Without an opt-in declaration, verification follows the
ordinary fresh execution path.

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
binding prevents publication of the reuse result.

The engine hashes the declaration, file contents, revision outputs and query
executable identities before and after execution. It also binds the parent
process environment, declared absent environment names, configured runtime and
verifier identity. Stored receipts contain digests rather than file, revision or
environment values. A revision token is only as complete as the source that
produces it: callers must declare every external input that can affect the
selected units. The executor does not sandbox filesystem, network, clock,
databases or clusters, and matching declarations do not prove isolation. Use
fresh execution when those dependencies cannot be reliably bound.

First record a run with the declaration; then explicitly select its run ID:

```sh
dctx0 verify run SPEC --project APP \
  --reuse-inputs .dctx0/testing/reuse-inputs.json
dctx0 verify run SPEC --project APP \
  --reuse-inputs .dctx0/testing/reuse-inputs.json --reuse-from RUN_ID
```

The same flags apply to `verify application-tests run` and
`verify application-tests hybrid`, alongside their required `--plan`, `--group`,
`--recipe` and unique `--report` arguments. `--reuse-from` requires
`--reuse-inputs`. Add `--no-adopt` to retain diagnostic-only behavior. A run with no parent records the
input receipt while executing normally. The `.incremental` result reports the
parent, reused, executed and required case counts, and adoption authority. A
missing or incompatible parent receipt causes fresh execution; a changed binding
during execution is rejected.

These reuse options apply to the run paths above. Authored inspection and review
completion flows (`verify complete`, `application-tests complete`, and
`application-tests hybrid-complete`) continue to require fresh evidence.

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
