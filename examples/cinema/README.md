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

This is a complete reference project for exercising the dctx0 workflow. It contains the application implementation, V2 core bindings in `dctx0.bindings.ts`, and the separate Hono profile in `dctx0.hono.bindings.ts`. Generated Runatron artifacts remain unversioned.

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
