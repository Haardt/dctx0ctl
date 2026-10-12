# Documentation site

The English product landing page and user guide are built with **mdBook 0.5.4**. Sources live in `docs/site/src`, presentation in `docs/site/theme`, and navigation in `docs/site/src/SUMMARY.md`. Generated HTML is ignored under `target/docs-site`.

The site presents current DCTX0: Rust CLI installation, collaborative specification authoring, WorkshopHub contract modeling, MiniBoard's executable applications, language reference, task workflows, native TypeScript/Go/Rust adapters, source-bound reviews, skills, and editors. It contains no registry/account/pricing preview or server dependency.

## Build and validate

From the checkout root:

```sh
python3 scripts/install_mdbook.py
cargo build --release --locked -p dctx0-cli --bin dctx0
python3 scripts/docs_site.py --dctx0 target/release/dctx0
# Optional browser checks with installed Bun and Chromium:
python3 scripts/docs_site.py --dctx0 target/release/dctx0 --browser
```

The installer verifies the official Linux x86_64 archive against its pinned SHA-256 before installing under `target/docs-tools`. On another platform:

```sh
cargo install mdbook --version 0.5.4 --locked
python3 scripts/docs_site.py --mdbook /path/to/mdbook --dctx0 /path/to/dctx0
```

The validation runner checks generated internal links, anchors, script/font/style assets, and every complete DSL block marked `<!-- dctx0:check -->`. Excerpts require their surrounding declarations and are identified as contextual examples. External URLs are not fetched by the local-link check.

Functional checks run first, then one warm-up and three fresh-process release mdBook build measurements. Each report retains raw elapsed milliseconds, peak RSS bytes, input content hash, compiler binary hash, Git revision/dirty status, platform/hardware, and cache conditions. Linux memory uses `wait4.ru_maxrss` for each child/reaped descendants, not simultaneous collective RSS; unsupported platforms report `null`, never zero. When available, the existing Linux systemd memory guard caps the runner and descendants at two thirds of available startup RAM. The report records its active state/budget; unavailable or disabled support is explicit.

Reports, including failed validation, are written atomically under a lock to `benchmarks/history/docs-site/`. The latest 20 are retained. These are separate documentation workloads; there is no accepted documentation baseline, so regression comparison is explicitly unassessed. They do not promote or alter `benchmarks/baseline.json`.

## Preview

```sh
target/docs-tools/mdbook serve docs/site --hostname 127.0.0.1 --port 4180
```

Open <http://127.0.0.1:4180/>. For an already built artifact:

```sh
python3 -m http.server 4180 --bind 127.0.0.1 --directory target/docs-site
```

The landing page and documentation share a single build. Documentation provides local full-text search, responsive sidebar navigation, code copying, and selectable themes. The landing page has its own responsive layout; all fonts and scripts are served locally.

## GitHub Pages

[The documentation workflow](https://github.com/Haardt/dctx0ctl/blob/main/.github/workflows/pages.yml) checks pull requests and builds pushes to `main`. CI also runs desktop/mobile navigation, local search, highlighting, copy-control, and asset checks with Bun and the installed Chromium/Chrome, retaining screenshots. Only successful `main` builds deploy. The environment is `github-pages`; the deployment job has `pages: write` and `id-token: write` while the build job has read-only repository access.

In repository **Settings → Pages → Build and deployment**, select **GitHub Actions** as the source. The workflow can then publish the project site at `https://haardt.github.io/dctx0ctl/`. This is the configured destination, not evidence of a deployment until GitHub reports success.

`docs/site/book.toml` sets `site-url = "/dctx0ctl/"` for project-subpath and 404 assets. Chapter/landing links and local assets are relative, so normal navigation also works in a local preview. If the repository or hostname changes, adjust the repository/edit URLs and `site-url` together. A custom domain additionally needs the corresponding GitHub Pages/DNS configuration.

Builds produce a standard Pages artifact. No running application, hosted account service, Node server, analytics, or external fonts are required.

## Content maintenance

Update authored Markdown, then rerun the site validator. Complete examples should carry the compiler-check marker; partial examples must state their scope. Keep executable support separate from compiler acceptance, native observations separate from agent judgments, and Review approval separate from testing PASS.

Current conformance tasks can be approved and Done/Unverified; complete acceptance recognizes current source-bound reports separately through `reviewed_tasks`. Explain task counts with their actual requirements/planner scope. Avoid promising a native Java target, production generation, or hosted services that are not implemented.

The supported DSL catalog is maintained by the parser, compiler, interpreter, and native examples. Check command flags against `crates/dctx0-cli/src/args.rs`, `verification.rs`, and `tasks/args.rs` when updating guides. Keep detailed developer evidence in the existing `docs/` files and link to it from the user guide as needed.

Generator and deployment references: [mdBook HTML configuration](https://rust-lang.github.io/mdBook/format/configuration/renderers.html), [mdBook CI](https://rust-lang.github.io/mdBook/continuous-integration.html), and [GitHub Pages custom workflows](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages).
