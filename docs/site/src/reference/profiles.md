# Editable target profiles

For a practical introduction covering profile roles, application configuration,
and extending shipped defaults in a DCTX0 checkout, read
[Configure and extend profiles](../docs/profiles.md).

The compiler and planner remain language neutral. Four separate crates under
`targets/{typescript,java,go,rust}` implement the core-owned
`TargetProfileProvider` contract. They supply embedded Markdown defaults and a
versioned manifest, without generating code or verifying implementations. The
CLI statically registers them through the `typescript`, `java`, `go`, and `rust`
Cargo features; all four are enabled by default. No target SDK or interpreter is
introduced by this increment.

## Create and edit

Run these commands in the application directory, independently of the spec
root:

```sh
dctx0 profile typescript create
dctx0 profile java create --project /path/to/app
dctx0 profile rust create --default --agents
dctx0 profile go create --dry-run
```

The first target becomes the project default. Other targets change the default
only with `--default`. Profiles live under `.dctx0/profiles/<target>/common.md`
and `<implement|use>/<semantic>.md`. `config.json` records format version 1 and
the project default; each target's `manifest.json` records the target, template
version and mandatory standard keys. Architecture choices belong in Markdown.

Creation preserves every existing Markdown file, including empty files. It
repairs missing defaults without applying SDK updates to edited local profiles.
A changed technical manifest is a conflict requiring deliberate migration.
Paths reject traversal and symlinks. New files use atomic link publication without replacement, including when an editor creates a file after preflight. Creation validates templates and conflicts,
uses a shared profile lock, atomically publishes each file, and publishes the
configuration last. An interrupted creation can be repeated. Dry-run does not
create locks or directories.

`--agents` maintains only the block between `<!-- dctx0 profiles begin -->` and
`<!-- dctx0 profiles end -->` in application `AGENTS.md`. Ambiguous or unmatched
markers are errors. Existing instructions outside the block remain intact.

Standard roles cover scalar, value, type, entity, message, event, failure,
operation, port, surface, module, external, example, invariant, lifecycle, proof,
test and review. Additional files may use any existing `StatementKind` keyword,
for example `use/intent.md`. Additional keys are optional. A present empty file
means no additional guidance. Missing actually required standard files or
`common.md` are shown by their exact relative path and block new claims.

Existing project conventions take precedence over defaults. Defaults suggest
TypeScript branded domain primitives, Java immutable value objects, named Go
types, and Rust newtypes and traits. Each domain representation and checked
construction path is defined once and reused. Static distinctions do not replace
runtime validation; casts, decoding and numeric conversions must preserve the
contract. They impose no DI or UUID library. Contract tasks publish contracts;
intents explain decisions without adding domain requirements.

Language-wide representation, validation and delivery rules live in `common.md`
and appear once in a task's selected bundle. Keyword profiles describe the
specific assignment: `implement` owns the assigned contract or behavior; `use`
consumes an existing contract without rebuilding it. They cover actual operation
execution, transport boundaries, mock versus production-adapter evidence,
isolated examples and lifecycle sessions, and source-bound reviews separately.
Project profiles may override style choices such as classes, Hono or Awilix.

The revised bundled defaults apply when creating missing profile files. Existing
project Markdown is preserved: review and edit those local files explicitly to
adopt the new guidance. A changed embedded default does not silently update
previously created profiles or frozen task execution bundles.

Optional [Gin and Axum presets](http-frameworks.md) add concrete HTTP composition and testing guidance. Merge them into local Markdown deliberately; they neither activate a harness nor add a framework CLI selector.

## Inspect and claim

```sh
dctx0 tasks show ./dctx0 T42 --project . --target rust
dctx0 tasks next ./dctx0
dctx0 tasks context ./dctx0 T42 --section implementation --format json
dctx0 tasks claim-next ./dctx0 --actor worker --target rust
```

Without `--project`, the engine searches upward from the spec directory for the
nearest `.dctx0/profiles/config.json`. Without configuration, existing task
behavior is preserved. An explicit target must be configured. There is still
one task history per spec project, shared across targets.

Selection uses the pinned snapshot, subjects, required type closure, facts, and
actual calls. Own subjects get implement profiles; required types, called
operations, relevant facts and port/surface boundaries get use profiles.
Grouping is deterministic and lists the associated subjects. Namespace parents
do not create extra implementation mandates. `common.md` appears once.

