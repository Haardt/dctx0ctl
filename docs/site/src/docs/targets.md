# Targets and profiles

DCTX0 separates language semantics from implementation language and framework choices. Rust centrally owns generation, expectations, assessment, shrinking, and replay. Targets execute the common plan through testing-only adapters.

| Target | Profiles | Structural discovery | Native execution |
| --- | --- | --- | --- |
| TypeScript / Bun | Yes | Tree-sitter | Yes |
| Go | Yes | Tree-sitter | Yes |
| Rust | Yes | Tree-sitter | Yes |
| Java | Yes | Not implemented | Not implemented |

Structural discovery identifies declarations, imports, candidates, and ambiguity. It is not a semantic type proof or a complete runtime call graph.

## TypeScript / Bun

Install Bun and application dependencies. Cinema uses independent production code and an external adapter under `.dctx0/testing/typescript`.

```sh
bun install --frozen-lockfile --cwd examples/cinema
dctx0 verify run examples/cinema/dctx0 --project examples/cinema --no-adopt
```

Read [TypeScript adapters](typescript-adapters.md). Frameworks are application choices: direct Bun functions and Hono/Awilix classes can implement the same contract.

## Go

Cinema Go uses Go 1.25+, `net/http`, ordinary interfaces, and CGO-free SQLite. Prepare modules before managed verification:

```sh
go -C examples/cinema-go mod download
dctx0 verify run examples/cinema-go/dctx0 --project examples/cinema-go --no-adopt
```

A managed build compiles the worker once, records selected packages and compiler inputs, and captures the actual executable. Central Rust assessment supplies the verdicts. [Go target boundaries](https://github.com/Haardt/dctx0ctl/blob/main/docs/go-target.md) document supported module/dependency capture.

## Rust

Cinema Rust uses ordinary application types and traits, SQLite, and real HTTP. Prepare the testing package's locked dependencies:

```sh
cargo fetch --manifest-path examples/cinema-rust/.dctx0/testing/rust/Cargo.toml --locked
dctx0 verify run examples/cinema-rust/dctx0 --project examples/cinema-rust --no-adopt
```

Managed Cargo builds are offline/locked. Captured package/toolchain inputs and the archived worker executable receive separate provenance. A guarded cache avoids recompiling unchanged dependencies. [Rust target boundaries](https://github.com/Haardt/dctx0ctl/blob/main/docs/rust-target.md) document C toolchain capture and unsupported overrides.

## Editable profiles

See [Configure and extend profiles](profiles.md) for what profiles contain, application configuration, and contributing new defaults from a DCTX0 checkout.

```sh
dctx0 profile rust create --project /path/to/app --default --agents
dctx0 tasks show /path/to/app/dctx0 T42 --target rust
```

Four targets ship embedded editable Markdown defaults under `.dctx0/profiles`. Existing files are preserved. Profiles guide architecture and implementation without changing specification semantics. Used bundles are pinned with claims and reservations.

Profile edits do not automatically create tasks or change results. Explicit preview/update operations can create adaptation work; historical results remain intact. Prose in a profile is not automatically proved.

## HTTP framework presets

Go/Gin and Rust/Axum have optional English Markdown presets to merge into the application's existing profiles. There is no framework-selection flag: language selection remains `--target`, and the reviewed project adapter independently connects the actual router. Existing Cinema HTTP defaults remain alongside the added framework variants.

See [the complete setup and acceptance guide](http-frameworks.md) for profile files, application/adapter selection, supported versions and evidence boundaries. Source-bound reviews remain separate from native HTTP checks.

## Cinema acceptance

The documented complete Cinema applications use matching specifications and semantic plans with **5,674 mandatory finite cases**. Tests include actual persistence/HTTP, five production mutations for Go and Rust, and broken/repaired replay.

A fresh Cinema plan has 105 tasks with four Review tasks for 25 manual/suite obligations. Review approval stays separate from native testing. Task counts describe the selected requirements and planner, not a universal benchmark.

Native manual/suite UNKNOWN and source-bound authored review remain separate. Current complete acceptance can recognize delivered, bound review tasks without pretending their approval is native testing PASS.

## Process limits

Managed native execution currently requires Linux. Workers/builds have deadlines, cancellation, output limits, and cleanup. When available, a Linux systemd user scope provides an optional collective memory cap. These limits are not a security sandbox. Unavailable tools, unsupported representations, or incomplete provenance remain explicit gaps.
