---
name: dctx0-language
description: Author, review or repair .dctx0 specifications with the Rust compiler, including typed contracts, examples and lifecycles.
---

# dctx0 language

Treat reviewed `.dctx0` files as the agreed behavior. Preserve the user's intent;
report missing business decisions instead of inferring assertions from narrative.
Read [references/language.md](references/language.md) for syntax and identity rules.
Use the project's complete examples for complex supported clauses, then validate
with `dctx0 check <spec-root>`. `parse <file-or-root>` diagnoses syntax alone.
Independent dctx0 commands may run in parallel against the same local database;
SQLite transactions and locks protect it, and `check` takes the project lock.
Coordinate concurrent spec edits separately so checks see the intended sources.

Keep source composition and implementation binding distinct. A spec can span
sorted files through top-level fragments; application names, libraries and file
layout belong to the external test adapter and editable target profiles.

Compilation is not execution. `verify plan` reveals executable obligations and
unsupported/manual points; native `verify run` supplies actual observations.
After an authorized spec change, inspect its diff and the resulting task/package
delta before resuming implementation. Preserve historical results; changed
requirements need current evidence. Do not weaken an invariant, remove mandatory cases or change expectations to
accommodate an implementation failure. Unsupported execution should remain explicit.

Use `diff`, `impact` and explicit `rename --dry-run` when requested to understand
changes and continuity. Plain remove/add edits do not imply a rename. `stableId`
retains its data-contract meaning; it is not universal symbol identity. Never
edit derived SQLite history to change requirements or outcomes.

For inventory and verification investigations, read [the query guide](references/query.md). Start with filtered lists or SQL aggregates, then fetch individual details.
