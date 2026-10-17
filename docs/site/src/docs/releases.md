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

## v0.2.5 candidate: bounded verification storage

Verification history retains three complete runs per project and compact historical
summaries for older runs. Cases are compressed once, large artifacts are compressed
when beneficial, and successful cases omit duplicate diagnostic journals. New
`verify history` and `verify gc --compact` commands expose retention and explicit
maintenance without a backup. SQLite writers migrate additively to schema 5;
older CLI versions cannot read that schema. Task history and old verdicts remain,
while pruned evidence cannot authorize replay or fresh adoption.

This candidate requires fresh workspace and five-platform release checks; the
v0.2.4 test exception does not apply. See [verification storage](../reference/verification-storage.md)
for compatibility, commands and measured maintenance results.

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
