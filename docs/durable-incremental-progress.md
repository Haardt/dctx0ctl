# Durable incremental verification: implementation checkpoint

This is a partial implementation of the supplied four-package plan. None of
packages A–D is being declared fully accepted. Historical evidence is unchanged.

Implemented in this working tree:

- Shared `--reuse=auto|off|require`, `--reuse-from` and `--reuse-inputs` options
  on native, application-test, hybrid and complete commands. Auto is the default;
  `--no-adopt` remains independent. Automatic lookup chooses the latest same-scope
  native observation rather than an older passing run.
- Automatic available-input receipts. The standard additional declaration is
  `.dctx0/testing/reuse-inputs.json`. Absent declarations do not fabricate
  deterministic scopes or external isolation.
- Explicit controlled child environments, shared by builds, tests, revisions
  and compiler inspection, with effective PATH resolution. Existing configuration
  continues to inherit. Environment values are hashed in execution evidence.
  Controlled Linux worker identity still checks the actual procfs image and
  initial environment; it does not substitute a launcher claim for observation.
- Version-2 input receipts preserve missing runtime, snapshot and revision
  observations as typed gaps. They cannot authorize reuse or task delivery.
  End-of-run drift retains the actual attempt as incomplete, discards copied
  historical cases and preserves the old immutable evidence.
- Machine-readable unit decisions with changed file/variable names. An explicit
  old basis contradicted by a later bound FAIL/UNKNOWN observation is rejected,
  including inside the evidence publication transaction.
  Contradiction diagnostics name a parent-cohort cause once per unit, avoiding
  the product of unit and contradictory-case counts.
- Schema-7 retention preserves the widest complete native case baseline per
  target/scope, and status shows it separately from the latest partial attempt.
  This field is historical coverage, not a claim of current effective PASS.
- The missing `(body_hash,run_id)` retention lookup index is created for new,
  converted and writable existing stores. Read-only opens do not migrate.
  Regressions check both the covering access plan and SQLite VM-step scaling.
- Receipt-only diagnostic adapter cohorts remain incomplete, cannot publish
  reuse proofs and cannot deliver tasks. Imported receipt-only copies remain
  untrusted, so the existing replay reader can diagnose corrupt historical data.
- Hybrid text distinguishes fresh from incremental execution using actual reused
  counts. A regression covers both labels. Explicit `environment: null` keeps
  inherited execution, matching the typed configuration reader.

The cross-process CLI regression uses actual production callback counters:
changing `CODEX_TOOL_CALL_ID` in controlled mode does not repeat callbacks;
changing a forwarded variable does; `require` stops before callbacks when no
compatible unit exists; `off` executes callbacks freshly.

Still required before the supplied objective is complete:

1. A fully warm run must skip build, registration and native test processes.
   Current paths can still execute those processes while skipping production
   cases. Initial application attempts also need complete provisional decisions.
2. Introduce versioned per-unit semantic, concrete-case and dependency bindings.
   Whole-program/full-capture identity and conservative `other-cases` remain.
   Independent spec additions therefore still invalidate unchanged evidence.
3. Resolve target-supported package/module closures and dynamic/external bounds;
   distinguish source/build inputs, execution artifacts and pure outputs.
4. Add indexed unit candidate lookup, validated multiple origins and durable
   in-use references. The widest-baseline retention rule does not replace these.
   Contradiction handling is currently conservative at the parent cohort boundary.
5. Reconstruct old receipts only from retained historical artifacts, with explicit
   per-unit gaps; implement idempotent migration and guarded review reuse. No
   historical result has been reset, rewritten or given a blanket PASS here.
6. Use the same pure decision component for static/live incremental planning and
   execution. Static status/preflight must remain free of provider/revision calls.
7. Complete final Cinema/legacy/hybrid, coverage, scaling and release acceptance
   on the final source, then install the accepted system CLI and reconcile
   Runatron through its normal guards. No Runatron wrapper or direct task-state
   change was introduced.

Runatron was inspected read-only with the installed system CLI. Its latest run
`8a72520c160267f78fa5714563faee003582af30a4cc9a28a09cc3147b0c2a67`
records 1,805 PASS cases in a 37,945-case inventory, 36,140 missing in that attempt,
74 open obligations and stale task-plan/source pins. Those missing cases are not
new implementation work. The installed CLI, historical results, current task
states, source reviews, and Runner configuration were not changed by this work.
The historical manual Hallo demonstration is not substituted for automated
runtime acceptance. Existing Go coverage/Podman gates remain independent.

The read-only history inspection also found that the 2026-11-12 full PASS is
stored as imported run
`ae4c1577afd7cc9bcca9ac9f8ccb359e0e9d0cba62e889c8528ae4994621baa9`.
The preceding native run
`86f660b7610e5a373979e47fe1a550900bf722c4334b085e6fe1e2be8410cec0`
contains all 35,820 PASS cases but has overall UNKNOWN from manual requirements.
Migration must validate the native observation and separately bind any source
inspection/review completion; the imported PASS flag is not native authority.
The installed CLI's read-only `verify compare` between that native run and the
latest attempt reports 2,126 added and one removed case ID, with
`compatible_requirements=false` and `newly_executed_after=false`. This confirms
the inventory delta, not semantic compatibility of the 35,819 shared IDs.
Static `verify preflight ... --diagnostic` still projects all 37,945 cases. It
reports stale/missing task pins, no deliverable pinned tasks and
`EXAMPLE_BINDINGS_REQUIRED` for the `RuntimeTransport` typed hello wire-roundtrip
example's concrete `none` input. This read did not execute adapters, revise
fixtures, publish a task plan or establish either outstanding binding as fixed.

