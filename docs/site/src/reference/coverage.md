
# Experimental execution corpus (2026-10-28)

The opt-in `dctx0 experimental coverage` commands use separate versioned tables
in the existing database. This corpus is independent of the aggregate 97% gate
and all specification/task evidence. Each Linux libtest execution retains its
LLVM export and merged profile, timing and `wait4` RSS; the raw exports may also
contain dependency/test code. The lookup expands positive coverage segments
into observed file/line memberships. It does not reconstruct ordered call paths
or prove that unobserved tests are unaffected by a subsequent change.

The embedded Python process adapter is exercised separately by Python tests
with real LLVM output and timeout/failure cleanup. These tests are not a claim
of Python source coverage. CLI/storage integration tests exercise collection,
append-only imports, interval boundaries, project isolation and read-only
queries. The main SQLite history schema version is unchanged; explicit writes
create additive `experimental_coverage_*_v1` tables transactionally. SQLite
backup-based maintenance retains these tables.

Peak RSS uses Linux `wait4` `ru_maxrss`, including process startup and waited
children; it is not incremental allocation, post-exec-only RSS or a sum of
concurrent descendants. Execution time includes supervisor polling (20 ms),
so short tests have a measurement floor. Build costs are separate from individual
test costs. The initial collector uses instrumented debug builds for coverage;
these costs must not be compared to uninstrumented release performance baselines.

Development validation is reproducible with `python3 scripts/validate.py --focus
experimental-coverage --samples 1 --iterations 1`. This explicitly partial focus
pairs the collector/store/CLI checks with release speed/RAM measurements. The
2026-10-28 run completed 82 release workloads, with performance unassessed against
the accepted baseline. The attempted full workspace run failed an independent
Skills catalog assertion (24 installations versus 18 expected) and was stopped
after the lengthy Cinema integration case; workspace coverage acceptance is not
claimed. Full evidence and raw release samples are retained in
`docs/experimental-coverage-validation.json`.

## Affected project tests (2026-10-28)

`dctx0 experimental coverage affected <application-root> --base HEAD` compares
Git-visible staged/unstaged changes and untracked files with recorded **project
Rust/libtest coverage**. No DCTX0 specification or generated verification harness
is required. Git and Python 3 are needed for the query; collecting profiles also
needs the Rust LLVM tools. Ignore `.dctx0ctl/` before collecting a clean baseline.

The base is resolved to a commit. Matching uses the old hunk coordinates, not
current shifted line numbers. Positive old-line overlap produces candidates with
historical execution IDs, baseline outcomes, reasons and optional Cargo argument
arrays. Insertions may use the old neighboring lines as suggestions, while the
inserted code remains unknown. The query never executes its suggested commands.

Only exact-revision clean records with available coverage, source/environment
hashes and compiler identity are eligible. Timeout records are excluded. The
newest compatible cohort is selected, deduplicating by test and target metadata.
New captures use target-kind-qualified identity version 2; older immutable rows
remain readable. Older records without invocation metadata return null commands.

Missing coverage, new/untracked files, binary/mode changes and build/configuration
inputs are explicit unknowns. Complete catalog coverage or new control-flow paths
cannot be established from historical profiles. `can_skip_other_tests` therefore
remains false. An empty candidate list is not permission to skip testing. Git
renames are treated as deletion plus addition, without identity heuristics.

The database is opened read-only; a missing database produces unknown baseline
information without creating state. Specification checks, task state and testing
PASS evidence are unchanged. Native SDK/harness case selection and non-Rust
collectors are separate future extensions.

Affected-query validation: the feature-scoped checks and 82 release workloads
completed successfully with reduced sampling; the accepted performance baseline
was not promoted and comparison remains unassessed. A separate release-CLI
experiment uses a normal Git/Cargo project with two tests and no DCTX0 spec or
SDK adapter. Changing one production function yields exactly its calling test as
a candidate; the full suite confirms that test fails while the other passes.
The emitted Cargo argv reproduces the failure, and insertion/untracked-file
experiments retain explicit unknowns. Raw measurement samples, the fixture,
commands and output are retained in `docs/coverage-affected-validation.json`.
This is not a full-workspace coverage acceptance or statistical recall claim.
