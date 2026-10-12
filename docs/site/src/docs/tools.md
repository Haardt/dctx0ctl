# Skills and editors

## Three bundled skills

| Skill | Purpose |
| --- | --- |
| `dctx0` | Navigate the CLI workflow and interpret evidence. |
| `dctx0-language` | Author, review, and repair typed specifications, examples, and lifecycles. |
| `dctx0-implementation` | Implement application tasks and external adapters from pinned requirements, profiles, and native evidence. |

The agent can write the specification with you, then work through application tasks. Product decisions remain explicit; a failing implementation does not authorize weakening the contract.

```sh
dctx0 skills list
dctx0 install --project /path/to/app --agent codex,claude
dctx0 install --global --agent codex
```

Preview with `--dry-run`. Existing differing content is preserved unless you explicitly use `--force`. Skills are embedded in the executable; installing them does not require a source checkout or Python.

## Language server

```sh
dctx0 lsp --stdio
```

The read-only native server uses the language/compiler implementation for diagnostics, completion, hover, navigation, symbols, and semantic colors. Unsaved multi-file changes participate in analysis. It does not publish snapshots or mutate work history.

## VS Code and Zed

```sh
dctx0 editor install vscode --output "$HOME/dctx0-editors" --activate
dctx0 editor install zed --output "$HOME/dctx0-editors"
```

VS Code receives a local VSIX with client, syntax assets, and native server. `--activate` invokes `code --install-extension`. Zed uses its **install dev extension** UI with the generated directory; bundle preparation additionally uses Git. These are local packages, not a claim of marketplace publication.

Read the [skill installation guide](https://github.com/Haardt/dctx0ctl/blob/main/docs/skills.md) and [editor acceptance/setup](https://github.com/Haardt/dctx0ctl/blob/main/docs/editors.md).
