# Configuration and limits

Read the installed command's help and project files. These are the shipped Rust
CLI contracts; an older binary or an explicit library caller can differ.

## Select the correct configuration

- Native verification: `<app-root>/.dctx0/testing/verification.json`. Select the
  application explicitly with `--project <app-root>` or follow the association in
  `<spec-root>/.dctx0ctl/verification.json`; it points to the application and is
  derived state, not another set of execution options.
- `schema_version`, `target`, `runtime`, `adapter`, `fixtures`, `scope`, and
  `budgets` describe the existing native workflow. Preserve their values unless
  changing that aspect is the task. An empty scope does not select everything.
- TypeScript runs Bun; Go and Rust use target-owned native builds. Java currently
  supplies profiles without native execution. Do not switch targets to hide a
  missing toolchain. Prepare dependencies with the actual package manager when
  authorized; managed Go/Rust builds can require already available dependencies.
- Paths to adapters and fixtures are application-relative, confined paths.
  Unknown configuration fields and unsafe paths are rejected. Do not invent
  `--timeout`, `continue_on_timeout`, or an environment override for a JSON field.

## Native deadlines: `execution`

When `execution` is absent, ordinary native worker defaults are:

| Field | Seconds | Meaning |
| --- | ---: | --- |
| `initialization_seconds` | 30 | Worker startup/initialization |
| `case_seconds` | 5 | Entire case, including session begin, validation/invocation and session end |
| `run_seconds` | 120 | Overall native worker deadline; raising a case limit does not raise this |
| `cleanup_seconds` | 5 | Bounded termination/wait/stream cleanup |

If adding the `execution` object, supply all four time fields. Durations must be
positive representable integer seconds. The schema has no zero/unlimited mode.
For example, merge this block into the existing top-level JSON object:

```json
"execution": {
  "initialization_seconds": 60,
  "case_seconds": 30,
  "run_seconds": 900,
  "cleanup_seconds": 5
}
```

These numbers are an example for a measured workload, not settings to apply to
every project. Dedicated self-verification defaults/API-supplied limits may be
larger. Native builds have separate limits: raising `case_seconds` does not give
Cargo, Go or another build command a longer deadline.

A timeout causes UNKNOWN, terminates/reaps the worker and currently aborts the
remaining native execution. Remaining required cases are unexecuted, not PASS.
There is no current configuration switch to continue with a fresh worker. A
higher case limit can avoid an otherwise legitimate slow-case timeout; it does
not implement restart/resumption. Preserve evidence for the timed-out input.

## Message, evidence and value sizes are different

Optional fields inside `execution`:

| Field | Default | Maximum | Scope |
| --- | ---: | ---: | --- |
| `message_bytes` | 1,048,576 | 67,108,864 | One native protocol frame |
| `evidence_bytes` | 4,194,304 | 268,435,456 | Retained evidence for a session/case |

Both require positive integers. Raising them does not raise generated value
limits, report import limits, process RAM caps or time budgets. Native log limits
and evidence-entry limits are internal `NativeLimits` fields, not additional
accepted keys in this project JSON schema.

`budgets.value_bytes` is a separate canonical-value/evaluation bound. A larger
transport frame does not make an oversized generated or returned value valid.

## Corpus and interpreter: `budgets`

The standard `VerificationBudgets` defaults are:

```json
"budgets": {
  "seed": 42,
  "generated_cases": 100,
  "collection_size": 4,
  "recursion_depth": 8,
  "value_bytes": 65536,
  "shrink_steps": 100,
  "evaluation_steps": 100000
}
```

These select finite inputs and bound interpreter/shrinking work, not seconds or
process memory. Prefer existing project budgets; changing seed, generated-case
count or collection size changes the tested corpus and provenance. Do not lower
these to present a timeout as a full-scope PASS.

## Dependency discovery

`dependency_resolution.max_references` defaults to 10,000 and supports inclusive
integer values 1 through 1,000,000. It bounds native literal-reference traversal,
not test cases. Repeated/cached references still consume budget. Incomplete
capture/provenance stays open even if executed behavior passes. This is separate
from process deadlines; inspect the exact `DEPENDENCY_RESOLUTION_BUDGET` issue.

## RAM, profiles and codec settings

Process memory supervision is separate from `execution.evidence_bytes`. Native
CLI optional Linux/systemd supervision and repository QA's sampled process-tree
RSS watchdog have different behavior. Follow the actual command's logs and
project instructions. `DCTX0_MEMORY_GUARD=0` does not change native case deadlines;
do not disable memory supervision as a generic timeout repair.

Editable guidance lives in `<app-root>/.dctx0/profiles/config.json` and the
selected target's Markdown documents. `profile <target> create --project ...`
preserves existing files. Profile text is not executable verification evidence;
claims/reservations freeze selected bundles, and explicit adaptations create new
work rather than rewriting completed history.

A codec generator's `--config` is its own versioned representation configuration,
not a replacement for `verification.json`. Representation choices must expose
actual production acceptance/rejection and cannot reconstruct expected answers.

## Verify an adjustment

Use `verify plan <spec-root> --project <app-root>` for a read-only plan check.
A native run's `application/vnd.dctx0.native-limits+json` artifact records actual
process limits; dependency resolution records its own limits/summary artifacts.
Inspect saved diagnostics with `verify show <spec-root> <run-id> --summary --json`
and the available detail/artifact tools after reading their help. Distinguish a
whole-case deadline from an overall run deadline, transport overflow, missing
observer, resolver budget or interpreter evaluation budget.
