# Configure and extend profiles

## What is a profile?

A profile is editable Markdown guidance for the coding agent that implements a task. The specification defines the required behavior; profiles explain how to implement it in your language and project: representations, checked construction, error handling, dependency boundaries, framework conventions, and testing practices.

DCTX0 ships defaults for TypeScript, Go, Rust, and Java. It selects relevant documents from the task's specification context and includes them in its implementation guidance. Profiles do not generate production code, add specification requirements, or turn prose into executable assertions.

| Document | Purpose |
| --- | --- |
| `common.md` | Shared project and language conventions, included once per bundle. |
| `implement/<keyword>.md` | Guidance for building the task's own contract or operation. |
| `use/<keyword>.md` | Guidance for consuming an existing dependency without redefining it. |

For example, TypeScript's shared guidance recommends branded domain primitives with checked construction. A value task defines its representation once; a consuming operation reuses that type and constructor. Go defaults use named types, Rust uses newtypes, and Java uses immutable value objects. Adapt these choices to the existing application.

## Configure an application

Create profiles in the **application root**, independently of the specification directory:

```sh
dctx0 profile typescript create --project /path/to/app --default --agents
```

Replace `typescript` with `go`, `rust`, or `java` as appropriate. This creates:

```text
/path/to/app/.dctx0/profiles/
  config.json
  typescript/
    manifest.json
    common.md
    implement/
      value.md
      operation.md
      surface.md
      ...
    use/
      value.md
      operation.md
      surface.md
      ...
```

`config.json` records the default language target; `manifest.json` records the template contract. Use the CLI to create this technical configuration. Put architecture and coding choices in the Markdown files.

The first configured target becomes the default. Later creation changes it only with `--default`. An explicit task `--target` must refer to a configured target. Without an explicit application root, task commands search upward from the specification directory for the nearest profile configuration.

Edit `common.md` for shared conventions, such as a classes-only application, one shared branded-type helper, or the project's error-handling approach. Edit `implement/surface.md` for HTTP composition rules and `use/surface.md` for how callers consume that boundary. Keep shared rules in `common.md` rather than repeating them in every keyword document.

`--agents` adds a managed profile-discovery block to the application's `AGENTS.md`. Existing instructions outside that block remain intact. `AGENTS.md` supplies project-wide instructions to the agent; task profiles supply the selected implementation guidance. Keep both consistent with the reviewed specification.

Inspect the guidance delivered for a task:

```sh
dctx0 tasks show /path/to/app/dctx0 T42 --project /path/to/app --target typescript
dctx0 tasks context /path/to/app/dctx0 T42 --section implementation --format json
```

Use an actual task ID from your plan. The context command can discover profiles when the specification is inside the configured application root.

## Apply changes deliberately

Running `profile ... create` again preserves existing Markdown, including empty files, and fills missing defaults. Updating the executable therefore does not overwrite your conventions.

Editing profiles does not change task progress or testing results. Claims and reservations pin the guidance used for that attempt; historical bundles remain available. For completed work that should adopt revised guidance, preview affected tasks and explicitly publish adaptation work:

```sh
dctx0 tasks profile-changes /path/to/app/dctx0 --all-affected --format json > update.json
# Review update.json before applying it.
dctx0 tasks profile-update /path/to/app/dctx0 --file update.json \
  --reason "Adopt revised implementation guidance"
```

The update creates eligible adaptation work with new identities and preserves historical results. It does not rewrite completed tasks or automatically prove the edited guidance.

## Extend the shipped defaults in DCTX0

To contribute defaults for future projects, check out DCTX0 and edit the target's source Markdown:

```sh
git clone https://github.com/Haardt/dctx0ctl.git
cd dctx0ctl
# Edit targets/go/profiles/common.md and the relevant implement/use files.
cargo build --release --locked -p dctx0-cli --bin dctx0
target/release/dctx0 profile go create --project /path/to/fresh-app --default --agents
```

Equivalent directories exist under `targets/typescript`, `targets/rust`, and `targets/java`. The target's `src/lib.rs` registers embedded documents through `Provider::templates()` and `include_str!`. Editing an existing registered file requires rebuilding the executable. Adding a new shipped document also requires registering its path and content there.

Keyword documents use existing specification statement kinds. An optional document such as `use/intent.md` can add guidance for an existing keyword; creating a Markdown filename does not add a language keyword. Changes to mandatory keys or manifest versions require a deliberate compatibility migration, because a conflicting local manifest is rejected.

Export updated defaults into a fresh directory to review them. Merge approved changes into existing projects manually; creation preserves their local files. Run the repository's documented validation before contributing changes, including profile publication and task-context checks. See [Architecture and contributing](contributing.md) for the development workflow.

## Framework guidance and executable support

The optional [Gin and Axum presets](http-frameworks.md) provide concrete HTTP guidance to merge into local Go or Rust profiles. They are repository presets, separate from the embedded language defaults; there is no `--framework` profile switch.

A profile can tell an agent to use a framework. Native verification still needs a reviewed testing adapter that calls the actual application and router. Adding new executable capabilities requires implementing the target and SDK contracts and their conformance tests; editing Markdown alone does not activate a harness or establish verification support.
