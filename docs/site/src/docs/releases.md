# Releases and installation

Install the published CLI on Linux or macOS:

```sh
curl -fsSL https://github.com/Haardt/dctx0ctl/releases/latest/download/install.sh | bash
export PATH="$HOME/.local/bin:$PATH"
dctx0 --version
```

On Windows x64, use PowerShell:

```powershell
irm https://github.com/Haardt/dctx0ctl/releases/latest/download/install.ps1 | iex
$env:PATH = "$env:LOCALAPPDATA\DCTX0\bin;$env:PATH"
dctx0 --version
```

Release targets are Linux x64/ARM64 with glibc 2.35+, macOS Intel/Apple Silicon with macOS 14+, and Windows x64. The shell script needs Bash, curl, tar and `sha256sum` or `shasum`; Windows uses PowerShell and a ZIP. Installation requires no Rust compiler or sudo.

Each installer selects one stable tag, checks the archive's SHA-256 and executable version, then atomically replaces the binary. Download/checksum/version failures preserve an existing installation. Shell profiles and persistent PATH settings are unchanged.

**Install your own application toolchain and dependencies**: Bun for TypeScript, Go for Go, or Rust/Cargo plus a C compiler for Rust. Specification checks, task planning and LSP need only the DCTX0 binary. The installer supplies DCTX0.

**Managed native verification, replay and guarded automatic adoption are verified on all five release platforms** for TypeScript/Bun, Go and Rust. Each native job requires actual success and failure checks, repaired replay, task/review guards, Cinema persistence/HTTP and separate platform coverage. Linux retains process-tree supervision; macOS and Windows terminate and wait for the direct child with bounded stream cleanup. Optional process-tree supervision and unavailable RSS remain explicit capabilities. Rust requires Xcode Command Line Tools on macOS or an MSVC developer environment with the Windows SDK on Windows.

For v0.2.4, the maintainer explicitly requested no further tests. Workspace,
distribution and native acceptance tests are skipped only for that tag; all five
native builds, version guards, packaging and checksums remain required. Existing
local evidence and incomplete coverage/performance acceptance are recorded in
`docs/dependency-resolution-validation.json`. This release introduces configurable
native dependency resolution with a default of 10,000 references.

## v0.2.5: bounded storage and large review groups

Verification history retains three complete runs per project and compact historical
summaries for older runs. Cases are compressed once, large artifacts are compressed
when beneficial, and successful cases omit duplicate diagnostic journals. New
`verify history` and `verify gc --compact` commands expose retention and explicit
maintenance without a backup. SQLite writers migrate additively to schema 5;
older CLI versions cannot read that schema. Task history and old verdicts remain,
while pruned evidence cannot authorize replay or fresh adoption.

Inspect, Attest and Complete share a streaming JSON reader with a 64 MiB default
and configurable `--max-input-bytes`. Complete 19/47-review groups with 543 source
modules are covered by native acceptance and individual release CLI RSS samples.
Mismatched or duplicate bound reviews are rejected before publication; no implicit
accumulation or promotion of incomplete native evidence is introduced. See
[large review inputs](../reference/inspection-inputs.md).

This version requires fresh workspace and five-platform release checks; the
v0.2.4 test exception does not apply. See [verification storage](../reference/verification-storage.md)
for compatibility, commands and measured maintenance results.

## v0.2.6: actual call observation in examples

Call-count assertions now consume actual adapter calls across the completed
`when` prefix, including declared application failures. An observed empty list
can prove zero calls; a missing observer remains UNKNOWN. Repeated calls count
separately, arguments and operation identity must match, and mock callbacks do
not double count actual calls. Call observation no longer requires mock injection
unless the example prepares mock replies.

The Linux QA guard monitors aggregate process-tree RSS at one third of startup
available RAM, including detached descendants, and works without a cgroup memory
controller. Shared pages count per process in 50 ms samples; this is not a hard
kernel allocation limit. Release builds and test jobs run serially. See
[call observation](../reference/call-observation.md) and
`docs/call-observation-validation.json` for the retained local evidence, including
the unsupported local Go Race host and unassessed performance baseline. Normal
CI validation, installer checks and all five native hosts remain release gates.

## Pin a version

```sh
curl -fsSL https://github.com/Haardt/dctx0ctl/releases/download/v0.2.1/install.sh \
  | bash -s -- --version v0.2.1 --bin-dir "$HOME/.local/bin"
```

```powershell
& ([scriptblock]::Create((irm https://github.com/Haardt/dctx0ctl/releases/download/v0.2.1/install.ps1))) -Version v0.2.1 -BinDir "$env:LOCALAPPDATA\DCTX0\bin"
```

The executable includes the CLI, language server, target profiles, agent skills and testing templates. Releases contain four tar archives, one Windows ZIP, both installers and `SHA256SUMS`. Archives include the MIT license and a source/toolchain receipt. The manifest covers every archive and installer.

