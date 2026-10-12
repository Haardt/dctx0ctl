# Gin and Axum project integrations

DCTX0's language targets execute a common Rust-assessed plan. HTTP frameworks are application choices, connected through a reviewed project adapter. This increment adds reference integrations for **Gin 1.12.0 / Go** and **Axum 0.8.9 / Rust**. It does not add a framework selector to DCTX0, change specification semantics, or generate production servers.

## Choose agent guidance

Create the target's editable profiles in the **application root**:

```sh
dctx0 profile go create --project /path/to/app --default --agents
# Or:
dctx0 profile rust create --project /path/to/app --default --agents
```

Review and merge the preset from `targets/go/framework-profiles/gin/` or `targets/rust/framework-profiles/axum/` into `.dctx0/profiles/<target>/`. Each preset provides `common.md` and `implement`/`use` documents for surface, operation, port, external and test. The common file already includes target-wide guidance once. Preserve project-specific conventions when merging. These repository files are downloadable guidance, not additional files installed automatically by the binary.

`common.md` covers framework composition and lifecycle. Keyword documents tell the coding agent how to implement or consume its assigned boundary. No profile forces migration of an adopted application's framework. URLs, methods, status codes, authentication and middleware requirements must be explicitly agreed; a domain operation does not imply a public REST endpoint.

Inspect a real task to confirm the selected context:

```sh
dctx0 tasks show /path/to/app/dctx0 T42 --project /path/to/app --target go
```

