# Learn dctx0 step by step: MiniBoard

This tutorial grows a small TypeScript task board through 16 complete examples. Each directory contains the specification at that stage, actual production code, independent tests and an external verification adapter. Every step retains earlier contracts and introduces a new part of the language. Production code imports neither dctx0 runtime nor specification/testing artifacts.

Each step’s README explains what was added, shows the actual specification extension, identifies its TypeScript implementation and describes the tests. Start at step 1 and compare adjacent directories, or use a complete later step as a reference.

The [complete acceptance report](validation/complete.json) records all sixteen current projects and 158 feature occurrences. The final coupled workspace gate passed, including the user-approved 97% Rust coverage threshold. [Acceptance notes](../../docs/site/src/docs/workflow.md) explain the exact scope, measurements and evidence limits.

## The learning path

| Step | New features | What the new checks establish |
| --- | --- | --- |
| [1: Values](../tutorial-step-1-values/README.md) | Workspace/domain, intent, range, pure operations, pre/postconditions, examples and quantified invariants | Legal counter values, real increment results and monotonicity |
| [2: Refinements](../tutorial-step-2-refinements/README.md) | Scalars, alias chains, length/where, formats and data stableId | Inherited validation, text/identity/date boundaries and collection lengths |
| [3: Structured data](../tutorial-step-3-data/README.md) | Records, variants, options, results and builtin representations | Recursive field contracts, exact numeric values and alternative preservation |
| [4: Expressions](../tutorial-step-4-expressions/README.md) | Arithmetic/Boolean operators, membership, construction and sequential bindings | Actual calculations, precedence, predicates and constructed values |
| [5: Collections](../tutorial-step-5-collections/README.md) | Collection/string functions and exact decimal rounding/division | Order, cardinality, empty cases, signed ties and precision |
| [6: Entities](../tutorial-step-6-entities/README.md) | Single/composite keys, messages, state lists/observes and before-state expressions | Actual stored reads, identity uniqueness and public projections |
| [7: Boundaries](../tutorial-step-7-boundaries/README.md) | Modules, ownership, required ports, surfaces, decisions and constraints | Real reader/consumer composition plus separately reviewed architecture |
| [8: Mocks](../tutorial-step-8-mocks/README.md) | Given scenarios and exact collaborator call traces | Real consumer behavior, actual call arguments/count and asynchronous replies |
| [9: Failures and events](../tutorial-step-9-failures-events/README.md) | Declared failures, conditional outcomes and emitted event payload/count | Success publication, missing-task failure and no event on failure |
| [10: Effects](../tutorial-step-10-effects/README.md) | Read/observe permissions and an explicit clock boundary | Actual scoped resource access, clock sampling and observer cleanup |
| [11: Writes](../tutorial-step-11-state/README.md) | Insert/replace/upsert/remove, keyed after-state and state predicates | Correct stored changes, preserved failures and composite-key isolation |
| [12: Lifecycles](../tutorial-step-12-lifecycle/README.md) | Initial/terminal states, guarded transitions and status/presence paths | Connected real-state transitions and fresh independent sessions |
| [13: External HTTP](../tutorial-step-13-external/README.md) | External service contracts and explicit endpoint configuration | Real client/handler interaction, response validation and cleanup |
| [14: Fragments](../tutorial-step-14-fragments/README.md) | Multi-file composition and qualified references | Cross-file contract inheritance and actual fragment operation results |
| [15: Review and policy](../tutorial-step-15-reviews-policy/README.md) | Review inputs/criteria/questions, named suites and release requirements | Evidence composition and rejection of missing, stale or adverse reviews |
| [16: HTTP and UI](../tutorial-step-16-http-replay/README.md) | Explicit HTTP operations, interactive board, mutation acceptance and replay | Real routes/writes, browser behavior and reproducible counterexamples |

features.json (historical evidence is available in Git history) inventories 158 features across registered statements/context nodes, builtins, operators, functions, formats, expression kinds and state predicates. [check-features.ts](check-features.ts) compares the inventory with the actual Rust language registries and inspects parsed/typed specification nodes. [validation/features.json](validation/features.json) records the first step and source position for each feature. Mentioning a feature in an intent or comment does not count as an executable occurrence.

## Prerequisites and your first run

Build the Rust CLI from the repository root. Bun runs the TypeScript application and tests; each example pins its development dependencies in bun.lock. The retained native acceptance uses Bun 1.4.2. Full native verification currently requires Linux and a reachable systemd user manager for the process memory limit: two thirds of available memory, without swap. This resource limit is not a security sandbox.

