# Coverage measurement

As explicitly approved by the user on 2026-10-11, the gate is 97% of measured first-party source lines and LLVM functions. Historical 100% results below retain their original scope. No source files, ordinary errors, or production functions are excluded.

## Build isolation (2026-10-11)

The runner builds and exports from one fresh temporary target directory per
invocation, with both Cargo and cargo-llvm-cov bound to that directory. It removes
only its own temporary build after retaining the raw LCOV/LLVM exports. Existing
build caches are preserved. Rust/Cargo input hashes before and after the run must
match before the report can certify the final tree.

This corrects object discovery that mixed older instrumented binaries with current
profiles. A focused diagnostic contained multiple older function identities for
the same file and reported an unexecuted line at a currently blank location.
Those mappings cannot establish current-source coverage. This is separate from
the same-source cfg(test)/library instance limitation described below. No counts
are filtered or replaced, and the 100% source-line/function gates are unchanged.
Fresh workspace results are required; earlier percentages are historical evidence.

Run `python3 scripts/validate.py --coverage`. Its `scripts/coverage.py` stage runs instrumented workspace tests and enforces LLVM function coverage at 100%. It exports LCOV and JSON, sums line counts by `(source file, line)` across instrumented builds, and fails if **any** exported source line has zero executions. A line is covered only when an actual execution counter is positive; unrelated lines/files never compensate for a missing line. Tests verify merging and rejection of malformed exports.

## Instrumentation limitation

With rustc 1.97.1 / LLVM 22.1.6, the raw file/function-instance summary reports 19 uncovered lines in separately instrumented CLI, rename, store and task instances, despite zero uncovered lines in the merged LCOV export and source display. Libraries and their `cfg(test)` builds carry distinct instrumentation records. The source tests execute both successful and rejected paths across these builds. After the task increment (2026-10-09), the merged result is 8,591/8,591 source lines and 806/806 functions. The raw aggregate is 9,073/9,092 lines (99.79%); raw region coverage is 96.91%. Function coverage is 100%. Preserve this raw result alongside the merged result rather than relabeling it as 100% LLVM summary coverage.