Own subjects select `implement`, while referenced contracts and collaborators select `use`; common guidance appears once. The maintained `AGENTS.md` block points agents to the profiles. Existing instructions outside that block remain intact. Claims and reservations freeze the used bundle. Editing profiles changes neither task requirements nor historical results; completed work uses the explicit profile adaptation workflow documented in [profiles](https://github.com/Haardt/dctx0ctl/blob/main/docs/site/src/reference/profiles.md).

## Connect the actual application

```sh
dctx0 verify init /path/to/app/dctx0 --project /path/to/app --target go
# Implement .dctx0/testing/go/adapter.go against actual production code.
dctx0 verify run /path/to/app/dctx0 --project /path/to/app --no-adopt
```

Use `--target rust` and `.dctx0/testing/rust/adapter.rs` for Rust. Initialization preserves existing files and produces an explicitly unimplemented scaffold. Profiles instruct the agent; the adapter supplies the executable connection. Neither a profile edit nor a framework dependency alone creates a working harness integration.

The adapter owns fresh sessions, translates wire/domain values and invokes real services. HTTP cases invoke the actual production router, including middleware and error mapping. Mocks replace declared collaborators only. Missing state/effect observers remain unsupported; a mocked consumer test does not establish production adapter behavior.

## Additional Cinema variants

The existing Go `net/http` and Rust builtin HTTP servers remain defaults. Gin and Axum reuse the same Cinema domain, services, SQLite persistence and unchanged spec; they add transport implementations rather than copied applications.

### Go / Gin

```sh
go -C examples/cinema-go mod download
CINEMA_HTTP_FRAMEWORK=gin go -C examples/cinema-go run ./cmd/cinema
```

`CINEMA_HTTP_FRAMEWORK` accepts `net-http` or `gin`; empty means `net-http`. The project adapter's `config.json` accepts the optional setting `"httpFramework": "gin"`. Omitting it keeps `net-http`. These are Cinema application/adapter settings, not DCTX0 framework configuration.

Gin registers explicit routes, translates path/query/body values and uses the actual service. Body parsing preserves exact integers through the existing domain decoder. Tests use `httptest.NewServer`; The reference integration configures Gin release mode once before sessions and directs debug/error writers to stderr, avoiding per-router debug output exhausting the harness log budget. The framework middleware supplies `X-Cinema-Framework: gin`, providing an actual wiring observation.

### Rust / Axum

```sh
cargo fetch --manifest-path examples/cinema-rust/Cargo.toml --locked
CINEMA_HTTP_FRAMEWORK=axum cargo run --manifest-path examples/cinema-rust/Cargo.toml \
  --features axum-http
```

The production selection accepts `builtin` or `axum`; empty/unset means `builtin`. Axum requires the optional `axum-http` Cargo feature. The project adapter enables that feature and accepts `"httpFramework": "axum"` in its own `config.json`; omission keeps `builtin`.

Axum composes explicit routes with typed shared state. Common Cinema request translation is reused, and blocking service/SQLite work runs through `spawn_blocking`. One owned current-thread runtime per HTTP session uses at most two blocking threads. The server binds a dynamic loopback port in tests and has bounded graceful shutdown. Tower tests exercise the production Router. A standard HTTP client in the testing area supports chunked transfer, explicit status checks, a five-second timeout, no redirects/proxies and a 1 MiB response limit. Middleware supplies `X-Cinema-Framework: axum`.

In the framework variants, generated real Surface operation cases also use HTTP, so missing-resource failures exercise the actual status mapping. Mocked consumer scenarios retain their separate direct-service test path.

Both variants retain Cinema's explicit successful 200 responses and failure mapping: missing domain resources 404, closed voting 409, unavailable persistence 503, malformed/invalid input 400. Unknown routes are 404 and unsupported methods 405 in the framework variants. Gin retains the existing 400 response for bodies above 1 MiB; Axum rejects them with 413. These transport decisions are separate native HTTP assertions, not new spec requirements. Successful bodies preserve the actual domain payload.

## Acceptance and evidence

```sh
PATH="$HOME/.cargo/bin:$PATH" python3 scripts/framework_acceptance.py
```

The contributor runner executes native Go/Rust tests, static checks and both framework Cinema plans, with actual routing/error/middleware/business mutations and broken/repaired replay. It then measures release first-build and warm native execution, and HTTP router checks, separately. Existing TypeScript, Go and Rust reference paths remain acceptance cases. Python is contributor orchestration; installed DCTX0 remains Rust and executes its native toolchains without Python.

Every framework plan must retain all **5,674 mandatory cases**, no missing cases and complete provenance. Native manual/suite obligations remain UNKNOWN until separately reviewed. Current source-bound reviews address the assigned 25 obligations without turning review approval into native testing PASS. Changed config, adapters, routers and lockfiles require current review binding; historical approval is not copied onto new code.

Evidence is retained under `target/frameworks/` and the latest 20 coupled measurement reports under `benchmarks/history/frameworks/`. Cold preparation uses a fresh managed build with the existing compiler/module caches; it is not a claim of an empty machine cache. Peak RSS reports its process/descendant measurement scope and is not collective allocation. New framework workloads are unassessed; the accepted baseline is never promoted automatically.

Native harness acceptance currently runs on Linux. The existing optional Linux/systemd guard limits the runner and children to two thirds of available RAM; unavailable/disabled support is explicit. This is not a security sandbox. Framework support means the documented reference versions and tested boundaries, not automatic compatibility with every middleware, framework version or application.

## Retained reference acceptance (2026-10-12)

The retained framework report (historical evidence removed during repository cleanup) records both variants with 5,674 mandatory/recorded cases, 105 delivered tasks and four source-bound Done/Unverified reviews each. All eight routing, status, middleware and application-exception mutations produce executed HTTP failures; broken routing replay fails and repaired replay passes. The original TypeScript/Go/Rust paths also retain their complete case counts. Native Go tests/vet/race checks, Rust default/all-feature tests, adapter chunked-response checks and clippy pass.

Full fresh workspace coverage, including the deferred benchmark conformance test, is **30,356/31,076 merged source lines (97.68%)** and **2,454/2,529 LLVM functions (97.03%)**. This is workspace Rust coverage, not HTTP/library-helper coverage or branch completeness. The earlier partial-performance-deferred coverage remains an explicitly failed checkpoint. Contributor Python test-command integration was corrected, and editor acceptance passed after installing its required local Tree-sitter CLI; no Rust core changes or threshold reductions were needed.

On the retained Linux machine, three warm full-plan release samples have medians of **9.22 seconds for Gin** and **14.73 seconds for Axum**. Worker-reported peak RSS is approximately **21.9 MiB / 8.3 MiB**, respectively. CLI/build/descendant peaks are separate and larger (approximately **393.5 MiB / 292.4 MiB**); they are not simultaneous collective allocation. The first managed builds use existing compiler caches and are not empty-cache measurements. The raw report includes build audits, compiler/worker identity, source/binary hashes, individual samples and memory scopes. These new workloads remain unassessed; the accepted baseline was not changed.
