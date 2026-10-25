# Binding requirements foundation

This checkpoint implements implementation-independent binding planning and reviewed
description models. It is an initial part of the generated-adapter workflow plan;
it does not establish acceptance of that plan.

After an explicit successful `check`, use:

```sh
dctx0 verify bindings plan SPEC --subject QUALIFIED_SUBJECT
dctx0 verify bindings plan SPEC --task T42 --json
dctx0 verify bindings plan SPEC --project APP
```

The optional application configuration supplies scope, fixtures and budgets. Explicit
subject/task selection overrides configured scope. Without an application, explicit
selection is required and no implementation files are read. No check, migration,
reservation, publication or application process is started by planning.

The version-1 requirements report identifies the published snapshot and check
occurrence, selected subjects, operation signatures, production-validator roles,
required entity observers, adapter capabilities, data declarations and issues.
Resolved helper/preparation calls and nested alias, record, list, option, variant
and failure types are retained. Unselected sibling operations are excluded.
Missing bindings can be reported with the original declaration positions.
The report establishes neither native type compatibility nor readiness.

`BindingDescription` in core describes reviewed session factory/cleanup, functions,
session methods, invoke hooks, parameter order and field projections, result and
modeled-failure hooks, production validators with explicit raw-input form, entity
and effect observers, and named native conversion hooks. The engine's
`read_verification_bindings(APP, RELATIVE_PATH)` reads at most 512 KiB, rejects
unknown/duplicate struct keys, duplicate binding subjects, escaping/nonportable
file paths and symlinked configuration files. Native source roots are named
references; registering/capturing those roots remains future work. Description
validation checks structure only. It does not inspect native sources, validate
signatures, assert complete bindings or execute hooks. No JSON code fragments
are executed. Native types must be checked by a future target generator.

The SDK interface version is 2. Transport framing remains independently versioned
at protocol 1; historical packets, canonical run payloads and IDs are unchanged.
Rust session methods and adapter `describe`/`begin` now return `TechnicalError`.
`TechnicalError::adapter(error)` and `TechnicalError::environment(error)` explicitly
attribute testing-infrastructure failures. Conversion from ordinary strings keeps
the cause unclassified. Panics outside the application boundary remain adapter
errors. Rust workers preserve the category and still close sessions after errors;
Go workers now name their default category `unclassified`. TypeScript's existing
explicit error helpers retain the same meaning. Actual application exceptions
continue through the application's observation boundary. Converter errors never
establish production rejection. Saved outcomes are not reclassified.

Development validation is coupled to release speed/RSS with:

```sh
python3 scripts/validate.py --focus bindings --samples 1 --iterations 1
```

This partial focus includes binding behavior tests, real Rust error/cleanup and
mutation checks, SDK/target conformance and CLI/self-verification tests. It still
performs release measurements, including `verification-bindings-plan`. That new
phase times requirements projection from an already prepared plan; process RSS
includes preparation. Its samples are unassessed until an explicitly authorized
baseline promotion. Full acceptance continues to require workspace coverage,
native platform tests and all required end-to-end consumer checks.

The runner uses CI's default dev/test settings without debug symbols and without
incremental compilation; explicit environment overrides are preserved. Runtime
identity limits are unchanged. Recorded build-environment metadata distinguishes
these validation settings from release performance settings. Debug assertions and
overflow checks retain their normal dev/test defaults.

Still outstanding: the target-owned build contract/refactor, standard adapter
generation for all three targets, atomic generated-file publication, configuration
2, preflight/shared preparation, continuations, external source capture, closure
and existing-run completion, schema 8/query view 3, per-case comparison, the CLI
reorganization/correlation, skill migration and the complete reference/Cinema/
Identity acceptance and actual consumer migration. No consumer files or histories
have been migrated by this checkpoint.

## Checkpoint validation

The complete local Linux x86_64 runner passes on the final implementation inputs:
634 regular Rust executions, 635 instrumented executions, 40 Python tests, the
supported CLI feature matrix, qualified Go Race and native Go/Rust Cinema checks.
Each application passes 5,674 cases, detects five actual production mutations and
replays counterexamples. Authored effective review remains separate from native
evidence; the SDK-2 testing delta was self-inspected by its implementing agent.

Merged source coverage is 38,690/39,846 lines (97.10%); raw LLVM functions are
3,178/3,275 (97.04%). Raw LLVM lines are 40,546/42,083 (96.35%) and regions are
60,432/64,648 (93.48%). Thresholds and exclusions are unchanged. Branch coverage
and a new aggregate native-helper coverage percentage are not claimed.

All 82 release speed/RSS workloads complete with one sample, one iteration and
one warmup. Baseline comparisons remain unassessed; no baseline was promoted.
The new pure binding phase takes 0.506891 ms from a prepared program plan, with
23,236,608 bytes process VmHWM including preparation. This is a reduced development
sample, not an end-to-end CLI latency or incremental-allocation measurement.
The entire QA tree peaks at 1,041,502,208 bytes under its independent RSS watchdog.

The full command uses the documented local QA profile; installed Go/Bun/Rust must
be on `PATH`:

```sh
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_PROFILE_TEST_OPT_LEVEL=1 \
CARGO_PROFILE_DEV_DEBUG_ASSERTIONS=true CARGO_PROFILE_TEST_DEBUG_ASSERTIONS=true \
CARGO_PROFILE_DEV_OVERFLOW_CHECKS=true CARGO_PROFILE_TEST_OVERFLOW_CHECKS=true \
python3 scripts/validate.py --coverage --samples 1 --iterations 1
```

[The versioned validation report](../../../verification-bindings-validation.json)
records source/binary hashes, commands, metrics, earlier failed/interrupted
attempts and all outstanding work. The source hashes agree before testing, after
testing and during measurements. The latest 20 measurement records are retained.

Both neighboring histories have consistent read-only backups and a preliminary
81-file inventory. Five captured Identity/Access files changed afterwards,
including the reviewed specification. No original consumer file or history was
installed or overwritten. That inventory cannot authorize a later migration
without the fresh capture and guards required by the full plan.
