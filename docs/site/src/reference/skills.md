# Bundled skills and installation

The Rust binary embeds three independently named skills: `dctx0` for workflow
routing, `dctx0-language` for specification work and `dctx0-implementation` for
application/task/adapter work. Supporting references are installed with the entry
points. Installation needs neither a spec nor network access or Python.

```sh
dctx0 skills list
dctx0 skills install --project /path/to/application
dctx0 install --agent codex,claude --project /path/to/application --dry-run
dctx0 install --agent codex --global
dctx0 install --agent all --skill dctx0-language --project /path/to/application
```

The default installs all three into project `.agents/skills`. `install` is a
compatible shorthand for `skills install`. `--agent` and `--skill` accept repeated
or comma-separated selections; `--agent all` selects all supported layouts.
Use global `--json` for a schema-1 installation report. `--dry-run` previews without
creating directories or lockfiles. Byte-identical installed trees are unchanged.
Different content is preserved unless `--force` explicitly replaces that skill tree.
Unrelated skills, AGENTS.md, profiles, specs and history are untouched.

| Agent | Project root-relative skills directory | Global directory |
| --- | --- | --- |
| agents | `.agents/skills` | `HOME/.agents/skills` |
| codex | `.agents/skills` | `CODEX_HOME/skills`, else `HOME/.codex/skills` |
| claude | `.claude/skills` | `HOME/.claude/skills` |
| copilot | `.github/skills` | `HOME/.copilot/skills` |
| cursor | `.cursor/skills` | `HOME/.cursor/skills` |
| gemini | `.gemini/skills` | `HOME/.gemini/skills` |
| opencode | `.opencode/skills` | `XDG_CONFIG_HOME/opencode/skills`, else `HOME/.config/opencode/skills` |

HOME falls back to USERPROFILE. Explicit Codex/config roots do not require HOME.
These layouts preserve the former installer conventions; discovery remains the
responsibility of the selected agent and its supported version. Agent aliases
sharing a destination are installed once and reported together.

Every target is checked before skill trees are changed. Installations acquire
ordered root locks, stage all complete trees, then rename them into place. A normal
application error rolls back all applied destinations and restores previous trees.
If rollback/cleanup itself fails, the error/warning identifies retained backups;
no silent deletion of recovery data occurs. Existing symlinks, special files,
escaping paths and oversized trees are rejected even with force. This protects
ordinary installation and concurrent clients; it is not a hostile-filesystem sandbox
or a crash-recovery journal. Coordination locks may remain after installation.

## Comparison with Runatron

The reference is `../runatron/apps/dctx0/src/dctx0/dctx0-skill-installer.ts` plus
its portable skill and plugin language/implementation skill trees.

| Reference behavior | Rust CLI behavior |
| --- | --- |
| Portable dctx0 skill plus plugin language/implementation playbooks | Three embedded skills, with short references specific to the Rust CLI |
| Agent-specific project/global roots | Same agent names/layouts; Codex project/agents deduplication retained |
| Content equality, force, staged replacement and rollback | Retained, plus preview and serialized root writes |
| Runtime copied package skill assets | Compile-time embedded content; executable works outside the source checkout |
| Hosted tool names and standalone build/generate/task-check/finalize | Current check/tasks/profile/verify commands; no imaginary compatibility aliases |
| Generated TypeScript/Spring bindings and host promotion envelopes | Independent production, reviewed external adapters, guarded task events and finite native evidence |

Old rules forbidding all AGENTS.md edits or requiring fresh approvals per wave
were not copied as universal permissions. The new skills preserve the user's
scope and existing authorization, distinguish profile guidance from assertions,
and retain manual/UNKNOWN evidence rather than approving away native failures.
The skill content is versioned in `skills/`; change its embedded catalog/version
when intentionally changing shipped instructions. The installer does not fetch
third-party skills or manage unrelated agent plugins.