Validation is recorded in [durable-incremental-validation.json](durable-incremental-validation.json),
with compressed raw reports/logs whose original bytes are hash-bound. The final
focused command passes all 19 selected steps, 107 Rust test functions and 98
Python tests on unchanged source hash
`98b4f4de0d0b47dd33907d045019aaf25332027b01c03d4aae59e7f2a2611b5c`.
Five reduced-sample release workloads complete; the performance gate is
unassessed because the baseline is incompatible. No baseline was promoted.

The earlier all-feature workspace attempt recorded 892 passed and two failed
Rust test functions before the broader run was interrupted. Its hybrid-label
regression and live installed-skill preview assumption were then corrected and
passed in the focused run. The preview regression now checks that installed
files remain byte-identical. Coverage and final complete acceptance have not
been established for the final source. Focused validation is development
evidence, not full coverage, full release acceptance or Runatron acceptance.

## Runatron reconciliation repairs, 2026-11-14

Actual Runatron execution exposed two additional engine defects. Ordinary
captured inputs lost executable permissions, so the captured, hash-valid Git
LFS binary could not execute. Capture now preserves source permissions for
ordinary inputs as well as helpers. Four capture tests pass, including execution
of a copied executable outside the helper paths.

Hybrid planning also rejected a project-wide deterministic declaration even
with `--reuse=off`. The adapter-ownership restriction now applies when reuse is
enabled. Fresh execution still captures inputs and executes both backends. All
five hybrid regression tests pass; the added regression performs two fresh runs
and checks actual production callback counters in both backends. Engine clippy
with all targets/features passes with warnings denied.

The rebuilt local CLI is version 0.3.1, SHA-256
`daf1267b1fd6c0c0def99f472279d646d61d1a558c6be182d2dadbf2ccd3e437`.
Previous installed binaries are preserved in the user's local install backups.
These focused repairs do not establish complete tool release acceptance or
complete incremental unit/dependency support.

Runatron Identity now has 3,306 actual native passing cases, eighteen explicitly
authored source-bound assessments and 75 current Done/PASS tasks. Publication
used `verify inspect` and `verify attest`, with native-origin, complete-corpus,
source/spec/profile/task-pin and transactional dependency guards. Its final
status has no open obligations, stale task evidence or next causes. Historical
runs and the old inspection-notes file were preserved. The convenience
`verify complete` preflight still rejects Open tasks whose manual requirements
have no native-case mapping; the explicit guarded inspection/attestation flow
was used rather than weakening that guard or writing task/database state.

The Runatron runner uses its existing mixed assignment: 1,805 application cases
and 36,140 adapter cases. Repreparation preserves all 37,945 required IDs, exact
backend assignments and original 300/5,400-second group budgets. A pure adapter
diagnostic is not acceptance of this mixed cohort. The full hybrid run and
remaining project gates are tracked separately in Runatron's
`reports/dctx0-reconciliation/progress.json`.

## Source review and fail-closed group reuse, 2026-11-14

The current source review is saved in
`docs/durable-incremental-code-review.json`, bound to the exact reviewed file
hashes. It requests changes for the unimplemented parts of packages A–D; this
is not a blanket approval of the dirty workspace or of Runatron acceptance.
There is no existing dctx0ctl self-task database to update, so no task state was
fabricated.

Two safety findings were repaired. Reuse no longer accepts an unclassified
parent-group failure merely because its code was absent from a four-item
blacklist. Only separate manual/suite inspections and task-occurrence metadata
may remain without invalidating otherwise complete native observations.
Unassigned native verdicts and incomplete execution provenance block reuse.
The engine names the original cause under `PARENT_GROUP_INCOMPLETE`.

The Store's transactional contradiction query now also detects later group
errors without individual failing case rows. A later abort can contain zero
records and still block the entire original cohort. Standalone native failures
and source drift cannot be hidden by explicitly selecting an older green run.
Regression tests retain the immutable parent and reject a forged reuse proof.
The expanded planner regression failed before the repair. Focused final-source
validation and associated release speed/RSS measurements are tracked in the
source-review artifact; full workspace coverage and release acceptance remain
separate requirements.

The same review also reproduced and repaired a baseline-retention defect:
an unclassified overall UNKNOWN with complete passing case rows could replace
the good baseline. The shared status/GC baseline filter now admits only PASS or
explicit manual/suite/task-occurrence-only UNKNOWN verdicts. Regressions check
both rejection of the bad replacement and retention of valid manual-only
baselines. The earlier validation attempts are retained as interrupted, not
accepted final-source evidence.

Final focused validation for these safety repairs completed on unchanged source
`d5dbbf07f16a5d2141751dff9bf68617e8acd4d0d54a27e692d71f60d1f16238`: all 19 selected steps pass, including
111 Rust test functions and 98 Python tests. The five release workloads complete
with speed and process-tree RSS observations. Performance regression assessment
is unassessed because the accepted baseline is incompatible; the baseline is
unchanged. Compressed raw reports/logs and scoped source reviews are retained in
`docs/durable-incremental-code-review.json`. The exact release CLI exercised by
the CLI regressions is installed locally; the prior binary is backed up. See
`docs/durable-incremental-local-install.json`. Full coverage, final release
acceptance and the remaining durable incremental features are still open.
