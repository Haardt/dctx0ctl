# Releases and installation

A pushed **`v<SemVer>` tag creates a GitHub Release** after full validation and all five native distribution jobs pass. Branch, PR and manual runs test packaging without publishing. Stable tags become the latest release; prereleases are marked explicitly and excluded from the default installer.

Runs for the same Git ref supersede older in-progress runs. Different version tags run independently. Correcting an unpublished tag therefore cancels its obsolete build; published release tags should remain immutable.

## Install DCTX0

On Linux or macOS:

```sh
curl -fsSL https://github.com/Haardt/dctx0ctl/releases/latest/download/install.sh | bash
export PATH="$HOME/.local/bin:$PATH"
dctx0 --version
```

On Windows x64, in PowerShell:

```powershell
irm https://github.com/Haardt/dctx0ctl/releases/latest/download/install.ps1 | iex
$env:PATH = "$env:LOCALAPPDATA\DCTX0\bin;$env:PATH"
dctx0 --version
```

| Platform | Native target | Runner | Package |
| --- | --- | --- | --- |
| Linux x64, glibc 2.35+ | `x86_64-unknown-linux-gnu` | `ubuntu-22.04` | `.tar.gz` |
| Linux ARM64, glibc 2.35+ | `aarch64-unknown-linux-gnu` | `ubuntu-22.04-arm` | `.tar.gz` |
| macOS Intel, 14+ | `x86_64-apple-darwin` | `macos-15-intel` | `.tar.gz` |
| macOS Apple Silicon, 14+ | `aarch64-apple-darwin` | `macos-14` | `.tar.gz` |
| Windows x64 | `x86_64-pc-windows-msvc` | `windows-2022` | `.zip` |

The shell installer needs Bash, curl, tar, and `sha256sum` or `shasum`. It detects architecture, enforces the Linux glibc/macOS minimum, resolves the stable tag once, checks the archive's checksum and executable version, and atomically installs to `~/.local/bin`. The PowerShell installer streams only `dctx0.exe` from a checksum-verified ZIP and atomically installs to `%LOCALAPPDATA%\DCTX0\bin`. Both preserve an existing installation on download/checksum/version failure and reject non-regular destinations. Neither changes shell profiles or persistent PATH settings.

**Install your application's toolchains and dependencies yourself** before native verification: Bun for TypeScript, Go for Go, or Rust/Cargo and a C compiler for Rust. The installer supplies DCTX0. Compiling specifications, planning tasks and using the LSP require only the binary.

**Managed native verification and automatic adoption currently require Linux.** The macOS/Windows checks establish CLI/LSP/installation and testing SDK transport behavior, plus cancellation of owned test processes. They explicitly verify that unsupported managed execution cannot return PASS or adopt tasks. This does not claim a Windows/macOS port of the Linux process-group and `/proc` provenance machinery.

## Pin a version or destination

```sh
curl -fsSL https://github.com/Haardt/dctx0ctl/releases/download/v0.1.0/install.sh \
  | bash -s -- --version v0.1.0 --bin-dir "$HOME/.local/bin"
```

```powershell
& ([scriptblock]::Create((irm https://github.com/Haardt/dctx0ctl/releases/download/v0.1.0/install.ps1))) -Version v0.1.0 -BinDir "$env:LOCALAPPDATA\DCTX0\bin"
```

Both accept versions without the `v` prefix. `DCTX0_INSTALL_DIR` supplies a default destination; the explicit option overrides it. `DCTX0_REPOSITORY=owner/repository` supports forks and must be set in the installer's environment.

The executable embeds the CLI, LSP, target profiles, agent skills and testing templates. Each archive contains `dctx0` (or `dctx0.exe`), `LICENSE`, `README.txt`, and `release.json`. The receipt identifies tag, architecture, source revision, toolchain, features and executable SHA-256. Tar/ZIP archives have normalized timestamps, permissions and member order. `SHA256SUMS` covers all five archives and both installers. Checksums are transfer checks, not a separate publisher signature.

Public URLs become available after the first successful tag publication. No tag or release is created by local validation.

## Publish a reviewed version

1. Set the workspace version in `Cargo.toml` and update `Cargo.lock`.
2. Commit the reviewed code, documentation, installer and workflow. Validate that checkout before tagging.
3. Create and push the matching tag:

```sh
git tag -a v0.1.0 -m "DCTX0 v0.1.0"
git push origin v0.1.0
```

Tag/version mismatches stop before builds. Prereleases such as `v0.2.0-rc.1` must match Cargo exactly. Numeric prerelease identifiers cannot have leading zeros.

[The release workflow](https://github.com/Haardt/dctx0ctl/blob/main/.github/workflows/release.yml) uses native runners, Python 3.12, stable Rust and Bun 1.4.2. macOS builds explicitly target macOS 14. The optional systemd memory guard runs **only on Linux**. Publishing depends on [full Linux validation](https://github.com/Haardt/dctx0ctl/blob/main/.github/workflows/validate.yml) and every distribution job. Only the publishing job has `contents: write`; it creates the GitHub Release with exact assets, generated notes and `--verify-tag`. Existing releases are not silently overwritten.

## Local validation and measurements

```sh
cargo build --release --locked --all-features -p dctx0-cli --bin dctx0
python3 scripts/release_acceptance.py --cargo-tests --platform-tests
```

Distribution validation checks tag guards, ELF/Mach-O/PE architecture headers, reproducible archives, the exact seven-asset manifest, and actual installer behavior with deterministic network fixtures. Unix tests execute piped and pinned installation, updates, embedded skills and compilation, plus rejection and archive-path cases. Windows additionally runs the native PowerShell installation suite. Platform checks run CLI compilation, LSP diagnostics/shutdown, real Bun transport (calls, state, events and mocks), process-tree cancellation, and the explicit managed-execution boundary outside Linux.

Tests record elapsed time and memory, followed by one warm-up and three fresh-process release packaging/startup samples. Linux/macOS use `wait4` child/reaped descendant peak RSS; this can include inherited memory before exec. Windows polls the child's OS `PeakWorkingSetSize` and excludes descendants. Unsupported queries produce `null`, never zero. These metrics are not simultaneous collective RSS. The Bun platform report names its self-memory scope separately.

The latest 20 reports, including failures, are written atomically under `benchmarks/history/release/` with cross-platform locks, raw samples and provenance. New distribution workloads are explicitly unassessed against a baseline; the core baseline is unchanged. See retained local evidence (historical evidence removed during repository cleanup). Actual Windows/macOS execution occurs in configured CI and is not claimed by Linux-local evidence.

Runner labels follow [GitHub's runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners). Image-header validation follows [Microsoft's PE format](https://learn.microsoft.com/en-us/windows/win32/debug/pe-format) and [Apple's Mach-O definitions](https://github.com/apple-oss-distributions/xnu/blob/main/EXTERNAL_HEADERS/mach-o/loader.h).
