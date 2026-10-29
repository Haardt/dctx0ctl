# Incremental project-test research

This opt-in experiment uses ordinary Linux Rust/Cargo libtest projects. It needs
neither a DCTX0 specification nor a target SDK harness. Its reference is the full
set of isolated test executions in a declared scope, not a normal concurrent
Cargo suite. Process isolation does not make inputs hermetic.

```sh
dctx0 experimental coverage catalog ./project --all-features
dctx0 experimental coverage collect ./project --all-features
dctx0 experimental coverage collection ./project COLLECTION_ID

dctx0 experimental test-analysis shadow ./project \
  --base BASE_COMMIT --baseline COLLECTION_ID --strategy coverage

dctx0 experimental test-analysis replay ./project --manifest experiment.json
dctx0 experimental test-analysis replay ./project --manifest experiment.json --resume REPLAY_ID
dctx0 experimental test-analysis show ./project REPLAY_ID
dctx0 experimental test-analysis export ./project REPLAY_ID --output dataset.ndjson
dctx0 experimental test-analysis recompute dataset.ndjson --stage prediction
dctx0 experimental test-analysis recompute dataset.ndjson --stage evaluation
```

Catalog and collect accept `--package`, `--filter`, `--all-features`,
`--no-default-features`, comma-separated `--features` and `--timeout-secs` (300
by default per subprocess). Default Cargo features remain enabled unless
explicitly disabled. The catalog builds and lists tests without running test
bodies. Collection publishes the complete catalog before executing any test and
checks that its inventory still matches. Executable tests run in logical-key
order. The second no-run build reuses the same artifact target directory; both
actual build costs are recorded separately and their total is charged once. Ignored tests remain inventory entries and receive Ignored observations.
Doctests and custom harnesses are visible unsupported targets. Toolchains and
dependencies must already be installed: research Cargo commands use
`--locked --offline`. External local path dependencies and source symlinks are unsupported in v1.

The additive `dctx0-test-analysis` crate owns pure typed models, old-coordinate
coverage matching, conservative selection and evaluation. It imports no engine,
store, filesystem or process adapter. Logical test keys contain project,
package name, target kind/name and full test name, without temporary paths or
package versions. Git project identities derive from root commit identities and
the project prefix, so local clones retain logical keys. Non-Git catalogs use
a persisted local project identity. The configured SQLite database and its
journals are derived state even when an explicit `--db` places them inside the
repository. Individual definition hashes remain unknown, with explicit
provenance; executable hashes appear only in raw execution provenance.

Collections distinguish catalog completeness, execution completeness and an
evaluable reference. Ordinary test failures are valid reference labels. Timeout,
crash, infrastructure failure and missing results cannot become PASS or ordinary
test failures. A build failure records a blocked event and diagnostic rather
than fabricating a successful collection. Catalog completeness applies to the
declared supported libtest scope only.

Shadow requires a complete new-format collection with exactly the requested
base commit, source contents and scope. Legacy coverage records alone cannot
supply this baseline. It materializes the target's Git-visible worktree changes
into an owned detached worktree, inventories tests and commits the prediction
before invoking target test bodies. New or unmapped tests are selected. Binary,
configuration, insertion and unobserved changes conservatively select the whole
scope. Every selection records its reasons. `can_skip_other_tests` is always
false.

A version-1 replay manifest explicitly selects chronological pairs:

```json
{
  "schema_version": 1,
  "scope": {
    "package": null,
    "filter": null,
    "all_features": false,
    "no_default_features": false,
    "features": []
  },
  "timeout_secs": 300,
  "strategy": "coverage",
  "pairs": [{"base": "BASE_COMMIT", "head": "HEAD_COMMIT"}]
}
```

References resolve to commit IDs before the replay begins. Each base must be an
ancestor of its head, and every preceding head must be an ancestor of the next
base. This intentionally rejects nonlinear series. A new invocation creates a
new replay ID; `--resume` requires identical resolved inputs and implementation
fingerprints. Completed pairs are retained. Interrupted collector steps retain
atomic execution files and reuse those completed executions on resume. Blocked
pairs preserve diagnostics while later independent pairs can proceed. Owned
worktrees and raw run artifacts are retained under `.dctx0ctl/analysis`; they are
not affected by benchmark history rotation. The user worktree is never checked
out or rewritten.

