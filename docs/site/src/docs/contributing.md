# Architecture and contributing

DCTX0 is a Rust workspace with a language-neutral core. AST and validated IR remain separate. Targets consume validated semantics; filesystem/process effects belong to adapters.

## Processing flow

Sorted source discovery → parsing → composition and scoped resolution → type checking → validated IR → semantic graph → fingerprints and snapshot publication.

Diagnostics retain source files, codes, and positions. SQLite snapshots/task events/native runs are derived history, not a replacement for reviewed specifications.

## Workspace boundaries

| Area | Responsibility |
| --- | --- |
| Core and language | Models, parser, resolution, typed contracts |
| Interpreter and verification | Pure evaluation, deterministic corpora, plans, central assessment, shrinking |
| Graph and changes | Dependencies, cycles, impact, fingerprints, explicit lineage |
| Tasks and store | Pure planning, immutable definitions, append-only events, transactional history |
| Engine and target SDK | I/O, locks, managed builds/workers, versioned transport |
| Targets | Profiles, testing artifacts, supported native boundaries |
| CLI and LSP | Commands/rendering and editor-facing language service |

Production applications import no DCTX0 runtime or generated harness. Workers report observations rather than independent verdicts. Add targets without changing language semantics.

## Validation with speed and RAM

Use current stable Rust, Python 3, Linux for RSS measurement, and the native target toolchains. The project CI also installs the required coverage tooling.

```sh
rustup component add rustfmt clippy llvm-tools-preview
cargo install cargo-llvm-cov --locked
python3 scripts/validate.py --coverage
```

The runner couples relevant checks, Rust/native tests, formatting, Clippy, and coverage with uninstrumented release measurements at the end. Production Rust coverage must reach **97% merged source lines and 97% LLVM functions**. Raw LLVM totals and non-Rust helper coverage remain separate. Line coverage is not complete branch proof.

Release benchmarks retain elapsed milliseconds, peak RSS bytes, raw samples, workload/input hashes, toolchain, environment, and cache provenance. Native compiler/worker costs have their own memory scope. The latest 20 runs live in `benchmarks/history/`; `benchmarks/baseline.json` is separate.

Comparable regression limits default to 20% runtime and 10% RAM. Missing/incompatible baselines are unassessed. Promote only intentionally after review:

```sh
python3 scripts/validate.py --accept-baseline latest
```

Reduced samples and `--skip-performance` are development modes, not final performance acceptance. Never update a baseline automatically to hide a regression.

## Release distribution

A pushed `v<SemVer>` tag matching the workspace version creates a release after full validation and native x86_64/ARM64 builds. Archives include a versioned executable, license, and source/toolchain receipt; the Bash installer checks SHA-256 before atomic installation.

```sh
python3 scripts/release_acceptance.py --cargo-tests
```

Build the release CLI first. The focused runner couples archive/installer checks with release packaging/startup speed and RSS, retaining the latest 20 reports under `benchmarks/history/release/`. Baseline comparison is unassessed until explicitly accepted. Read [the release guide](releases.md).

## Build this documentation

This site uses pinned **mdBook 0.5.4**, a Rust documentation generator. The generated HTML, search index, scripts, and assets are served statically.

```sh
python3 scripts/install_mdbook.py
python3 scripts/docs_site.py --dctx0 target/release/dctx0
```

The installer downloads a checksum-verified Linux x86_64 release into ignored `target/docs-tools/`. On another platform, install `mdbook` with `cargo install mdbook --version 0.5.4 --locked` and put it on PATH.

The site runner builds HTML, validates internal links/anchors/assets and complete DSL examples, then measures release mdBook build speed/RSS. Raw documentation measurements live separately under `benchmarks/history/docs-site/`, retaining the latest 20; they do not alter the core accepted baseline. A comparison is explicitly unassessed until documentation has an accepted compatible baseline.

```sh
target/docs-tools/mdbook serve docs/site --hostname 127.0.0.1 --port 4180
```

The generated site is `target/docs-site`. Navigation, search, themes, and code copying are bundled locally. Authored chapters are under `docs/site/src`; generated output stays ignored. See the [publishing guide](https://github.com/Haardt/dctx0ctl/blob/main/docs/site/src/reference/documentation-site.md) for GitHub Pages setup and content maintenance.

## Further developer references

[Task API](https://github.com/Haardt/dctx0ctl/blob/main/docs/site/src/reference/tasks.md), [conformance reviews](https://github.com/Haardt/dctx0ctl/blob/main/docs/site/src/reference/conformance-reviews.md), [coverage](https://github.com/Haardt/dctx0ctl/blob/main/docs/site/src/reference/coverage.md), and [architecture boundaries](https://github.com/Haardt/dctx0ctl/blob/main/docs/site/src/reference/refactoring.md) retain detailed contracts and checkpoint evidence.
