# DCTX0 workflow improvements

## 2026-10-29: release validation cadence

State: proposed. Classification: repository workflow; no acceptance exception.

Observation: local 0.3.1 acceptance passed all 32 stages, 673 regular all-feature
Rust executions, 674 instrumented executions and 88 release workloads. The coupled
run took 3,811,327.730 ms, with sampled process-tree peak RSS of 1,054,556,160
bytes. Merged source lines and LLVM functions both exceed the unchanged 97%
thresholds. Performance comparison is unassessed; the accepted baseline is
unchanged. [The receipt](docs/release-v0.3.1-validation.json) retains source pins,
measurement methods, samples and prior incomplete attempts.

The public [release run](https://github.com/Haardt/dctx0ctl/actions/runs/40873631934)
separately requires five native platform jobs and the shared validation workflow.
Its default storage-acceptance stage runs broad engine `verification_run` tests;
the later full workspace/coverage stage includes that integration target again.
This is scope overlap, not proof of identical inputs or interchangeable evidence:
features, build profiles, instrumentation and prerequisite preparation can differ.

Hypothesis: broad overlap and unoptimized QA contribute to delivery latency.
No causal time/token split has been measured. Local and hosted measurements are
not a controlled comparison; no percentage improvement is established.

Proposed trial for a separately reviewed workflow change:

- Evaluate focused storage checks plus the existing full workspace gate. Preserve
  the UI checks and every other obligation currently supplied by the broad stage.
  Compare exact covered targets/features before removing a repeated invocation.
- Compare the documented local QA optimization level 1 with hosted QA, retaining
  debug assertions, overflow checks, full coverage and the separate uninstrumented
  release performance profile. Measure on comparable hosts with explicit caches.
- Keep five-platform native verification, mandatory cases, source-bound reviews,
  tutorial acceptance, 97% gates and intentional-only baseline promotion intact.
- Measure first usable behavior, complete accepted delivery, justified/avoidable
  repeats and check wall time separately. Missing usage and attribution stay
  unmeasured; CI waiting time is not active agent work or a billing estimate.

Trial result: pending. This record changes no CI command, task, scope or evidence
authority and is not permission to skip checks.

## 2026-10-29: native verifier image size

State: retained. Classification: confirmed build configuration repair.

The unoptimized debug-symbol CLI grew to about 271 MiB and exceeded the existing
256 MiB verifier identity capture limit. Native cases passed, but complete evidence
correctly remained unavailable. Omitting QA debug symbols reduced the image to
about 78 MiB and the three focused completion/guard tests passed. Release binaries,
capture limits and acceptance requirements were unchanged. All five native
platform jobs subsequently passed; see the
[platform receipts](benchmarks/artifacts/release-v0.3.1/platform-jobs.json).

The paired diagnostics include compilation with different cache conditions;
their wall times and RSS do not establish a controlled performance improvement.

## 2026-10-29: ownership of build outputs during feature tests

State: retained. Classification: confirmed local orchestration repair.

A focused all-features Cargo diagnostic ran while the feature matrix was using
the shared `target/debug/dctx0` executable. Cargo serialized compilation, but the
other integration tests were already running and could observe the replaced
binary with different enabled features. That local matrix was invalidated; see
the [incomplete checkpoint](benchmarks/artifacts/release-v0.3.1/local-checkpoint.json).
The final full validation ran serially and passed.

Serialize incompatible builds that share an executable under test, or give them
separate target directories. Coordinate generated artifacts as well as source
files. This does not restrict independent dctx0 commands sharing SQLite; their
existing transaction/claim rules remain applicable. No test failure from the
invalidated run is treated as acceptance or as a product regression.

## 2026-10-31 — Configurable verification capture budgets

A consuming project requested a larger capture file limit because regenerable
build outputs of the application (an 18,467,869-byte runner binary) exceeded the
hardcoded 16 MiB per-file limit and had to be moved out of the capture root.

`VerificationConfiguration` now accepts an optional strict
`capture: { max_file_bytes, max_tree_bytes }` block. Defaults keep the
historical 16 MiB per-file and 512 MiB tree limits; each bound is validated
non-zero and inclusive up to 1 TiB, and `schema_version` is unchanged, so
configurations without the block behave exactly as before. The budgets flow into
`Capture::new_with_budget`/`Capture::inventory_with_budget` and are used by the
run capture, the captured/application freshness rechecks, adoption preview,
task attestation, program inspection, review templates and stored review notes,
so stored and current inventories are always compared under identical limits.
Oversized files are rejected from their metadata before reading; the existing
error message strings are unchanged. The CLI self-verification keeps its own
fixed limits, and old binaries reject the new key because the configuration is
strict (install CLI and pinned toolchains together).

Validation performed: `cargo check --workspace` clean; full `dctx0-engine` lib
suite 84/84 (process tests need `DCTX0_TEST_PYTHON`); `verification_go`
integration test green with the target Go toolchain on PATH; consuming-project
acceptance (116/116 native cases with the oversized binary present, adoption
inventory recomputed without budget errors). The repository's full
self-verification and its dated capability/coverage status pages were not
refreshed by this change and remain deferred to an explicit request.
