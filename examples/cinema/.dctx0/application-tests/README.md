# Cinema application-owned Bun tests

`cinema.test.ts` registers the native `CinemaSuite` parent. The generated typed
facade calls all 26 production operations directly; authored fixtures use actual
SQLite persistence, HTTP handlers, state/call/event/resource observations and
scoped central collaborators. Constructors, operations, state changes, assertions,
scenarios, lifecycle paths and centrally derived identity cover the unchanged
5,766-case manifest. Existing adapters and configuration remain supported.
Production imports none of this testing-only package.

Prepare and complete from the Cinema directory:

```sh
dctx0 verify application-tests library --project . --target typescript
dctx0 verify application-tests prepare dctx0 --project . --plan .dctx0/application-tests/plan.json --group cinema-native --test-name CinemaSuite --form all --boundary value-constructor --group-seconds 300
dctx0 verify application-tests complete dctx0 --project . --plan .dctx0/application-tests/plan.json --group cinema-native --recipe .dctx0/application-tests/native-run.json --report .dctx0ctl/application-tests/receipt.json
```

`complete` uses current source-bound authored notes from
`.dctx0/testing/typescript/inspection-notes.json` or `--notes PATH`, the prepared
check and current target profiles. Fresh managed `run` attempts transactional
task delivery by default; add `--no-adopt` for diagnostics. Only actual delivery
sets `adoption_authority`. Neither library finalization nor `prepare`, `serve`,
saved/imported diagnostics or replay independently authorizes delivery.
A fresh complete acceptance targets 105 Done/PASS tasks, including four explicit
native-backed authored review attestations. Review approval/manual completion
alone does not set PASS; existing Done/Unverified reviews remain unchanged.
Full current-tree coverage and release acceptance remain pending.

`library-pin.json` binds the local library sources. The repository publisher
`bun run scripts/publish-application-library.ts` reproduces that copy separately
from handwritten tests. The engine captures source, performs frozen/offline
installation and facade generation, and verifies independent native exit/JUnit.
Bun collection uses actual top-level registration calls through a recording
`bun:test` API; it is not an official Bun listing command or per-case enumeration.
Central case coverage is journaled independently. Unsupported registration forms
are rejected.

Actual converted inputs are witnessed before production. Codec/projector errors
are binding errors, never production rejection. `value-constructor` preserves
Cinema's native constructor boundary; the separate decoder API passes raw bytes
unchanged. The 46 non-Option malformed Some(Unit)/None inputs that both map to
undefined are explicitly annotated representation limits, not distinguishable
Option payload evidence. Actual Option targets need tagged representations when
payloads otherwise collapse. See [HANDOVER.md](HANDOVER.md) for evidence and limits.

From the repository root, diagnostics and mutation/replay acceptance are separate:

```sh
python3 examples/cinema/scripts/application-acceptance.py --form all
python3 scripts/application_test_managed_acceptance.py --target typescript --replay --report target/cinema-bun-managed/result.json
```

The runners use isolated application copies and retain elapsed time and
process-tree RSS. Probes include caught contract failures, post-verdict native
assertions, swapped constructor/operation inputs, missing/extra writes, physical
keys, observer scope, cleanup, unexpected mock ports, wrong returns and declared
failures. Replay reruns actual native inputs and remains diagnostic. The immutable
before-reference is under
`benchmarks/application-tests/typescript-before-20261108T172542Z/` in the repository;
no runner promotes the accepted benchmark baseline. See the
[current workflow](../../../../docs/site/src/docs/application-tests.md).