```sh
cargo build --release -p dctx0-cli --all-features --locked
cd examples/tutorial-step-1-values
bun install --frozen-lockfile
bun run typecheck
bun run test --coverage
../../target/release/dctx0 verify complete dctx0 --project .
```

`verify complete` publishes a current compiler check, runs the explicitly configured plan and binds valid authored inspections. The Rust verification core decides the result from actual values and observations. A reviewer cannot override native FAIL, and architecture prose is not automatically turned into assertions. Changed code needs a new substantive inspection before its evidence hashes are updated.

These product commands and tutorial helpers do not require Python. The repository’s combined development/coverage/performance runner still uses Python.

## Use an agent to implement a step

From the repository root:

```sh
./examples/tutorial/agent-step.sh 1
./examples/tutorial/agent-step.sh 16 -- codex exec --full-auto
```

The script creates a new temporary exercise directory. It supplies the current step’s complete spec, testing configuration, adapter, fixtures and acceptance tests. Production starts at the previous step; step 1 starts empty. The full reference solution in Git remains untouched. The script omits SQLite history and old inspection approvals.

The printed directory contains AGENTS.md and TASK.md with implementation and verification requirements. The agent must implement real application behavior, run the provided checks and review manual requirements against the new sources. Step 16 also requires the actual server entry point and interactive page. `DCTX0_BIN` selects a different Rust binary. An optional agent command runs in the exercise directory and receives the task as a separate argument.

## Reset code and databases for repeated agent experiments

Use the reset helper to create or reset all sixteen exercises in a stable location:

```sh
# From the repository root:
bun examples/tutorial/reset-agent.ts all
# Reset only step 8 back to the previous step's production code:
bun examples/tutorial/reset-agent.ts 8
# Start every step with no production solution at all:
bun examples/tutorial/reset-agent.ts all --empty
# Reset an existing temporary exercise created by agent-step.sh:
bun examples/tutorial/reset-agent.ts 8 --directory /tmp/dctx0-tutorial-step-8-YOUR-ID
```

By default, exercises live under `target/tutorial-exercises/step-N`, outside the versioned reference examples. For `all`, `--directory` selects a collection directory; for an individual step, it selects that exercise directory. Only marked exercises matching the selected step can be replaced. The script refuses reference examples, unmarked existing directories and symlinked exercise roots. Stop any agent/application using an exercise before resetting it.

Reset replaces production code with the clean previous-step baseline, restores the current spec/tests/adapter and removes the local databases in both `.dctx0ctl/` and `dctx0/.dctx0ctl/`. This clears snapshots, tasks, claims and saved verification history for the exercise. It also removes stale inspection approvals, generated transport files and dependencies; run `bun install --frozen-lockfile` afterward. `--empty` removes inherited production code too, while retaining the current acceptance tests. The Git references and their databases are untouched.

Existing exercise `AGENTS.md` and `.dctx0/profiles/` edits are preserved so you can repeat an agent experiment with the same architecture guidance. Specifications and acceptance tests are restored from the reference. Changes to TASK.md or application files are discarded. The newly generated TASK.md records whether production starts empty. If you want to discard guidance edits too, create a new exercise with `agent-step.sh` rather than resetting the old one.

## Agent guidance and editable profiles

`AGENTS.md` tells a compatible coding agent how to work: which architecture to follow, which checks to run and which files are requirements rather than generated output. The exercise script creates a local file with these instructions. Tools differ in automatic discovery; explicitly tell your agent to read it and the named profiles if it does not load AGENTS.md itself. Repository/parent instructions may also apply when an exercise is placed inside the checkout. A default temporary exercise lives outside it and receives its own instructions.

These instructions influence the agent's implementation choices. They do not change the dctx0 spec, execute tests, alter task state or automatically establish PASS. A request to use classes, Hono or a DI container is implementation guidance unless an explicit test or reviewed requirement enforces it. The classes variant includes both source inspection and concrete architecture checks.

Profiles provide reusable target-specific guidance. From an application or exercise directory:

```sh
# Use the absolute path to your built Rust CLI in a temporary exercise:
/absolute/path/to/dctx0ctl/target/release/dctx0 profile typescript create --project . --agents
```

The command creates missing defaults and preserves existing Markdown edits. `--agents` adds or updates only the managed profile-reference block in AGENTS.md, leaving other instructions intact. Edit these files as ordinary Markdown:

