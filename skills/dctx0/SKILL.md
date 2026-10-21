---
name: dctx0
description: Use the Rust dctx0 CLI to inspect specifications, choose task or adapter work, and interpret verification evidence.
---

# dctx0 workflow

Use the project's installed `dctx0` binary; inspect `dctx0 --help` and the relevant
subcommand help before choosing flags. This is the Rust CLI, not Runatron's hosted
tools or its former `build`, `generate`, `task-check` or `finalize` workflow.

Identify the reviewed spec root and the separate application root. `check <spec-root>`
compiles and publishes a valid snapshot; failed compilation preserves the previous
head. Read-only history, task and verification reports describe pinned inputs,
not automatically the current working tree. Use `--json` for structured reports.

For spec changes use `$dctx0-language`; for application tasks or an external adapter
use `$dctx0-implementation`. Keep the user's actual scope and existing authorization.
Do not invent business requirements or alter a spec merely to pass verification.
For multi-task implementation, follow the implementation skill's pinned package,
context-delta and evidence-to-task closure routine. Save the starting plan/event
pin when measuring progress; current task totals do not measure this run's yield.

`verify plan <spec-root>` projects the selected published requirements without
running production. `verify run` executes the configured finite scope and stores
actual evidence. `--no-adopt` preserves task progress. `verify show` reads a saved
run; `verify replay` executes saved inputs as a new comparison. `verify complete
<spec-root> --project <app-root>` explicitly checks, executes the configured scope,
binds authored inspections and requires finished/PASS active tasks.

PASS applies to the declared mandatory finite plan. UNKNOWN, missing observers,
unsupported capabilities and resource aborts remain open. Free-text constraints,
reviews and undefined suites require explicit source-bound inspection; approval
cannot override an actual FAIL. Task Done and testing PASS are separate.

Product commands require Rust and the selected native toolchain, not Python.
Repository QA scripts are separate. Java currently supplies editable profiles;
TypeScript/Bun, Go and Rust supply executable native targets. Respect project
AGENTS.md and editable profiles without presenting their prose as automatic proof.
