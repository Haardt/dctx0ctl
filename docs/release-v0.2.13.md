## Structured verification causes and conservative run comparison

Verification diagnostics now project structured issue-code causes and concrete
next steps. `verify show SPEC RUN --cause adapter,environment --limit 20 --json`
filters cases, and compact summaries retain additive cause counts. Historical
summaries without these counts report unknown rather than inventing evidence.

`verify compare SPEC BEFORE AFTER --limit 20 --offset 0 --json` compares indexed
case metadata without decoding the full observation corpus. Exact identities,
requirements and assessment versions govern comparison. Only a matching newly
executed native PASS can resolve an old problem. Removed cases, changed
requirements, reused observations and pruned details are not counted as repairs.

**Intentional compatibility change:** newly assessed unclassified application
exceptions produce UNKNOWN, not FAIL. Actual wrong returns and witnessed contract
violations remain FAIL. Historical results are unchanged. Assessment version 17
and interpreter version 6 distinguish the new evaluation basis. TypeScript and Go
adapters can explicitly identify environment and adapter errors through existing
technical-error packets; a missing requested entity observation is an adapter
failure, while an explicit empty list is a valid observed empty state. SQLite
schema and transport wire format are unchanged.

External workflow metrics accept explicit activities, correlation fields, nullable
token counts and declared provider cache/reasoning semantics. The three embedded
skills are version 2 and document the available commands. CLI-generated IDs and
phase tracing are not part of this release.

The eight-part workflow proposal remains incomplete. Nested native clause
continuations, preflight, registered external evidence roots, closure preview,
complete separate inspection/native aggregates and isolated Identity/Access
acceptance remain outstanding. This release makes no completion claim for them.

The pre-version-bump coupled checkpoint passed 627 Rust test functions and 40
Python tests, supported CLI feature combinations and native Cinema mutation/replay
checks. Coverage was 38,344/39,492 merged source lines (97.09%) and 3,139/3,234 raw
LLVM functions (97.06%). Raw LLVM measurements are retained separately; this does
not establish branch completeness. All 81 release speed/RSS workloads completed
with one measured sample/iteration and one warmup; general baseline comparisons
remain unassessed. The accepted baseline is unchanged. Exact inputs and evidence
are recorded in `docs/verification-workflow-validation.json`. Normal tag CI and
all five native distribution gates are required before publication; no release
validation exception is introduced.

The v0.2.13 release binary also passed the coupled local distribution/installer
and CLI/LSP/harness/cancellation acceptance. Its one-sample package/startup
measurements and raw results are retained in
`benchmarks/history/release/20261024T232106.298727Z-918426.json`. Distribution
measurements have no accepted comparable baseline and remain unassessed.
