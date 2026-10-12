# Task planning and administration

The Rust task core plans from a published, valid spec snapshot. It does not require targets, execute agents, interpret examples, or run native checks. Tasks, plans, results, reservations and append-only history live in the existing project SQLite database.

## Daily workflow

```sh
dctx0 check examples/cinema/dctx0
dctx0 tasks preview examples/cinema/dctx0
dctx0 tasks plan examples/cinema/dctx0
dctx0 tasks waves examples/cinema/dctx0
dctx0 tasks list examples/cinema/dctx0 --wave 1 --state open
dctx0 tasks next examples/cinema/dctx0
dctx0 tasks claim-next examples/cinema/dctx0 --actor agent-1
```

The claim receipt supplies a task ID, claim token and state version. Read the actual assignment before implementing it:

```sh
dctx0 tasks show <root> T42
dctx0 tasks context <root> T42
dctx0 tasks context <root> T42 --section types
dctx0 tasks dependencies <root> T42 --transitive
dctx0 tasks history <root> T42
```

`show` and `next` include the actual assignment, specific acceptance criteria and complete declaration blocks by default. Entity/message/event fields, keys, annotations and refinements remain in their original spec text; referenced types accompany them. The same context is available through `context`, including intent, constraints, decisions, examples, effects, state rules and the complete required type closure: aliases, fields, variants, failures, keys and refinements. JSON additionally supplies typed data and operation models for the selected content. Given steps remain collaborator stubs; spec compilation is not execution evidence.

Complete or release with the current claim and version:

```sh
dctx0 tasks complete <root> T42 --actor agent-1 --claim 123 --version 1 \
  --summary 'Implemented the modeled behavior and failure cases' \
  --artifact src/operation.rs --implementation commit-or-content-hash
dctx0 tasks release <root> T42 --actor agent-1 --claim 123 --version 1 \
  --reason 'Returning unfinished work'
```

`Open → Progress → Done` describes work. Done never creates PASS. Release returns unfinished work to Open and preserves the attempt. There is no generic status setter or reopening of completed work.

## Granularity, prerequisites and changes

Normal units are one data/operation contract, one surface operation, or one port operation. Recursive contract components are grouped with an explicit reason. Module integration and spec reviews are separate tasks. External services contribute contracts rather than invented implementation work.

Every hard dependency names the needed result and its reason. Calls normally require published contracts; they do not automatically require completed callee implementations. Semantic cycles, execution prerequisites, historical links and physical write conflicts remain distinct. Waves describe topology; unrelated earlier work does not create a global barrier.

Relevant spec changes create new task IDs and concrete before/after facts. Unchanged work retains IDs. Explicit renames produce history links; snapshot hashes and declaration names alone are not permanent identity. Plans also pin check occurrences, so deletion and A→B→A do not silently resurrect old work. Begun or delivered removed requirements produce retirement tasks when their role remains absent; retirement waits for affected former consumers. Historical Done/PASS records remain available. Integration and review reuse must compare prerequisite task identities as well as requirement hashes; identical text after a continuity break is still new work.

`tasks plan` publishes only the latest valid snapshot; `--snapshot` pins it. Failed checks retain the valid spec head. Reading saved tasks neither compiles current files nor claims working-tree freshness. Historical views use `--plan P7` or the task's latest saved membership. `tasks diff <root> P1 P2` compares memberships and history relationships. Repeated current planning is idempotent. Historical `preview --snapshot <id>` shows the snapshot's candidate plan without reconciling against current work.

## Formats, filters and pagination

Markdown is the task-command default. `--json` or `--format json` produces schema-versioned typed reports. The existing commands retain their prior output conventions. Human output omits empty values, duplicate own-type declarations and initial-implementation change blocks. Instructions and acceptance are prose; exact spec excerpts are fenced code. JSON additionally retains original definitions/state and structured data/operation models. Missing raw source is explicitly labeled as a reconstructed contract, with retained clauses.

List filters combine `--wave`, `--state`, `--check`, `--kind`, `--subject`, `--domain`, and `--actor`. `--all-plans` explicitly includes historical tasks. `next` reads only and displays the selected assignment, contract, acceptance instructions and required spec context directly; `--budget`, `--section` and pinned continuation cursors work as for `context`. JSON retains the overview and adds a structured `context` field when a task is selected. `claim-next` selects and starts atomically. Both select ready Open work rather than blocked or already claimed tasks.

Prerequisite overviews include available artifact paths and implementation decisions; full historical task bodies remain explicit lookups by ID.

Lists default to 50 rows. `show`, `next` and `context` share a soft 24-KiB text-content budget, selected with `--budget`. `--section goal`, `acceptance`, `contract`, `types`, `rules`, `examples`, `decisions`, `changes`, `dependencies`, `history` or `result` selects a specific part. Follow the printed `--cursor` using the same query options. Cursors pin plan, event sequence, query, presentation version and offset. Older-layout cursors are rejected after an upgrade rather than resuming at a misleading offset. Required content is never silently cut: missing sections and continuation are explicit. Indivisible items can exceed the budget. Byte/character metrics describe selected text content; tokens remain unmeasured until a named tokenizer is supplied.

## Domain slices

```sh
dctx0 tasks slice <root> T42
dctx0 tasks claim-slice <root> T42 --actor agent-1
dctx0 tasks start <root> T17 --actor agent-1 --reservation 7
dctx0 tasks release-slice <root> G7 --actor agent-1 --reason 'Free future work'
```

A later task anchors its transitive prerequisite closure, including overviews down to the roots. Delivered results form the boundary; unfinished work is explicit. A domain filter exposes external open prerequisites rather than hiding them. Structural closure does not prove agent capability or resource isolation.

