# Immutable verification history

Writer schema 4 additively extends schema 3 with immutable verification runs,
content-addressed artifacts, run/artifact membership and future task-link/adoption
records. Existing snapshots, task definitions, work/check results and profiles are
preserved. Read-only opening accepts schemas 1–4 and never migrates them. Legacy
readers return no verification runs when their schema has no verification tables.
Writers perform the existing 1→2→3 migrations before adding schema 4.

The new verification report begins at schema 1. Existing task reports remain
schema 2. Core owns lossless typed run, case, state and provenance models. Each
stored report pins a successful project check occurrence and snapshot, mandatory
cases and subjects, recorded outcomes, open/bounded obligations and immutable
artifact references. Provenance has fields for implementation/dependencies,
adapter/configuration/harness, interpreter/generator/protocol/target/runtime,
build configuration, budgets and optional task/profile pins. Unknown or incomplete
inputs remain explicit; a report's existence alone does not make those inputs
complete or authoritative.

Artifact IDs are SHA-256 of exact bytes. Storage checks byte length, media type,
digest and exact supplied membership, including the pinned plan. Conflicting
content/metadata cannot replace an existing artifact. Run IDs address canonical
serialized report content; actual execution occurrence timestamps distinguish
fresh runs. Repeating the same stored report is idempotent. Run/artifact publication
uses one SQLite transaction, and a publication failure rolls back artifact writes.

Contradictory PASS, missing/surplus/duplicate/unscoped cases and mismatched check
occurrences are rejected. An imported report may be retained as imported evidence;
it cannot change work progress, testing state, task links or adoption receipts.
No automatic adoption API is implemented by this increment. Native execution
provenance and guarded eligibility must be validated by the actual managed-run
workflow before any later adoption action; a self-declared origin is insufficient.

`dctx0 verify show <spec-root> <run-id>` reads saved reports in Markdown;
`--json` supplies the complete typed record. It does not check current source
freshness, recover files, migrate SQLite, invoke the adapter or application,
re-evaluate cases, or update tasks. There is no native `verify run` command yet;
managed whole-plan execution/publication and guarded adoption remain required.

The managed worker retains an ordered `NativeSessionEvidence` journal for the
latest requested case, including when execution returns a transport error or
times out. It records acknowledged Begin/End replies, actual operation replies
(including events and optional call observations), state observations, received
mock requests and successfully written mock replies. A written mock reply is
not proof that the worker consumed it. Invalid/missing replies add no invented
observation, and failed cleanup adds no End marker. Each execution attempt resets
the journal, including attempts rejected before a session starts. The runner can
read this prefix after cancellation; it is not a standalone PASS, complete run,
or adoption authorization.

This is an explicit SQLite compatibility increment. Snapshot IR/version and
compiler goldens are unchanged. Raw LLVM metrics and paired release speed/RSS
measurements remain required; no baseline promotion is automatic.


Validation checkpoint (2026-10-10): 229 Rust tests, eight Python tests, four Bun
transport tests and 36 native Cinema tests pass. Merged Rust lines are
15,894/15,894; LLVM functions are 1,343/1,343. The release runner measures
90 workloads, including separate warm report publication and read phases for
both complete examples. These benchmark reports are explicitly imported UNKNOWN
records and make no native execution/adoption claim. Preparation publishes a
representative compiled snapshot and pinned plan outside the measured loop; RSS
includes preparation. Publication records a fresh report occurrence each iteration
with already-cached artifacts. Read phases load the same immutable saved report.
Reduced development sampling is unassessed; the accepted baseline is unchanged.
Retained evidence (historical evidence removed during repository cleanup) keeps full measurements
and distinct raw LLVM metrics. Native managed whole-plan execution and adoption
remain the next required implementation.

Session-prefix continuation (2026-10-10): the complete coupled validation passes
229 Rust tests, eight Python tests, four Bun transport tests and 36 native Cinema
tests. All 15,931 merged Rust lines and 1,344 LLVM functions are covered. Ninety
release workloads complete with stable source provenance; reduced development
sampling is unassessed and does not promote the baseline. Raw LLVM lines,
regions and instantiations remain separate in
retained evidence (historical evidence removed during repository cleanup). Error-prefix tests
exercise malformed Begin/Invoke/End replies, rejected mock requests, actual
Cinema mock replies, state observations and repeated fresh cases.
