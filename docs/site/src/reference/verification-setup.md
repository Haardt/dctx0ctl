# Verification setup and configured planning

`dctx0 verify init <spec-root> --project <app-root> --target typescript`
creates testing-only artifacts. It does not compile or publish a specification,
execute the adapter/application, claim work, record testing PASS, or adopt tasks.
The TypeScript target must be enabled in the CLI build. Java, Go and Rust retain
editable profiles but currently have no registered executable harness.

The application root receives:

- `.dctx0/testing/verification.json`: schema-1 technical configuration, target,
  runtime, adapter/fixture paths, explicit subject/task scope and core budgets.
- `.dctx0/testing/typescript/contract.ts`: project-adapter contract.
- `.dctx0/testing/typescript/worker.ts`: generated bounded protocol worker.
- `.dctx0/testing/typescript/adapter.ts`: explicitly unimplemented scaffold.
- `.dctx0/testing/typescript/examples.json`: initially empty reviewed inputs.

The spec root receives `.dctx0ctl/verification.json`, associating it with the
canonical application root. Lock files coordinate setup; SQLite/compiler/task
history is unchanged. An association with a different application requires an
explicit configuration edit. Paths and duplicate artifacts are checked before
publication. Symlinks, non-directory parents, non-regular leaf files and escaping
paths are rejected. Files are published durably without replacing existing files.
Repeating initialization preserves developer adapter code, fixture data,
configuration and edited artifacts; interrupted setup can be resumed.

The default scope has no subjects or tasks. An empty scope never selects the whole
application. Optional `--subject <qualified-name>` and `--task T31` initialize the
scope of a new configuration. Repeated init preserves an existing scope; edit
`.dctx0/testing/verification.json` explicitly to change it. Configuration stores
positive numeric task IDs, for example `"tasks": [31]`.

`dctx0 verify plan <spec-root>` loads the associated project's configured scope,
budgets and reviewed example inputs. `--project <app-root>` selects that
configuration explicitly. CLI subjects/tasks override configured selectors;
`--fixtures <path>` overrides configured example inputs. Explicit selectors still
work without any setup/configuration. Planning uses a published check occurrence;
stale source files require a successful `check` first. Historical selection stays
read-only and does not authorize adoption.

The scaffold's real Bun startup is tested: beginning a case reports missing
implementation, and no SQLite task history is created. Setup output contains
`executed: false`; it never certifies preserved developer code as functional.
The adapter must call actual application code, translate its representations and
provide real observers or declared collaborator mocks. It must not reconstruct
business results from examples.

The release runner adds `verification-init` workloads for both mandatory examples.
Each repetition prepares an owned temporary copy of the complete source workspace,
creates a fresh application testing area and removes owned temporary resources.
Elapsed time includes this preparation/setup/cleanup; RSS is the standalone
release process's post-exec VmHWM, including earlier compilation preparation.
Native scaffold initialization is measured separately through real Bun metrics.
New workloads are unassessed until an explicitly authorized baseline promotion.

The full runner remains `PATH=/home/held/.cargo/bin:$PATH python3 scripts/validate.py --coverage`.
Development runs may use `--samples 1 --iterations 1`; their reduced measurements
must not be presented as a comparable accepted-baseline result. Run/show/replay,
immutable verification history, whole-plan execution and automatic task adoption
remain required work beyond setup/planning.


Validation checkpoint (2026-10-10): 224 Rust tests, eight Python tests, four Bun
transport tests and 36 native Cinema tests pass. Merged Rust coverage is
15,619/15,619 lines; LLVM functions are 1,313/1,313. All 86 release speed/RSS
workloads complete with reduced, explicitly unassessed development settings.
Retained evidence (historical evidence is available in Git history) keeps raw LLVM metrics,
full measurement records, actual Bun scaffold failure and prior incomplete runs.
The filesystem publication rejection test uses an unprivileged POSIX user so
read-only directory permissions genuinely reject a write; the Linux CI runner
uses such a user. No coverage exclusion, threshold change or baseline promotion
was introduced. This checkpoint does not complete native run/history/adoption.