Slice claims reserve atomically and keep future work Open. Start each task only when its prerequisites are delivered. Own existing claims remain visible; foreign ownership blocks reservation. Release frees unstarted reservations while preserving ongoing claims. There is no automatic lease expiry. Relevant plan changes invalidate affected reservations without pretending work was done.

## Results, checks, reviews and follow-up

`complete --file result.json` accepts:

```json
{"summary":"Delivered behavior","artifacts":["src/operation.rs"],"decisions":["Preserve port boundary"],"implementation":"commit-or-content-hash"}
```

`record-check <root> T42 --file check.json` imports a report of an actual externally executed check. It requires a Done result and matching requirements hash and implementation. Required fields are `requirements_hash`, `implementation`, `verifier`, `verifier_version`, `command`, `started_at`, `finished_at`, `report`, and `status` (`pass`/`fail`). Manual completion and LLM review are not executed-check evidence. Imports do not run checks or assess current files; prior reports remain in history.

A Review task requires `record-review` before completion. Supply its actor, claim and current version plus `--file`. Reports contain `reviewer`, `judgement` (`approved`/`request-changes`), `summary`, and `findings`; findings contain unique `id`, `severity` (`low`/`medium`/`high`), `description`, `task_ids`, and `action_required`. Recording increments the state version and returns the new guard. Review approval does not become testing PASS.

Follow-up is explicit:

```sh
dctx0 tasks follow-up <root> T42 --reason 'Repair the failing implementation'
dctx0 tasks follow-up <root> T90 --finding F1 --reason 'Repair the reviewed boundary'
dctx0 tasks add-note <root> T42 --reason 'Useful implementation context'
```

Follow-up creates new work and required integration/review revisions, preserving old results. Repeated identical requests do not duplicate work. Notes do not alter requirements. A desired product change still belongs in the spec.

## Architecture

`dctx0-tasks` owns pure planning, contract SCC grouping, reconciliation, queries, context and lifecycle rules. `dctx0-store` handles SQLite transactions and append-only events; `dctx0-engine` coordinates project locks, snapshots and recovery. CLI arguments, command orchestration and Markdown rendering are separate modules. No target or interpreter dependency enters task planning.

## Storage and validation

Schema version 2 adds task tables transactionally. Version-1 history remains readable; writer opening performs the additive migration. Readers create no plan, run no checks and perform no migration. Unknown newer schemas and corrupt histories are rejected. Publication compares head, check occurrence and event sequence inside its transaction; claims use actor/attempt/version guards.

Run `python3 scripts/validate.py --coverage`: tests, strict coverage and release speed/peak-RSS measurements are coupled. The runner includes eight task phases for each full example in addition to the existing workloads. RSS includes prepared IR/SQLite and is not incremental task allocation. The latest 20 runs remain recorded; baseline acceptance stays explicit and new workloads remain unassessed until intentionally accepted. The `tasks-publish` workload publishes an actual new source snapshot with unchanged semantic requirements on every iteration; snapshot preparation is outside its timer. Its protocol version is 2, so earlier no-op publication timings are not comparable.

## Target implementation guidance

Task commands accept `--project` and `--target`. `show`, `next` and `context`
include implementation profiles; `--section implementation` loads them explicitly.
Claims and slices freeze their used texts transactionally. `profile-changes`
previews drift and optional explicit selections; `profile-update --file ...
--reason ...` creates immutable adaptation work for selected completed tasks.
See [editable profiles](profiles.md) for the complete workflow. Extended CLI
report envelopes use schema 2; SQLite writers now migrate additively to schema 4
for native verification history. Readers accept schemas 1–4 without migration.

## Verification planning for saved tasks

`dctx0 verify plan <root> --task T31 --task T60 --json` derives a read-only
verification scope from saved task subjects and their exact published check
occurrence. It preserves task definitions, progress, testing evidence and profile
bindings. A changed spec requires `check`; a changed check occurrence requires
matching task publication. Planning does not start, reserve or complete work.
See [verification planning](verification-planning.md) for symbolic inputs, complete
typed reports. Existing `record-check`
reports retain their original behavior and do not authorize automatic adoption.

## Native verification and guarded adoption

```sh
dctx0 verify run <root> --task T93
dctx0 verify run <root> --task T93 --no-adopt
dctx0 verify show <root> <run-id>
```

Native execution uses the configured application adapter and published task
requirements. It records immutable cases, observations and provenance. Without
`--no-adopt`, eligible active Open tasks may be delivered atomically after a
complete native PASS run and complete evidence for their requirements: all
mandatory mapped checks must PASS, with no
missing obligations, provenance gaps, stale pins or unmet scheduling prerequisites.
Claims and reservations prevent automatic delivery. Review and retirement work
are excluded; integration and production-adapter work require their own complete
native integration/adapter evidence. Bounded contract adoption is demonstrated;
the incomplete whole-Cinema evidence currently authorizes no automatic adoption.

The transaction uses ordinary start/complete/check events and freezes relevant
profiles. Reapplying the same publication is idempotent. Historical Done/PASS
results and immutable definitions are preserved. Rollback affects task events,
results and profile bindings only, never code or specifications. `--no-adopt`
stores evidence without changing task progress; `show` does not claim fresh code.
UNKNOWN is a verification outcome, not a new work state or task check status.
See [the execution workflow and evidence limits](verification-run.md).

## Source-bound agent conformance reviews

Planner version 2 assigns manual requirements and conformance suites explicitly
through `review_obligations`. Agents obtain bindings with `verify review-template`,
submit them through `tasks record-review`, and complete review work through the
usual claim/version guards. `verify complete` consumes these persisted reports;
a Done/Unverified review can satisfy an authored obligation without inventing a
test PASS. `tasks renew-review` creates a successor after implementation changes.
See [the complete workflow and evidence limits](conformance-reviews.md).
