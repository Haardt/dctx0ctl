# Home Cinema DCTX0

The Rust consumer acceptance exercise is documented in
[cinema-acceptance.md](../../docs/site/src/docs/workflow.md). Run it from the repository
root with `PATH=/home/held/.cargo/bin:$PATH python3 scripts/cinema_acceptance.py`.
It uses a fresh Cinema copy, records speed/RAM and exercises editable profiles,
task claims, native checks and selective adaptations. The small `src/domain`
implementation is the tested ReleaseYear contract; the copied project does not
yet contain the complete application described by the historical guide below.

A second bounded exercise covers exactly FilmId, the VotingRules contract and
the pure candidateOrderWinner operation. See
[cinema-chain.md](../../docs/site/src/docs/workflow.md), or run
`PATH=/home/held/.cargo/bin:$PATH python3 scripts/cinema_chain_acceptance.py`
from the repository root. It leaves all other tasks, including full module
integration, Open.

This reference application uses the native verification configuration in `.dctx0/testing/verification.json` and the application-test recipe in `.dctx0/application-tests/native-run.json`. The latter generates a typed facade from the prepared plan and connects it to the actual `CinemaSession` and Hono routes. The retained `dctx0.bindings.ts` and `dctx0.hono.bindings.ts` are historical, unvalidated plugin-binding drafts: their referenced source paths do not describe this implementation and are not used by those native workflows. They do not establish adapter or HTTP conformance.

The DCTX0 contract is organized as one logical `HomeCinema` workspace across two files:

- `dctx0/workspace.dctx0` declares the workspace.
- `dctx0/movie-night.dctx0` contains the domain model, modules, contracts, examples, properties, and lifecycle.

The one-active-vote rule is an operational `ScreeningPlanningApi.castVote` /
`VotingStore.cast` guarantee: a later successful vote by the same member in
the same poll replaces the earlier vote. It deliberately does not claim that
an arbitrary input `List<FilmVote>` is already duplicate-free. The Cinema
regression test exercises the real `createApplication` composition and an
intentionally broken vote-store keying to keep that wiring meaningful.

The release policy requires mechanically checked ownership to pass. Static
TypeScript effect verification must not report a known violation, but may stay
`UNKNOWN`: calls and events are checked statically, while arbitrary adapter
`read`/`write` behavior cannot honestly be proved from TypeScript source alone.

Dependencies are already described by `package.json` and `bun.lock`. Install them from this directory when `node_modules` is absent:

```sh
bun install --frozen-lockfile
```

Start Runatron/JAM with this directory selected and begin with:

```text
/dctx0 check
/dctx0 build
/dctx0 generate
/dctx0 task
```

To regenerate or evolve the implementation task-by-task, run:

```text
/workflow dctx0-implementation
```

The workflow pauses after generation so the implementation plan can be inspected before source files are created. Complete verification then uses `/dctx0 verify`, `/dctx0 test`, `/dctx0 review`, and `/dctx0 validate`.

## Application-owned Bun tests

The current native suite and generated typed production connections live in
[.dctx0/application-tests](.dctx0/application-tests/README.md). CinemaSuite covers
the original 5,766 finite-plan cases, including real SQLite/HTTP, scenarios and
lifecycle paths. Production imports no testing library. Existing adapters and
configuration remain compatible.

From the repository root, the isolated native diagnostic runner is:

```sh
python3 examples/cinema/scripts/application-acceptance.py --cli target/debug/dctx0 --form all
```

For task delivery, use the prepared native plan and engine-owned recipe:

```sh
dctx0 verify application-tests prepare examples/cinema/dctx0 \
  --project examples/cinema --plan .dctx0/application-tests/plan.json \
  --group cinema-native --test-name CinemaSuite --form all \
  --boundary value-constructor --group-seconds 300
dctx0 verify application-tests complete examples/cinema/dctx0 \
  --project examples/cinema --plan .dctx0/application-tests/plan.json \
  --group cinema-native --recipe .dctx0/application-tests/native-run.json \
  --report .dctx0ctl/application-tests/complete-native.json
```

The complete workflow retains source-bound authored inspection notes from
`.dctx0/testing/typescript/inspection-notes.json` (override with `--notes`). It
creates missing profiles/tasks from the prepared snapshot and uses the shared
transactional task guards. The expected complete Cinema state is 105 Done tasks:
all 105 with testing PASS on the fresh authored-complete path, including four
separately reported source-bound review tasks. Review approval/manual completion
alone does not set PASS; explicit native-backed inspection attestation does.
Existing Done/Unverified reviews are preserved rather than silently upgraded.

A plain `verify application-tests run` attempts eligible task delivery by default;
add `--no-adopt` for diagnosis. A diagnostic, replayed, imported or merely saved
PASS does not independently authorize delivery. Native captures, frozen builds,
collector/central completion, current task/profile pins and review/claim guards
remain required. Full repository validation with coverage and release speed/RSS
is a separate release gate and is not claimed successful by these instructions.
See the [application-test workflow](../../docs/site/src/docs/application-tests.md).