## Publish

A pushed `v<SemVer>` tag matching the Cargo workspace version triggers publication after full validation and all five distribution jobs pass. Stable tags become the latest release; prereleases are marked explicitly and skipped by the default installer.

For v0.2.3, the maintainer requested reusing the completed full Linux validation
and native five-platform acceptance. Its release gate checks pinned report hashes
and unchanged runtime inputs, allowing only the package version update. All five
new release binaries and installers still pass fresh distribution checks with
time/RAM measurements. The exception is limited to v0.2.3; later releases retain
the full validation and native acceptance stages.

```sh
git tag -a v0.2.1 -m "DCTX0 v0.2.1"
git push origin v0.2.1
```

Tag the reviewed, committed version and matching lockfile. The installer URLs become available after the first successful release. CI uses GitHub's native runners; systemd memory limits apply only on Linux. Local distribution validation records test results and packaging/startup speed/RAM in the latest 20 measurement reports.

See the repository's [release maintainer guide](releases.md) for commands, provenance and platform boundaries.

For v0.2.1, the maintainer explicitly requested skipping CI workspace validation
and distribution tests. Five native builds, version guards, packaging and checksums
remain required. Later releases retain normal validation. The release includes
[Static Site and Task Live View](site-companion.md), with embedded assets and no
Bun/Node requirement for end users.

## v0.2.8: pinned workflow context and native diagnostics

Dependency-closed work packages deduplicate complete context and retain pinned
specification, profile, task and evidence identities. Delta exports support
resumption; progress reports distinguish Done and historical PASS transitions.
Compact native summaries, filtered case pages and a read-only adoption preview
explain current evidence and delivery blockers. Updated embedded skills and an
external usage-analysis tool support bounded, measurable handoffs.

The companion retains task selection across live refreshes and reads large
evidence safely. Productive recursive generation alternatives remain available
when another branch has no bounded inhabitants. Existing task definitions and
SQLite schemas are unchanged. See [workflow efficiency](../reference/workflow-efficiency.md).

Fresh full validation and all five native platform jobs gate publication. The
local ARM64 Go race probe cannot run on its 39-bit VMA host; no exemption is added.

## v0.2.9: editable codec scaffolds and workflow continuation

Optional TypeScript codec scaffolds accept CLI/configuration differences and
per-type overrides. Strict decoding and a separate input decoder keep negative
contract tests connected to actual production validators. Offline package-delta
application reconstructs complete packages; read-only review comparisons follow
explicit renewal ancestry without inheriting approval. Embedded skills document
the full handoff. Existing examples need no migration; wire contracts, task
definitions and SQLite schemas are unchanged.

Fresh full workspace validation and all five native platform jobs gate publication.
Local evidence remains scoped in `docs/workflow-tools-validation.json` and
`docs/workflow-tools-followup-validation.json`; the accepted baseline is unchanged.

## v0.2.10: searchable JSON reports and parallel CLI guidance

CLI JSON reports and reduced verification GC exports use indented lines with a
trailing newline. This intentionally changes output whitespace so large reports
can be searched line by line; JSON values and internal hash/transport formats
are unchanged. Existing exports are not automatically rewritten.

Embedded skills document parallel dctx0 commands sharing a protected local
SQLite database, atomic worker claims and separate coordination of file edits.
Normal workspace validation and all five native distribution gates apply.

## v0.2.11: compact verification, public SQL and minimum storage

Verification JSON defaults to compact receipts; full consumers explicitly use
`--full` or atomic file export. Public SQL views expose task/evidence/case metadata
and pinned spec text with scalar parameters, joins, aggregates, historical pins
and enforced read-only resource limits.

Schema 7 shares immutable compressed history while preserving all task plans,
events and historical PASS. Explicit copy conversion and verified installation
retain a rollback backup; ordinary reads never migrate or repair data. Run/case
metadata identifies detail availability after retention. Embedded skills share
one query guide. See the query and verification-storage references for contracts.

The Company Manager comparison measured a 93% database reduction and identical
SQL results/full exports, with 83–98% faster SQL workloads. Local coverage reaches
97.17% source lines and 97.01% LLVM functions; Go Race is unavailable on the local
ARM64 host, so no full local aggregate PASS is claimed. Normal tag CI and five
native platform gates remain mandatory. The accepted baseline is unchanged.

## v0.3.1: delivery workflow and experimental test analysis

Five bundled skills now include `dctx0-delivery` for usable production packages,
change-aware context reuse, validation cadence and measurable alpha improvements.
The configuration skill and opt-in Rust/libtest coverage/test-analysis commands
are included alongside state-clause, nested-call and corpus-generation fixes.
Experimental test candidates do not authorize skipping other mandatory tests.
Main SQLite and SDK versions remain unchanged; full release gates still apply.