Profile text participates in normal context budgeting before pagination. The
implementation section renders as Markdown alongside separately fenced spec
excerpts. JSON includes path, target, role/key, subjects and content hash on
profile items; the context header carries the bundle hash and missing paths.
Context provenance distinguishes current local guidance, stored execution bundles, immutable requested revisions and unknown legacy execution. Unloaded required sections and oversized items remain explicit. Cursors include
presentation version 3 and the used bundle hash. Changes to unrelated profiles
do not invalidate a task cursor; changes to used unbound profiles do.

Task command envelopes are report schema 2. Language IR and snapshot goldens
are unchanged, and profile editing alone changes neither task requirements nor
waves. The compatible `dctx0_tasks::context` API still accepts no profiles;
`context_with_profiles` adds pure selection without filesystem access.

## Frozen execution context

SQLite writer migration 3 adds immutable execution bundles, event-linked claim
and reservation bindings, and profile update receipts. Readers still accept
schema 1 and 2 without migration. Existing task state JSON has optional defaults;
old records are preserved.

Task selection, claim creation, bundle persistence and slice reservation happen
in one immediate SQLite transaction. Slices pin all newly reserved tasks,
including later-wave work; reservation does not start it. Existing claims retain
their bundles. Start uses the reserved bundle. Release clears the current binding
so a new attempt can use current profiles; earlier event states retain their
texts. Completed, running and reserved tasks show saved texts even after local
files are deleted. Missing current files make drift comparison unavailable;
they do not erase historical context.

PASS continues to describe executed checks against pinned requirements and the
tested implementation. Binding free-form architecture guidance never asserts
that it has been tested.

## Selective adaptations

Profile edits create no tasks and change no work/check state.

```sh
dctx0 tasks profile-changes ./dctx0
dctx0 tasks profile-changes ./dctx0 --task T42 --task T51
dctx0 tasks profile-changes ./dctx0 --domain MovieNight --semantic entity
dctx0 tasks profile-changes ./dctx0 --all-affected --format json > update.json
dctx0 tasks profile-update ./dctx0 --file update.json \
  --reason "Adopt the revised representation"
```

Task IDs, domain/semantic filters and `--all-affected` are alternative selection
methods. Without a selection the preview creates nothing. Only selected Done
tasks with changed bundles (or explicitly adopted unknown legacy context) are
replaced. Open work can read current guidance without a new ID. Running and
reserved contexts remain frozen. `--all-affected` excludes legacy tasks whose
starting context is unknown, and tasks whose required current files are missing.

The preview lists old/new target and bundle assignments, changed used files,
selected tasks, necessary integration/review revisions and possible impacts.
It follows actual prerequisite paths, including unchanged consumers; other
implementation tasks are informational and are never automatically recreated.

Applying checks the pinned plan/event sequence, the observed valid check occurrence, selection and relevant bundle
hashes against freshly loaded profiles, then publishes transactionally. New
work is Open/Unverified with a new ID, unchanged requirement/spec hashes and
history links to old results. Its desired profile revision is immutable, so
later edits cannot change the requested adaptation. Replaying the same request
and reason returns the recorded plan. Replanning preserves typed adaptations;
legacy FAIL/review follow-ups keep their text-prefix read fallback. Historical
Done/PASS records are never rewritten.

## Validation and measurements

Use `python3 scripts/validate.py --coverage`. It pairs tests and coverage with
release speed and peak RSS measurements. The runner measures 34 existing
workloads plus 40 profile workloads: create, load, Markdown context, JSON
context and claim binding for every target and both complete example specs.
Claim timings include release for reproducible repeated attempts. Profile
creation uses a fresh per-process application root. RSS is Linux post-exec
`VmHWM` including setup and SQLite for task phases, rather than incremental allocations. Create/load phases measure profile files without preparing the compiler or SQLite. Source changes during a measurement make the run incomplete.

Provenance hashes include target crates and embedded Markdown. New workloads
are explicitly unassessed against the accepted 34-workload baseline. Existing
comparable workloads still use the independent speed/RAM gates. The baseline
is not automatically changed; its explicit promotion command is documented in
README. History retains the latest 20 runs, including incomplete failures.
