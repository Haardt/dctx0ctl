# Pinned work packages and delivery

Use a package for a multi-task slice or a handoff that will survive a context
reset. Ordinary small edits do not need an extra orchestration layer.

`dctx0 tasks package <spec-root> T<anchor> --project <app-root> --json` reads
all prerequisites of the anchor and their complete core context in one response.
`body.data.content` holds shared content by hash; each task references its items.
Keep the exported package, its ID and pin. `complete: false` means mandatory
context or implementation profiles need attention. Dependency closure and
ownership are reported separately; exporting a package does not reserve work.
Use existing `claim-slice`/start/complete guards when execution is authorized.

Before editing, identify the actual assignment, task IDs, prerequisite results,
expected artifacts, selected profiles, acceptance and required checks. Files are
implementation choices: inspect only the paths needed for those responsibilities.
Keep a compact working handoff with the package ID, decisions, changed paths,
completed checks and evidence IDs, remaining blockers and the next action. Store
it in the project's established working-artifact location when persistence is
needed; do not put transient execution instructions in reviewed specs.

After resuming, use `tasks package-delta <spec-root> T<anchor> --project
<app-root> --file <previous-package.json> --json`. It reports new/removed tasks,
requirement, prerequisite, profile, state and evidence changes and new content.
Save the delta separately, then reconstruct and validate the next complete package:

```sh
dctx0 tasks package-apply --file previous-package.json --delta delta.json --json > next-package.json
```

Retain `next-package.json` for the next comparison. The command checks both package
identities, pins and content references without opening project history. Do not
replace the prior package with the delta itself. Reload tasks marked
`reload_context` and newly added contents; retain unchanged content by its reference.
Read all required constraints, examples and type contracts before implementation;
shared references avoid retransmission, not requirements.
The delta is a comparison of pinned requirements and selected profiles, not a
working-tree freshness check. A saved package never overrides new claims or
changed requirements. Implementation changes still require relevant reinspection.

When delegation is authorized, give each worker one responsible slice or bounded
assignment, prerequisite/result references, file ownership and acceptance. Request
a single structured handoff: changed artifacts, decisions, actual checks and
source/evidence identity, blockers. Wait for completion events; ask for status
when a dependency, conflict or timeout needs a decision. Preserve useful work
through context resets instead of repeatedly rediscovering the repository.

During repair, run focused checks and replay the actual counterexample. Preserve
the project's mandatory native scope, mutation checks, coverage and performance
workflow at acceptance. Repeat a passed check when relevant sources, dependencies,
adapter/configuration, scope or test inputs change, or a remaining concern calls
its result into question. Exact command repetition alone says nothing about reuse.

For compact diagnosis use `verify show <spec-root> <run-id> --summary --json` or
`--outcome FAIL,UNKNOWN --limit 50 --json`. Follow the pinned cursor with the same
filters and limit. Request `--case <id>` for bindings and observations. Filtered
output retains the whole run assessment; it cannot establish a reduced-scope PASS.

Before ending a package, reconcile implementation, evidence and task state.
Use `verify adoption-preview <spec-root> <run-id> --project <app-root> --json`
to explain native adoption blockers; it never adopts, refreshes or executes.
Existing guarded verification/delivery commands remain the publication path.
For a stale assigned review, use `verify review-changes <spec-root> --task T<id>
--project <app-root> --json`. Inspect changed/added/removed files and historical
findings, then fill the empty current template after substantive review. A source
group covers all its assigned duties; the tool does not infer which individual
duty a changed file affects. Spec drift blocks a current template. This read-only
comparison never renews approval or changes task state. When a renewed task has
no report, it follows only explicit review-renewal ancestry; `previous_task` names
the source of the nearest latest judgement. That report is comparison context,
not an inherited approval.
Deliver authored reviews separately where required. Preserve Done/Unverified if
PASS requirements are not satisfied and name the missing evidence explicitly.
Retain result artifacts and decisions with the ordinary guarded task completion.

For a measurable run, retain the starting plan/event pin, package IDs, relevant
source identities and check/run IDs. At the end use `tasks progress <spec-root>
--from-plan <start-plan> --from-event <start-event> --json`; optionally pin the
end with `--plan` and `--as-of`. Newly Done and newly PASS are different measures;
new work and removed active PASS are separate. Join agent usage externally by
package ID; do not treat current PASS totals or elapsed request time as run yield.
