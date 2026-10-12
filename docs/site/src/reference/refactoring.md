# Refactoring boundaries

The refactoring keeps the seven-crate workspace and existing language behavior. Domain rules remain ordinary functions and structs. Ports represent filesystem/history effects; no service registry or new framework is introduced.

| Concern | Location | Extension rule |
| --- | --- | --- |
| Scoped symbol resolution | `language/src/resolution.rs` | Preserve exact-name precedence, scope ranking, and ambiguity ordering. |
| Declaration models | `language/src/compiler/types.rs` | Compile data/operations separately; validate aliases after model construction. |
| Semantic relations | `language/src/compiler/relations.rs` | Keep effects, architecture, obligations, state, and refinements separate. |
| Typed expressions | `language/src/expression/` | Keep parsing, type rules, constructors, calls, state, and clause scopes distinct. |
| Snapshots and diffs | `changes/src/{snapshot,diff,hashing,model}.rs` | Use named projections and canonical JSON; serialize enums using existing names. |
| Rename continuity | `changes/src/history.rs` | Compose explicit mappings; deletion ends identity, even if a name is reused. |
| History persistence | `store/src/lib.rs` | Implement history ports; read the old head and commit snapshot/event inside one transaction. |
| Rename planning | `engine/src/rename/plan.rs` | Consume sources and return proposed edits/snapshots without I/O. |
| Rename execution | `engine/src/rename/journal.rs` | Preserve locking, durable writes, source verification, and crash recovery. |
| CLI | `cli/src/{args,commands,render,benchmark}.rs` | Keep command syntax, application calls, formatting, and measurements separate. |

Paths in the table are relative to their corresponding `crates/dctx0-*` directory. `ExpressionContext` names the scope carried by expression compilation. An internal `ExampleStep` names invocation/outcome components while retaining their existing IR vector order. `RenameError` retains structured diagnostics and existing user-facing messages. `ChangeFacet`, `StatePhase`, and `AccessMode` replace ambiguous strings/booleans.

`HistoryReader`/`HistoryWriter` are domain-owned contracts with adapter-specific errors. Check publication consumes a history writer. `SourceReader`/`SourceWriter` are engine-owned source ports; callers coordinate multi-file writes with the project lock. SQLite transaction behavior and filesystem journal durability remain adapter responsibilities. Rust callers must import traits for their methods and choose an explicit database access mode.

Indexes are ephemeral and deterministic: qualified-name suffix candidates preserve symbol order, owner indexes preserve clause order, and graph traversal preserves sorted explanation paths. An unchanged diff builds no graph indexes. Targets and a future task DAG remain separate future work.

Validation includes existing reference fixtures, negative language cases, file races, interrupted journals, lineage chains, and the full pre-refactoring snapshot hashes of both examples. Coverage and release speed/RAM are measured together with `python3 scripts/validate.py --coverage`. Keep the last 20 runs and the accepted baseline separate; do not promote measurements automatically. The comparison artifact `benchmarks/refactoring-comparison.json` records before/after values and provenance.

## Recorded result

Validation passed: 67 Rust tests, 8 Python tests, rustfmt, and Clippy with warnings denied. Coverage records 5,137/5,137 merged source lines and 100% LLVM functions; the raw LLVM instance line summary is 99.89% (see `coverage.md`). Both complete example snapshots match their pre-refactoring IDs and serialized hashes.

Against the last passing pre-refactoring run on the same recorded environment, runatron-next compilation fell from 557.936 to 181.591 ms (-67.5%) and check from 1,029.951 to 593.821 ms (-42.3%). Compile RSS rose from 55.82 to 56.84 MiB (+1.8%); check RSS changed from 339.41 to 338.70 MiB (-0.2%). Across all workloads, measured RSS changes are between -2.7% and +1.8%. Some small workloads are slower: parser-next +12.4%, graph-next +14.0%, and changed-diff-cinema +12.7%. These are individual-run comparisons with five raw samples, not proof of a long-term trend.

All 18 workloads and both full raw records are retained in `benchmarks/refactoring-comparison.json`. The accepted baseline is unchanged. Its parser gates pass; the other 16 workloads still lack accepted baseline values, so the normal performance gate explicitly reports `partial`. This comparison does not change that status or accept a new baseline.