LLVM does not export the exact per-line decisions behind its summary; cargo-llvm-cov consequently reconstructs source-line coverage separately. See the [LLVM export issue](https://github.com/llvm/llvm-project/issues/126307), [cargo-llvm-cov implementation](https://github.com/taiki-e/cargo-llvm-cov/blob/main/src/json.rs), and [LLVM coverage-region documentation](https://github.com/llvm/llvm-project/blob/main/clang/docs/SourceBasedCodeCoverage.md). These sources describe the measurement distinction; the numbers above are observations from this repository.

`target/coverage/workspace.lcov`, `llvm.json`, and `source-summary.json` retain the evidence and are uploaded in CI. The normal LLVM percentage report remains visible. This is a change in aggregation, not an allowance for untested source lines. Reassess the discrepancy when upgrading LLVM; do not raise an uncovered-line allowance or omit error paths.

Region/branch coverage remains a separate goal. Current stable Rust instrumentation reports regions but no branch counters; line/function completeness does not prove every branch or semantic behavior. Target/native helper coverage will be reported separately when those components exist.

## Editable profile increment (2026-10-09)

The profile increment measures **10,567/10,567 merged source lines and 910/910
LLVM functions**. Raw LLVM instances retain **11,100/11,124 lines (99.78%)**
and **16,214/16,771 regions (96.68%)**. No branch counters are supplied and no
new exclusions are introduced. Raw exports and the merged summary remain
separate under `target/coverage/`. The script now preserves exports even when
the function gate fails, and still rejects any missing merged line or function.

The system `/usr/bin/rustc` is 1.99.0 without matching coverage tools. This run
uses the installed rustup stable 1.97.1 / LLVM 22.1.6 toolchain via
`PATH=/home/held/.cargo/bin:$PATH python3 scripts/validate.py --coverage`.
CI installs `llvm-tools-preview` with its selected Rust toolchain.

## Published verification planning (2026-10-10)

The coupled planning checkpoint passes 173 all-feature Rust tests and 8 Python
tests, with 13,143/13,143 merged source lines and 1,099/1,099 LLVM functions. Raw
LLVM reports 13,793/13,819 instrumented lines, 20,447/21,048 regions and
1,400/2,369 instantiations; branch counters are unavailable. No exclusions or
threshold changes were introduced. See complete raw evidence (historical evidence is available in Git history).
All 82 paired release speed/RSS workloads complete; reduced development settings
remain incompatible/unassessed against the accepted baseline. This measures Rust
planning/guards and retains native execution as outstanding work.


## Native transport increment (2026-10-10)

The full coupled checkpoint passes 177 Rust/8 Python tests plus four Bun
conformance tests, with 13,244/13,244 merged Rust lines and 1,108/1,108 LLVM
functions. Raw LLVM reports 13,898/13,924 instance lines, 20,601/21,203 regions and
1,409/2,389 instantiations. No branch counters or exclusions were added. Native
helpers/generated TS are not measured by LLVM and still require separate line
coverage. Raw evidence (historical evidence is available in Git history) retains these distinctions,
84 release workloads and native transport timing/RSS samples. Native single
samples and reduced development settings remain unassessed against the baseline.


## Managed native executor (2026-10-10)

The coupled workspace check passes 192 Rust/8 Python tests plus four Bun tests.
Merged Rust lines are 13,979/13,979 and LLVM functions 1,183/1,183. Raw LLVM retains
14,672/14,710 instance lines, 21,561/22,197 regions and 1,531/2,537 instantiations;
branch counters remain unavailable. No exclusion or threshold changes were added.
Native/generated TypeScript coverage remains outstanding.
Retained evidence (historical evidence is available in Git history) includes all 84 release
speed/RSS workloads and separate native process samples. Reduced development
settings and single native samples are unassessed against the accepted baseline.


## Cinema scenario execution (2026-10-10)

The coupled check passes 202 Rust/8 Python/4 Bun tests, with 14,526/14,526 merged
Rust lines and 1,232/1,232 LLVM functions. Raw LLVM retains 15,251/15,289 instance
lines, 22,316/22,969 regions and 1,582/2,635 instantiations. Branch counters remain
unavailable; no exclusions or threshold changes were added. Native/generated
TypeScript helper coverage remains outstanding.
Raw evidence (historical evidence is available in Git history) retains the 84 release workloads,
native examples/winner/transport costs, separate compiler costs, source hashes
and intermediate failed/incomplete runs. Development settings and native single
samples remain unassessed; the accepted baseline is unchanged.


## Cinema real-state and lifecycle checkpoint (2026-10-10)

The coupled workspace check passes 217 Rust/8 Python tests, four Bun transport
tests and 36 independent native application tests. Merged Rust lines are
15,332/15,332 and LLVM functions 1,298/1,298. Raw LLVM separately retains
16,105/16,144 instance lines, 23,475/24,147 regions and 1,658/2,769 instantiations.
Branch counters remain unavailable; there are no new exclusions or threshold
changes. Typed trace dispatch removes an impossible panic branch instead of
excluding it. Negative tests exercise rejected state/trace values, missing
contracts/observers, lifecycle guards and native setup/protocol/deadline failures.

Bun LCOV independently reports 210/210 production application lines and 131/131
functions across the Cinema domain, services, SQLite ports, Hono routes and Awilix
session. This does not measure the generic generated worker or testing adapter;
that helper coverage remains required. Raw checkpoint evidence (historical evidence is available in Git history)
retains full release speed/RSS records, native observations and source provenance.
Development sampling is unassessed against the accepted baseline; no baseline
promotion occurred. Complete verification CLI/history/adoption remains pending.


## Verification setup and configured planning (2026-10-10)

The coupled workspace run passes 224 Rust/8 Python tests, four Bun transport tests
and 36 native Cinema tests. Merged source lines are 15,619/15,619 and LLVM
functions 1,313/1,313. Raw LLVM separately reports 16,397/16,439 instance lines,
23,956/24,680 regions and 1,695/2,826 instantiations; branch counters remain
unavailable. Filesystem metadata/publication failure tests preserve ordinary
error-path coverage and demonstrate resumed setup. No exclusion or threshold
change was made. Retained evidence (historical evidence is available in Git history) includes
all 86 release workloads and actual native scaffold/Cinema observations.
Development settings remain unassessed; the accepted baseline is unchanged.
Generic generated-worker/adapter coverage and the full native workflow remain
required.


## Immutable verification history and saved evidence (2026-10-10)

The coupled run passes 229 Rust/8 Python tests, four Bun transport tests and
36 native Cinema tests. Merged source lines are 15,894/15,894; LLVM functions
1,343/1,343. Raw LLVM retains 16,689/16,732 instance lines, 24,466/25,239 regions
and 1,728/2,889 instantiations. Branch counters remain unavailable; no exclusions
or threshold changes were added. Raw evidence (historical evidence is available in Git history)
retains all 90 release workloads and provenance. Development settings remain
unassessed and the accepted baseline is unchanged. The new history is not native
execution/adoption authority; complete run orchestration and helper coverage remain
required.

## Native Bun helper coverage (2026-10-10)

The transport conformance fixture now runs actual generated worker variants
inside `bun test --coverage --coverage-reporter lcov`. It retains every raw
per-worker LCOV report, source SHA-256, merged reported line locations and raw
per-run function totals in `target/native-harness-conformance/*.json`.
These counters are separate from Rust/LLVM; they do not satisfy or change the
mandatory 100% Rust source-line/function gate.

The first successful diagnostic run reports 129/130 native line locations for
`worker.ts` and 4/4 for `contract.ts`. The uncovered worker location is the comment
about rejected adapter promises, not an executable statement. Actual rejected
translation promises are covered by protocol conformance cases. No source is
deleted or excluded to hide this Bun location-mapping limitation. Scaffold
variants have their own partial reports and do not imply full application
coverage.

Bun's LCOV supplies `FNF`/`FNH` totals without `FN`/`FNDA` identities, and those
totals vary between executions of the same worker bytes. The collector retains
per-run totals and reports merged function coverage as unavailable rather than
inventing a union or zero functions. Coverage-instrumented native timing/RSS
samples are explicitly labeled and cannot be used as release benchmark evidence.
Only the instrumentation host's initial Bun banner is recorded separately by the
fixture client; ordinary native execution retains its strict protocol decoder.

### ESM module observer native coverage (2026-10-10)

The generated TypeScript ESM observer has independent Bun LCOV from six actual
worker scenarios covering unexecuted/executed sites, UTF-8 offsets, changed
source hashes, invalid ranges, unsupported native arguments and trace exhaustion.
Merged observer-helper source lines are 54/54. Raw per-run function totals are
retained without claiming complete merged function identities. This measurement
is separate from required Rust coverage and from release performance;
retained native evidence (historical evidence is available in Git history) records the scope.
New Rust observer/planning/orchestration code still requires the final full Rust
coverage run; no Rust thresholds or exclusions are changed.

### Cargo cache tag for isolated builds (2026-10-11)

The coverage runner writes a standard `CACHEDIR.TAG` into only its newly created
temporary build directory before invoking Cargo. Without that tag, current
Cargo refuses the initial clean of an existing directory and emits a warning;
compilation still proceeds in the empty directory. A real Cargo clean probe
accepts the tagged owned directory, and all 13 Python runner tests pass. The
marker does not authorize cleaning a shared target directory, reuse old binaries,
filter metrics or change thresholds. The already-running instrumented measurement
was not restarted for this runner preparation fix.
