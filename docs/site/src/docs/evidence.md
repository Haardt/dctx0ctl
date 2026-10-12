# Evidence and replay

A report states what was checked against which requirements and implementation inputs. It is useful only when those boundaries remain visible.

## Outcomes

| Result | Meaning |
| --- | --- |
| PASS | Requirements in the selected mandatory finite plan are satisfied. |
| FAIL | A check found a concrete violation. |
| UNKNOWN | A required statement lacks sufficient evidence. |

Finite generated cases do not prove every possible input. Reports retain the selected scope, seed, budgets, required cases, observations, unsupported obligations, and provenance. A successful compiler check is not native behavior evidence.

## Observe centrally

Target workers report actual returns, visible failures, exceptions, calls, events, resource observations, and before/after states. The Rust core checks contracts and assertions against those observations. A worker's claim that it passed is not an accepted substitute.

Actual state observations must be exposed by the adapter. Missing projections/observers and opaque trace labels remain open. Declared collaborator replies test consumer behavior; real persistence/HTTP checks assess production adapters separately.

## Saved runs

```sh
dctx0 verify run <spec-root> --project <app-root> --no-adopt
dctx0 verify show <spec-root> <run-id> --json
```

Runs and artifacts are immutable. Source hashes, snapshot IDs, check occurrences, plan hashes, implementation inventories, and compiler/worker receipts have different provenance roles. Saved reports describe their pinned inputs and do not assert current working-tree freshness.

## Counterexamples, shrinking, replay

Supported failing operation/lifecycle cases retain concrete inputs and observations. Deterministic shrinking can find a smaller failure within the configured budget; exhausted shrinking is explicit.

```sh
dctx0 verify replay <spec-root> <run-id> --case <case-id> --project <app-root>
dctx0 verify replay <spec-root> <run-id> --case <case-id> --minimized --project <app-root>
```

Replay executes saved inputs against a new capture, records the comparison, and never adopts tasks. The original failure remains in history. A repaired implementation should pass the saved counterexample and the relevant full scope.

## Reviews and effective evidence

Authoritative automatic results remain separate from source-bound authored inspections. A current inspection can address an unresolved manual statement within its exact scope. It cannot override an actual native FAIL or replace missing required execution.

Current planner review assignments and ordinary task reports make these obligations actionable. Review approval records a judgment; task completion records progress; neither alone sets testing PASS. See [agent evidence](agent-evidence.md).

## Complete configured acceptance

`verify complete` checks the current specification, executes the configured plan, revalidates authored inspections/current reviews, and requires delivered work with appropriate evidence. A nonempty task plan is mandatory. Native execution gaps, running/reserved work, failed gates, and stale/rejected reviews block success.

Source-bound review IDs appear separately in `reviewed_tasks`; they may be Done/Unverified. This is distinct from executable tasks that require PASS. Repository coverage and performance checks remain separate contributor acceptance.
