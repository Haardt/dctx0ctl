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

**Managed native verification and automatic adoption currently require Linux.** macOS/Windows distribution CI runs actual CLI, LSP, installation, Bun SDK transport and test-process cancellation checks, and verifies that unsupported managed execution never reports PASS. SDK transport execution does not establish complete managed platform support.

## Pin a version

```sh
curl -fsSL https://github.com/Haardt/dctx0ctl/releases/download/v0.1.0/install.sh \
  | bash -s -- --version v0.1.0 --bin-dir "$HOME/.local/bin"
```

```powershell
& ([scriptblock]::Create((irm https://github.com/Haardt/dctx0ctl/releases/download/v0.1.0/install.ps1))) -Version v0.1.0 -BinDir "$env:LOCALAPPDATA\DCTX0\bin"
```

The executable includes the CLI, language server, target profiles, agent skills and testing templates. Releases contain four tar archives, one Windows ZIP, both installers and `SHA256SUMS`. Archives include the MIT license and a source/toolchain receipt. The manifest covers every archive and installer.

## Publish

A pushed `v<SemVer>` tag matching the Cargo workspace version triggers publication after full validation and all five distribution jobs pass. Stable tags become the latest release; prereleases are marked explicitly and skipped by the default installer.

```sh
git tag -a v0.1.0 -m "DCTX0 v0.1.0"
git push origin v0.1.0
```

Tag the reviewed, committed version and matching lockfile. The installer URLs become available after the first successful release. CI uses GitHub's native runners; systemd memory limits apply only on Linux. Local distribution validation records test results and packaging/startup speed/RAM in the latest 20 measurement reports.

See the repository's [release maintainer guide](https://github.com/Haardt/dctx0ctl/blob/main/docs/site/src/reference/releases.md) for commands, provenance and platform boundaries.
