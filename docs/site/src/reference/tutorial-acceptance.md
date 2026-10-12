# Progressive TypeScript tutorial acceptance

The complete MiniBoard tutorial is accepted against its explicit finite plans.
The retained [completion report](https://github.com/Haardt/dctx0ctl/blob/main/examples/tutorial/validation/complete.json)
connects all sixteen projects, the language inventory, native cases, production
coverage, mutations/replay, browser execution, agent exercises and final workspace
validation. Reference implementations and their English guides are versioned.

## Accepted scope

- Sixteen `examples/tutorial-step-N-*` directories progressively retain and extend
  the specification. Each includes normal TypeScript production code, independent
  tests, strict type checking, frozen dependencies and an external testing adapter.
- All 158 audited language features occur in actual statement/type syntax or
  typed execution plans. There are no missing catalog entries. Comments and
  narrative mentions cannot substitute for executable feature occurrences.
- All 43,490 mandatory native cases across the sixteen projects are recorded
  without case failures or UNKNOWN cases. Effective assessments bind separate
  authored inspections; there are no unfinished/non-PASS active tasks.
- Every production TypeScript file has 100% measured lines and functions. Step 16
  includes actual server startup. The browser report matches the current embedded
  UI script and separately covers 15 real interactions, all seven functions and
  58 executable line starts. No full branch/all-input coverage is asserted.
- Seventeen production/adapter mutations produce real failing counterexamples;
  four review/provenance mutations are rejected. Lifecycle and HTTP saved-input
  replay fail with broken code and pass after repair, retaining setup and sessions.
- Ten agent/reset-helper tests cover all sixteen exercises, previous-step and
  empty-code starts, restoration of requirements/tests, removal of both database
  locations, preservation of guidance and rejection of unsafe reset paths.
- Direct Bun and Hono/Awilix classes-only variants use identical specifications
  and semantic plans, with independent published check occurrences. Each passes
  28 mandatory native cases and 100% measured production lines/functions. Both
  reject a genuinely broken increment through actual HTTP. Scope/lifetime tests
  verify the Awilix composition rather than replacing its production service.

## Workspace gate and measurements

The final coupled command was:

```sh
PATH="$HOME/.cargo/bin:$PATH" CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 \
  python3 scripts/validate.py --coverage --example cinema --tutorial
```

Every recorded validation command exited successfully, including feature
combinations, strict lint/format checks, the sixteen-project series, mutations,
both architecture variants and isolated LLVM coverage. The instrumented phase
executed 451 Rust tests. Merged source lines are 26,149 / 26,784 (97.63%); raw LLVM
functions are 2,091 / 2,150 (97.26%), satisfying the user-approved 97% Rust gate.
Compiler snapshot goldens and the accepted performance baseline remain unchanged.

All validation/release input hashes match before and after execution. The
[retained measurement run](https://github.com/Haardt/dctx0ctl/blob/main/benchmarks/history/20261011T184817.266743Z-423989.json)
contains 58 release speed/RSS workloads; native variant metrics are separately
recorded in [variant evidence](https://github.com/Haardt/dctx0ctl/blob/main/examples/tutorial-variants/validation.json).
Functional checks and coverage precede performance. The performance comparison
is explicitly unassessed because no compatible baseline comparison was available;
no regression-free or baseline-promotion claim is made.

The earlier attempt
[20261011T181102.203230Z-376958.json](https://github.com/Haardt/dctx0ctl/blob/main/benchmarks/history/20261011T181102.203230Z-376958.json)
remains failed: three instrumented SQLite writes failed and function coverage was
below 97%. Functional fixtures now use a runner-owned on-disk cache under
`XDG_CACHE_HOME/dctx0ctl` (default `~/.cache/dctx0ctl`), with cleanup afterward.
This avoids the host's RAM-backed, capacity-limited `/tmp` for large test
databases/captured images. Release benchmark temporary-directory behavior stays
unchanged. The collective 2/3 available-memory limit and no-swap policy remain.

## Reproduction and evidence limits

See the [tutorial guide](https://github.com/Haardt/dctx0ctl/blob/main/examples/tutorial/README.md) for individual runs,
agent exercises, resets, profile edits and local UI startup. Product verification
uses the Rust CLI and Bun; Python is only the repository development runner.

`verify-variants.ts --skip-performance` runs functional acceptance.
`--performance-only` measures afterward and refuses changed source inventories
or a changed Rust binary. The combined runner uses these modes to keep performance
last; CI retains the variant report alongside the main tutorial reports.

PASS covers the mandatory cases of the declared finite scope. Generation is
bounded/deterministic, not an exhaustive proof over unbounded values. Manual
constraints, decisions, questions and architecture guidance remain explicitly
source-bound inspections rather than automatically synthesized assertions.
Native production code remains independent of spec/harness imports. The extra
architecture variants demonstrate a small shared contract, not a second full
sixteen-step series or execution support for other target languages.
