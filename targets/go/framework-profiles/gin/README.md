# Gin project profile preset

These English Markdown files are optional agent guidance, not an automatically selected target capability. The supported reference version and actual harness acceptance are documented in docs/http-frameworks.md.

From a checkout, first run `dctx0 profile go create --project /path/to/app --default --agents`. Review and merge this directory's `common.md` and `implement`/`use` files into `/path/to/app/.dctx0/profiles/go/`. The common file includes the target-wide defaults once. For an existing project, preserve custom instructions rather than overwriting files wholesale. Download the same authored files from the repository when using an installed binary; no new framework installer or CLI flag is provided.

Read the task's implementation context to confirm the selected documents. Profiles influence coding agents; the project adapter under `.dctx0/testing/go` must separately connect actual production code. `dctx0 verify init` supplies that adapter's scaffold and technical config, not a working framework application. See docs/http-frameworks.md for the complete workflow.

Profile edits do not change the spec or complete tasks. Used bundles stay frozen for claimed/reserved work. Apply the existing explicit profile adaptation workflow to completed work when needed.