Research tables use `experimental_analysis_*_v1`, with separate extension
metadata. Definitions, observations, predictions and evaluations are immutable;
append-only events and progress caches commit together. Coverage references point
to the collector execution artifact, so test stdout/stderr cannot masquerade as
LLVM evidence. Explicit writes initialize
the extension. Read-only queries create neither a database nor extension tables.
The main SQLite schema version, compiler IR, fingerprints, task models and native
SDK protocol are unchanged. Existing coverage import/list/affected records remain
readable; new collections also append legacy coverage records for those queries.
Historical coverage does not acquire an invented complete catalog.

Research event/observation timestamps are UTC epoch milliseconds encoded as
strings; raw collector provenance also retains ISO timestamps. Durations are
milliseconds and RSS/artifact sizes are bytes.

Content-addressed artifacts retain hashes and byte sizes; temporary files are
renamed before publication. NDJSON exports distinguish frozen prediction inputs
from later observations and labels, linked by IDs. Export includes only the selected replay and the dependency closure of its
frozen inputs, reference labels and content-addressed artifact metadata. Related
collection and prediction events preserve publication ordering.
Offline recomputation consumes only this NDJSON. Each fresh Shadow invocation
creates its own occurrence and executes a new reference; only explicit replay
resume can retain completed steps. Selection reports distinguish
observed pass-to-failure transitions, existing failures and unmapped failures.
Failure classifications separately record known/new/unmapped logical identities,
known changed/unchanged definitions, unknown definition hashes, prior outcomes and
previously ignored tests. Definition hashes are compared only with matching
provenance; missing hashes stay unknown. Historical reports without these labels
read them as unavailable, not as confirmed zero counts. One observation does not
establish reproducibility or causal regression. Missing
references and references with no failures have no determinate failure recall.
Shared builds are counted once; selected-test cost is hypothetical. CLI-level
measurements are separate from collector-recorded build and isolated test costs.

Run the coupled partial checks and six release workloads with:

```sh
python3 scripts/test_analysis_acceptance.py --samples 3
python3 scripts/validate.py --focus experimental-coverage --samples 1 --iterations 1
```

Raw samples and check logs are retained; the latest 20 measurement summaries live
under `benchmarks/history/test-analysis`. The standalone runner is a partial measurement receipt; `scripts/validate.py`
merges these workloads into the accepted-baseline comparison with independent
speed/RAM tolerances and recent historical trends. No baseline is automatically
promoted. These workloads are unassessed until a compatible baseline is explicitly
accepted. Full workspace acceptance additionally requires:

```sh
python3 scripts/validate.py --coverage
```

No research result authorizes spec/task PASS, automatic skipping, reuse of PASS,
training or model adoption. Statistical validation on independent projects is a
separate research task.

Validation on 2026-10-28 passed all 32 workspace validation steps, including the
unchanged coverage gates: 41,369/42,590 merged source lines (97.13%) and
3,371/3,471 LLVM functions (97.12%). Raw LLVM source-instance coverage remains
separate; this is not complete branch coverage. The serial QA run used dev/test
optimization level 1 with assertions and overflow checks enabled, followed by
uninstrumented release measurements. It took 2,838,858 ms with a sampled peak
process-tree RSS of 1,037,082,624 bytes (shared pages counted per process).

All 88 release workloads completed. For the two-test research fixture, five
samples after one warmup produced these median CLI wall times and maximum RSS:

| Workload | Milliseconds | Peak RSS (bytes) |
| --- | ---: | ---: |
| Catalog | 664.569 | 158,814,208 |
| Collection | 1,260.294 | 159,092,736 |
| Prediction | 1.258 | 28,409,856 |
| Evaluation | 1.235 | 28,409,856 |
| Replay | 3,714.158 | 161,091,584 |
| Export | 3.184 | 28,409,856 |

Catalog/collection/replay include instrumented Cargo costs; the other phases use
the frozen exported dataset without Cargo. RSS uses Linux `wait4` maximum child
RSS, includes the inherited parent footprint, and does not sum concurrent
processes. Build/test subcosts, artifact volumes, raw samples, source pins and
cache conditions are retained separately. Performance remains **unassessed**
against an accepted compatible baseline; the baseline was not changed.
See [the validation index](../../../test-analysis-validation.json) for the exact
command, retained workspace log, raw coverage, earlier failed/interrupted runs,
and an exported dataset supporting offline recomputation.