- `.dctx0/profiles/typescript/common.md`: shared architecture, libraries and representation choices.
- `.dctx0/profiles/typescript/implement/operation.md`: guidance for implementing operations; similarly use surface.md, entity.md and other semantic roles.
- `.dctx0/profiles/typescript/use/operation.md`: guidance when consuming an existing operation; other use files cover required contracts.

For example, a Hono/Awilix class-based profile can say:

```markdown
Use Hono for HTTP and Awilix with strict CLASSIC injection.
Register dependencies before resolving them. Use one scope per request and
dispose scopes in finally. Put production behavior in classes and methods.
Do not use standalone functions, function expressions or arrow functions.
Use bound methods for callbacks. Keep integers exact as bigint.
```

Align the relevant implement/use files as well as common.md so an inherited preference for ordinary functions does not conflict with your choice. Keep behavioral contracts intact. Existing adapters/tests may need explicit, reviewable changes when public implementation entry points change; editing a profile does not transform code automatically.

Profile text appears in selected task context and is frozen when work is claimed/reserved. Editing profiles alone changes no task's work/check status and does not rewrite historical results. Existing claims retain their captured guidance. The [profile guide](../../docs/site/src/docs/profiles.md) describes selective adaptations for completed work and the relevant commands.

The [two complete architecture variants](../tutorial-variants/README.md) show these choices against the same small spec: direct Bun and Hono with Awilix using classes. Their AGENTS.md and editable profiles are included so you can compare actual instructions with actual production code.

## Verify the series

From the repository root, after building the CLI:

```sh
bun examples/tutorial/verify-series.ts
bun examples/tutorial/mutation-acceptance.ts
```

The series runner installs frozen dependencies, checks TypeScript, runs independent tests, requires 100% measured production line/function coverage per file and executes every configured mandatory native case. It checks the saved evidence and active task results, retaining source hashes and actual worker peak RSS. A selected run such as `bun examples/tutorial/verify-series.ts 12 16` establishes only those steps, not the entire series.

The mutation runner works in isolated temporary copies. It introduces meaningful production defects, an adapter with a canned answer, incorrect HTTP insert/replace behavior and invalid review/provenance conditions. Production mutations must yield actual failing cases, rather than merely a technical error. Lifecycle and HTTP replay rerun stored counterexample inputs against broken and repaired implementations, with changed implementation provenance recorded. Reference task progress is preserved.

The retained per-step [validation reports](validation) record 43,490 mandatory native cases across the 16 steps and 100% measured production line/function coverage. [validation/mutations.json](validation/mutations.json) records 21 negative acceptance checks. These records describe their captured sources; they are not a claim that an arbitrary later working tree has already passed. The repository-wide gate is a separate report, and retained failed attempts remain explicitly failed.

The combined development validation runs long functional and coverage checks first, then release performance measurements:

```sh
PATH="$HOME/.cargo/bin:$PATH" python3 scripts/validate.py --coverage --example cinema --tutorial
```

It leaves the accepted benchmark baseline unchanged. Documentation-only edits do not require repeating the entire native/performance suite.

## Open the finished board

```sh
cd examples/tutorial-step-16-http-replay
bun install --frozen-lockfile
bun run start
```

Open <http://127.0.0.1:4173/>. Create tasks with the form, select them to view details and use the checkbox to mark them Done or reopen them. Writes go through the real HTTP routes and existing Board methods. The example stores changes in memory; restarting restores the three seed tasks. `PORT=4180 bun run start` selects another port.

The [step 16 guide](../tutorial-step-16-http-replay/README.md) explains separate Chromium acceptance and actual browser JavaScript coverage. Bun coverage of an HTML-generating module alone cannot establish browser execution.

## Understand the evidence

PASS means every mandatory check in the reported finite plan was executed successfully. Larger input spaces use deterministic bounded generation: seed 42, at most 100 generated cases per obligation and maximum collection size 4. Explicit mandatory examples are retained. This is not a proof over every possible input.

Actual mock interactions establish consumer behavior toward those collaborators; they do not automatically establish a production port adapter. Missing observers, unsupported capabilities, aborted budgets and incomplete mandatory plans prevent a complete proof. Free-text constraints, decisions and reviews retain separate authored evidence.

Measured production line/function coverage is distinct from full branch coverage. Browser coverage uses actual V8 ranges and executable line starts, with raw ranges retained. Intentional language/representation differences are documented in [tutorial-language-semantics.md](../../docs/site/src/docs/workflow.md).
