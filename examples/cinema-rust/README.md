# Cinema in Rust

This independent application implements the unchanged complete Cinema spec with
ordinary Rust traits and constructors, actual SQLite storage and explicit HTTP
routes. Production imports no dctx0 crate, spec or generated harness. Application
contracts use serde_json's arbitrary-precision numbers, checked BigInt conversion,
closed alternatives and application-owned runtime validators.

Prepare dependencies using the committed lockfiles:

```sh
cargo fetch --manifest-path Cargo.toml --locked
cargo fetch --manifest-path .dctx0/testing/rust/Cargo.toml --locked
```

Start the actual application:

```sh
cargo run --locked
```

It listens on `127.0.0.1:4182`, with durable `cinema.sqlite3` storage. Set
`CINEMA_ADDRESS` and `CINEMA_DATABASE` to override those defaults. The Linux
production ID factory uses OS entropy and production timestamps use UTC.

The external adapter supplies fresh sessions, actual consumer collaborators,
real SQLite state/query observations, controlled test sources and actual TCP
requests to the production HTTP server. Model metadata supplies representation
conversion only; the Rust verification core decides expectations and outcomes.

```sh
dctx0 check dctx0
dctx0 verify init dctx0 --project . --target rust
dctx0 verify discover dctx0 --project . --target rust
dctx0 verify run dctx0 --project . --no-adopt
dctx0 verify complete dctx0 --project .
```

The adapter is a separate Cargo package. Repeated init preserves its authored
adapter, manifest, lockfile and configuration. Complete verification combines
native finite evidence with explicitly source-bound review notes; it does not
translate free text into assertions. Use `DCTX0_MEMORY_GUARD=0` to disable the
optional Linux/systemd collective memory cap. The product CLI requires Rust and
a C toolchain for bundled SQLite, without Python.

Independent application checks are `cargo test --locked` and
`cargo clippy --all-targets --locked -- -D warnings`. Repository-wide validation
also exercises all targets, coverage, real production mutations and replay;
release speed/RSS measurements run last. Final retained acceptance is documented
in `docs/site/src/docs/targets.md` at the repository root.

## Additional Axum HTTP variant

The existing HTTP implementation remains the default. The additional Axum router reuses this application and its unchanged Cinema spec. See [framework setup](../../docs/site/src/docs/http-frameworks.md) for production selection, adapter configuration, optional profiles, tests and limits.
