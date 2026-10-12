# Cinema consumer acceptance — 2026-10-09

This exercise uses the actual Rust CLI as an LLM would: inspect assignments,
claim work, implement a small contract from its delivered context, execute native
checks, record evidence, and explicitly request a profile adaptation. Only the
complete Cinema specification is used. runatron-next is not checked, tested or
benchmarked by this exercise.

## Reproduce

```sh
PATH=/home/held/.cargo/bin:$PATH python3 scripts/cinema_acceptance.py
```

Requirements are the workspace Rust toolchain, Python 3, Bun and Linux `/proc`.
The script builds the release CLI, copies Cinema into a fresh directory under
`target/cinema-acceptance/`, installs its locked TypeScript dependencies there,
and uses real CLI commands. It does not modify the reference spec or its existing
SQLite history. The fresh directory retains profiles, SQLite history and the
tested implementation for inspection. Per-command stdout/stderr and the full
report remain under `target/cinema-acceptance/`.

The runner pairs its acceptance checks with release measurements restricted to
Cinema: the 17 established phases and 20 profile phases for the four targets.
It saves the measurement through the existing locked, atomic history writer,
which retains the latest 20 runs. It neither runs the full two-example validation
command nor promotes a baseline.

## Result

All 27 workflow assertions pass. TypeScript strict checking passes, as do all
18 Bun tests with 233 expectations. Tests cover every integer from 1888 through
2100, inclusive bounds, out-of-range integers, fractional/non-finite numbers and
non-number inputs. Intentional mutations of the upper bound and integer check
both fail the native tests, showing that the tests detect those defects.

The implementation is [release-year.ts](https://github.com/Haardt/dctx0ctl/blob/main/examples/cinema/src/domain/release-year.ts),
with [focused tests](https://github.com/Haardt/dctx0ctl/blob/main/examples/cinema/src/domain/release-year.test.ts).
`ReleaseYear` preserves its numeric representation and validates through
`releaseYear(input)`. The alias is deliberately a plain number rather than an
opaque branded type; callers validate untrusted values at the factory boundary.
No DI or UUID dependency was introduced, and no complete operation was added
as part of this contract task.

The workflow verifies:

- Dry-run writes nothing; repeated creation preserves edited Markdown and the
  original AGENTS text outside the generated block.
- FilmId, ReleaseYear, Film and searchFilms expose appropriate assignments,
  complete entity fields and type closure, constraints and profile roles.
- Tiny budgets expose oversized items and continuation; a used-profile edit
  invalidates an unbound cursor.
- Claims remain frozen after profile edits and deletion. Missing required files
  are visible and reject a new claim without changing the task's Open state.
- Release/restart uses current profiles. Slice reservation keeps work Open and
  preserves its stored bundle when subsequently started.
- Completion remains Unverified until an actual native check is imported.
  Done/PASS retains its stored bundle after local files are deleted.
- An unselected drift preview requests no replacements. Explicitly selecting
  completed T53 creates T104 Open/Unverified with unchanged requirements,
  while historical T53 remains Done/PASS.
- Exactly the previewed Catalog/Community integration and review tasks receive
  new IDs. Other implementation and assembly IDs remain unchanged. Repeating
  the update is idempotent, and replanning retains the typed adaptation.

The new adaptation is intentionally left Open, because creation must not imply
completion or PASS.

## LLM assessment

The ReleaseYear context alone supplies the base type, exact range, acceptance
criteria, source provenance and relevant editable guidance needed for this small
implementation. The Film context supplies all fields, its identity key and the
dependent type contracts. The searchFilms context separates requirements,
constraints, examples and implementation guidance; it also makes its unmet
prerequisites explicit. Those blocked tasks were inspected, not completed.

The copied Cinema README describes an existing application, but the copied
project initially contains no `src` tree. There are consequently no existing
source conventions to inspect beyond AGENTS.md and TypeScript configuration.
The supplied context does not mandate artifact paths or export names: those
remain implementation decisions recorded with the delivered result. This
exercise creates the first small application contract; it does not claim a
working Cinema API, interpreted DSL examples, generated artifacts or native
verification by a Rust target.

## Performance evidence

Retained evidence (historical reference is available in Git history) includes assertions,
command timings, sampled memory, native check evidence, input/implementation
hashes and raw release samples. The rotating full measurement is
`benchmarks/history/20261009T210056.150514Z-401668.json`. Its source hash is unchanged
before and after measurement. No baseline or tolerance was changed.

| Cinema release phase | Median ms | Peak RSS MiB |
|---|---:|---:|
| Compilation | 2.700 | 6.98 |
| TypeScript profile creation | 1.671 | 4.78 |
| TypeScript profile loading | 0.533 | 4.81 |
| Profile Markdown context | 3.341 | 21.67 |
| Profile JSON context | 3.048 | 21.97 |
| Profile claim/release | 13.305 | 21.84 |

Release metrics use three samples, three iterations per sample and the existing
warm-up protocol. RSS is post-exec process VmHWM, including preparation, not
incremental allocations. The scoped report is marked unassessed against the
accepted two-example baseline because its input set differs; new profile phases
also lack accepted reference values. The earlier full validation's runtime
regressions remain documented in [tasks-validation.md](tasks-validation.md).

Native costs are separate: strict TypeScript checking took 898.9 ms with
257.2 MiB sampled process-tree RSS; Bun tests took 7.8 ms with 15.8 MiB sampled
RSS. These command measurements poll the sum of live descendant VmRSS at 1 ms;
they can miss short spikes and are not exact VmHWM. Dependency installation and
intentional failing mutation checks retain their own measurements.
