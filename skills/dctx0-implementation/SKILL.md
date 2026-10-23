---
name: dctx0-implementation
description: Implement or repair dctx0 task requirements and testing-only adapters using pinned task context, editable profiles and actual native verification.
---

# dctx0 implementation

Start from the supplied task or the user's explicit application scope. Read
`tasks show`, required paginated `tasks context`, actual prerequisites and the
selected profiles before editing. Inspect relevant command help; do not translate
old Runatron task envelopes or tool names into invented Rust commands.
Read [references/implementation.md](references/implementation.md) for target and
evidence details when implementing adapters or interpreting incomplete verification.

For multi-task work, delegation or a resumed context, use the pinned package and
closure routine in [references/work-packages.md](references/work-packages.md).
It covers shared context, deltas, bounded handoffs, focused diagnosis and delivery.

Keep task definitions immutable. Claims/reservations, start/release/complete,
checks and reviews are guarded events. Respect another worker's ownership and
existing scope; dependency-ready does not itself reserve work. Independent dctx0
commands can run in parallel against the same local database; SQLite WAL, write
transactions and project locks protect shared state. Use atomic claims/reservations
for workers. Database protection does not coordinate edits to application files;
keep concurrent file ownership clear. On a busy timeout or stale-state conflict,
reload state before retrying. Do not implicitly
expand a task to unrelated application changes. Ask only for genuinely missing
requirements or authorization not already supplied by the user.

Production uses its own types, frameworks, files and constructors. It imports no
dctx0 runtime, spec or generated harness. A reviewed testing-only adapter calls
actual production, translates representations, creates/cleans sessions and
replaces declared collaborators through real dependency boundaries. Do not rebuild
business logic or constant expected responses inside the adapter.

Use native verification against the configured scope. Inspect mandatory/recorded
case counts, observations, unsupported obligations and provenance before claiming
success. Repair FAIL in actual implementation; retain UNKNOWN where observation
or executable meaning is absent. Replay saved counterexamples to verify repairs.
Automatic adoption requires complete native evidence and current transactional guards.
Source-bound authored review is separate and cannot approve away native failures.

Honor the project's test budget and validation workflow. Group expensive checks
when appropriate and measure performance last if requested. Preserve genuine
mandatory cases and meaningful tests. Editing profiles or completing work does
not establish PASS. Commit/push/deployment actions follow the user's authorization.

For inventory and verification investigations, read [the query guide](references/query.md). Start with filtered lists or SQL aggregates, then fetch individual details.
