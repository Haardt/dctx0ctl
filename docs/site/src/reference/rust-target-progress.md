# Rust execution target acceptance, 2026-10-12

The Rust target and independent Rust Cinema application are implemented and
accepted. The unchanged Cinema spec executes 5,674 mandatory cases; complete
verification finishes 104 tasks (46 native adoption, 58 source-bound inspection).
All five production mutations and broken/repaired replay pass the acceptance.
Native manual/suite requirements retain UNKNOWN and authored review remains explicit.

The final frozen-source grouped workspace run passes all 22 commands, including
feature combinations, TypeScript/Go compatibility, Rust native execution, lint,
independent SQLite/HTTP/concurrency tests and instrumented coverage. Rust source
lines are 97.94%, LLVM functions 97.19%, above the approved 97% gate. This is not
full branch/native-helper coverage. Source hashes match before and after checks
and measurements.

Performance ran last: 66 workloads, including ten Rust phases. Full finite native
verification takes about 8.8 seconds with existing caches, complete/adoption about
10.2 seconds; the actual worker peaks around 7 MB RSS. Build/CLI costs are recorded
separately. Reduced single-sample measurements/new workloads remain unassessed;
the accepted baseline is unchanged.

See [implemented contract, commands and limits](rust-target.md) and
retained evidence (historical evidence removed during repository cleanup). Remaining capabilities outside
this increment include Java native execution and additional framework/toolchain
integration; they are not claimed by this Rust Cinema acceptance.
