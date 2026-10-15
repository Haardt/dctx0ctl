# Native verification portability — acceptance pending

The executor uses the same protocol, size budgets and finite-plan assessment on
Linux, macOS and Windows. Auto retains Linux process-tree supervision; other
hosts use direct-child kill/wait and bounded stream draining. Rust callers can
select `NativeSupervision::Child` to exercise that backend on Linux. A stream
that remains open after cleanup produces incomplete execution, with bounded
reader cancellation. A missing mandatory reply or native result still prevents
PASS. Optional process-tree supervision and unavailable worker RSS do not.

`verify run --json` adds optional `supervision` capability metadata; saved runs
retain `application/vnd.dctx0.native-supervision+json`. Successful audits also
report capabilities. No SQLite migration is involved. Replay keeps its separate
run identity and never adopts tasks. Review completion remains distinct from
native testing PASS.

Runtime identity schema 1 remains readable and identifies Linux procfs live-image
observations. Schema 2 explicitly identifies `captured-launch`: the resolved
program path is launched directly, and its bytes, arguments, canonical working
directory and effective inherited/overridden environment digest are retained.
Environment names/values are not written to evidence. Digests use lossless native
OsStr bytes, sorted NUL-delimited entries; Windows override names are compared
without ASCII case. Ephemeral supervisor ownership tokens are excluded. The
executable and parent environment are checked again before publication, alongside
existing captured sources, compiler inputs and Go/Rust build-receipt guards.
This method is a launch receipt, not a kernel-observed executable image.

Go uses host executable suffixes and tool paths; compiler peak RSS uses Unix
Rusage when available (Linux KiB, macOS bytes), otherwise null. Rust captures host
Rust tools and standard libraries, GNU/Linux headers, macOS Xcode SDK/compiler
headers or MSVC binaries/INCLUDE roots. Windows needs the Visual Studio developer
environment; native CI initializes it. Dependencies must be prepared before
locked/offline builds. Windows checkouts retain LF bytes for authored reviews.

Run on each native host after building an uninstrumented all-feature release CLI:

```sh
python scripts/native_portability.py --binary target/release/dctx0 --coverage
# Windows: --binary target/release/dctx0.exe
```

The runner requires Bun, Go, Rust, Python and the host C toolchain. It passes an
absolute Python interpreter to portable process fixtures; production has no
Python dependency. `--transport-only` explicitly records partial development
validation, still with release measurements. Each run retains logs, raw samples,
toolchains, platform, binary/source hashes and revision. Native build audits and
worker metrics are retained separately in Cinema reports. Peak memory uses the
portable distribution runner's named wait4/PeakWorkingSetSize methods; null means
unavailable. These are not collective simultaneous RSS or Rust-only allocation
measurements. There is no accepted portability baseline. Existing baselines are
unchanged. History uses atomic writes/locking and keeps the latest 20 runs.

The five existing native release jobs now run on relevant PRs and workflow
dispatch, and gate new releases. Their final check requires one complete report
from each named target. Failed logs and native platform coverage are archived.
Native coverage is scoped to portable transport and compiled-target tests; Linux
workspace coverage separately retains the approved 97% source-line/function gate
via `PATH=/home/held/.cargo/bin:$PATH python3 scripts/validate.py --coverage`.
Cross-compilation or Linux coverage does not establish macOS/Windows acceptance.

Cinema completion reuses source-validated authored inspections and persisted task
reviews: 105 tasks, four review tasks Done/Unverified, 5,674 mandatory native cases
per target. Older scripts that required every task PASS now use that workflow.
No new architectural approvals are invented. Historical validation remains
historical; final native matrix evidence is still required before support claims
or release notes are broadened.
