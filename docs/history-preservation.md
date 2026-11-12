# Explicit verification-history preservation

`history_retention: "preserve-all"` is an optional field in application
`.dctx0/testing/verification.json`. Existing configurations omit it and retain
ordinary minimal retention. Older strictly validating CLI builds do not understand
this new field; use the matching updated verifier rather than rewriting history.

An explicitly configured native or inspected publication includes the immutable
`application/vnd.dctx0.history-preservation+json` artifact. Store publication
links it before automatic retention in the same transaction. Any linked hold
protects the entire SQLite database, including unowned orphan bytes, because a
second project writer must not collect data belonging to the protected migration.
It requires no schema migration or reinterpretation of historical results.

The hold is sticky: removing the configuration field does not release a hold
already linked to a saved run. Normal writers keep all existing details; explicit
verification GC, export pruning and verification compaction/install entry points
refuse destructive maintenance while held. The read-only Store GC preview reports
zero prunable runs. There is deliberately no implicit release or cleanup command.
An unrelated orphan artifact by itself does not establish a hold.

Preservation is independent of evidence authority. It does not alter observations,
FAIL/UNKNOWN, task states, claims, reviews or adoption guards. Imports can request
retention but cannot gain native delivery authority from the marker. A failed or
partial new run remains failed or partial while its old comparison evidence stays
available. Previously pruned bodies cannot be reconstructed by adding a hold;
archive existing readable history before starting a migration.

The Runatron migration first archived its dirty sources and all three databases.
Its direct PreparedPlans binding and migration journal live in the consumer;
existing adapters/tests remain. Activate this option there only after the updated
verifier passes its focused retention/guard checks. Full final tool and consumer
acceptance remain required; the earlier Cinema release validation does not prove
these later changes.


For coupled contributor validation during a preservation migration, use
`python3 scripts/validate.py --coverage --preserve-history`. The explicit QA
option keeps all previous successful, failed and incomplete measurement records;
it does not promote the accepted baseline or change tests, budgets or tolerances.
Ordinary validation without this option retains its established latest-20 policy.
Do not run report-cleanup scripts against existing consumer reports during the
migration; retain new evidence under distinct run paths.

Large-model migration also retains the complete plan. An independently configured
`application_tests.plan_bytes` bounds its file (default 32 MiB, maximum 256 MiB).
Preparation and reading enforce the same limit, preserving an existing output on
failure. Runatron's unchanged full plan measures 85,778,516 bytes, so its measured
migration setting is 134,217,728 bytes. This is not permission to shrink mandatory
cases or to raise protocol-frame, case-evidence or time budgets.
