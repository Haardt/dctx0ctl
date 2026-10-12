# Rust language server and local editor extensions

The Rust CLI serves `.dctx0` language intelligence with `dctx0 lsp --stdio`.
It uses the same compiler as `check`, without publishing snapshots, opening SQLite,
changing tasks or executing application tests. Neither Bun nor Python is required
by the server or the installation commands.

## Local installation

Build or install the CLI first. Release binaries make substantially smaller bundles:

```sh
cargo build --release -p dctx0-cli --locked
./target/release/dctx0 editor install vscode --output "$HOME/dctx0-editors" --activate
./target/release/dctx0 editor install zed --output "$HOME/dctx0-editors"
```

`--activate` invokes the locally available `code --install-extension` with a
30-second timeout. Without it, VS Code receives a prepared `vscode/dctx0.vsix`:
use **Extensions: Install from VSIX** or the printed `code` command. The package
contains syntax rules, a dependency-free editor client and a copy of the running
native executable. The client runs that executable with `lsp --stdio`. To use a
separately updated CLI, set `dctx0.serverPath` to its absolute executable path.

For Zed, run **zed: install dev extension** and select the generated `zed` directory.
Zed builds the Rust/Wasm launcher using its normal extension tooling. Keep the
prepared directory in place: its launcher references the included executable,
and its manifest references a local Git repository containing the grammar at a
fixed commit. Git is needed when preparing this bundle, not by the language server.
The package contains its own Cargo.lock. Enable Zed semantic coloring with
`"semantic_tokens": "combined"` (or `"full"`); Tree-sitter colors work independently.

Use `--dry-run` to inspect destinations without writing them. The default output
is `.dctx0/editors`. Identical bundles are unchanged; differing bundles are preserved
unless `--force` explicitly replaces that editor directory. Unrelated extensions
and user settings are untouched. Copies are for the current binary's platform;
this command is not a cross-platform marketplace publisher. Local preparation
is reported separately from successful VS Code activation. Zed activation remains
an explicit UI action. Generated directories are ignored by Git.

Copying only an executable is insufficient: editors also need language registration,
syntax rules and a launcher. A future store release can distribute the same client
with separately versioned, platform-specific server packages. These commands do not
publish to either store or claim marketplace availability.

## Features and behavior

- Compiler diagnostics, including related source locations.
- Keyword/type/symbol completion, expression helpers, narrative and surface snippets.
- Hover with declaration identity, inherited architecture intents and purity requirements.
- Hierarchical document symbols, workspace symbols, definitions and actual resolved references.
- Full semantic tokens, UTF-16 positions, multiline strings/comments and Unicode-safe edits.
- Open/change/save/close notifications, watched-file changes and multiple initial workspace folders.

Open documents overlay disk sources; unsaved changes never modify the spec. Each
workspace declaration anchors an isolated project, so nested Cinema/tutorial examples
are not merged into one compilation. `--project /path/to/spec-root` fixes the searched
root; otherwise the client supplies workspace folders. Dependencies between fragments
remain compiler-owned. When the workspace is invalid, current recovered syntax still
supplies colors, symbols and candidate completions; semantic navigation/hover return
no result rather than stale successful analysis.

Edits are versioned and applied atomically. Analyses run on one background worker,
coalescing typing bursts for 60 ms; obsolete generations never publish diagnostics.
Request cancellation is supported for queued requests. Document/source limits are
2 MiB per file, 16 MiB discovered sources and 16 MiB open buffers, 2,048 files, discovery
depth 32 and 20,000 entries. Protocol messages are capped at 4 MiB, headers at 8 KiB,
transport queues at 16 messages and pending requests at 128. Resource limits produce
explicit log messages. The server does not run native harnesses when editing.
Symlink source entries are ignored. Only local file documents are supported.
Changing workspace folders requires restarting the server; formatting, code actions
and editor rename are not advertised. The existing explicit CLI rename remains available.

## Syntax compatibility and acceptance

Runatron's TextMate and Tree-sitter files are starting references, not an authoritative
Rust syntax definition. The shipped files were checked against the Rust lexer, type
and expression compiler, and complete Cinema sources. The port fixes standalone
`value` in refinements (`trim(value)`) and field access after calls
(`before.Film(request.filmId).title`). Logical/collection expressions receive current
Rust-specific color rules. Tree-sitter is a syntax/coloring grammar, not a second
semantic validator. Compiler IR, snapshot hashes and task definitions are unchanged.

Contributor checks (Python/Node/Tree-sitter here are QA tools, not runtime requirements):

```sh
cargo test -p dctx0-lsp
cargo test -p dctx0-cli --test lsp
cargo test -p dctx0-engine --test editors
# Requires tree-sitter CLI 0.26.13, Node and Rust in PATH.
python3 scripts/editor_acceptance.py --binary target/debug/dctx0 --functional-only
# Run measurements last, against an uninstrumented release binary.
python3 scripts/editor_acceptance.py --binary target/release/dctx0 --performance-only
```

The shared validation runner includes editor acceptance before coverage and editor
measurements at the end. It records native server speed and Linux VmHWM separately
from editor/client memory. Initial analysis and changes include the debounce delay;
colors and navigation use an already analyzed session. New workload identities remain
unassessed until an explicitly accepted baseline. Reports are under `target/editors/`.
The standalone acceptance exercises the actual VS Code client against the packaged
native server and compiles the prepared Zed launcher. GUI/store installation is a
separate smoke test when the editors are available; automated reports state that limit.

For an editor-only follow-up after a full workspace acceptance, use:

```sh
python3 scripts/validate.py --focus editors --coverage --example cinema --samples 1 --iterations 1
```

This groups compiler/build/lint checks, focused editor/skill tests and package
acceptance. Coverage still executes the complete instrumented Cargo workspace;
release measurements run last. It avoids repeating the separate application
acceptance/race suites already retained by the full validation run. Its report
explicitly names the `editors` scope. Single-sample measurements are development
evidence, not an automatically promoted baseline.

Acceptance (2026-10-12): the final editor follow-up passes all eight grouped commands,
including full instrumented workspace tests, with 29,720/30,421 merged lines (97.70%)
and 2,396/2,467 LLVM functions (97.12%). The earlier full application run passed
22 functional commands, Go/Rust Cinema (5,674 cases each, five actual mutations and
replay), but failed its initial coverage gate; that failure remains explicit.
The final run retains 71 release workloads and matching source hashes. New/reduced
workloads remain unassessed and the accepted baseline is unchanged. On this Linux
machine, the release LSP used approximately 23.5 MiB peak RSS for Cinema; initial
analysis/symbols took 61 ms, unsaved changes 84 ms (including debounce), semantic
colors 12 ms and workspace navigation 12 ms. These are single development samples,
not universal latency guarantees. See retained validation (historical evidence removed during repository cleanup).
