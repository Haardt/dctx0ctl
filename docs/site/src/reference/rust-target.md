# Rust native verification target

The Rust target executes the same language-neutral plan as TypeScript and Go.
Tree-sitter Rust supplies structural candidates; Cargo supplies native compilation;
the shared Rust interpreter/verifier owns input generation, expectations, verdicts,
shrinking and replay. Production stays independent of dctx0.

## Adapter and worker

`verify init --target rust` creates testing artifacts under `.dctx0/testing/rust`:
a generated transport/contract, an unavailable adapter scaffold and an editable
Cargo manifest. Repeated setup preserves authored files. Add actual application
dependencies to that manifest and explicitly prepare its Cargo.lock. The adapter
implements `describe`, `begin` and actual `Session` entry points. The transport
supports value checks, real state/event/resource observations and declared mock
collaborators. Only actual application boundaries catch application panics;
adapter/translation errors remain technical errors, never worker PASS flags.

Every case receives a fresh session. Logs/protocol, message and trace bounds,
cleanup and managed-process limits apply. State and collaborator access require
real adapter capabilities; static imports do not establish runtime calls.
The first native implementation requires Linux and an available Rust/C toolchain.
This is process/resource management, not a security sandbox.

## Cargo evidence

Managed Cargo commands are offline/locked and use two build jobs with uninstrumented
native release code. Build tools, Cargo-selected package sources/manifests,
standard libraries and the supported host C toolchain/header closure are hashed.
Rust independently reassesses those inputs and binds the compiled executable.
Local package dependencies must remain inside the captured application. Compiler
dep-info is checked against the pre-build inventory; external included sources
prevent full evidence. The initial host C closure supports the default GNU
toolchain, including multiarch headers. Custom CC/CFLAGS and compiler overrides
remain unsupported instead of being silently treated as pinned.

An owned Cargo cache under XDG_CACHE_HOME (or HOME/.cache) avoids repeating native
dependency compilation. A cancellable bounded exclusive lease covers compilation
and binary capture. Workers execute the content-hashed archived executable under
application `.dctx0ctl/native/rust`, never a subsequently overwritten cache binary.
The build receipt identifies actual toolchain, packages, inputs and executable;
working-tree/source changes prevent automatic adoption. Native build and run costs
remain separate from central finite assessment.

Profiles, SDK protocol version 1, compiler IR/snapshot goldens and SQLite formats
remain compatible. Java retains profiles without a native execution claim.
Rust discovery reports public declarations and import syntax, including ambiguity;
macros are not expanded and no semantic type/call-graph proof is claimed.

## Cinema and acceptance

`examples/cinema-rust` contains the byte-identical Cinema spec, independent contracts,
constructor-injected services, real SQLite persistence with composite keys/upserts
and voting transactions, and twelve explicit HTTP routes with a production server.
The adapter translates actual values and observes actual operations; it does not
reimplement expected business results.

```sh
cargo fetch --manifest-path examples/cinema-rust/.dctx0/testing/rust/Cargo.toml --locked
dctx0 verify complete examples/cinema-rust/dctx0 --project examples/cinema-rust
```

Independent tests cover exact large integers, Unicode trim boundaries, candidate
order, changed ratings/votes, durable reopen and real HTTP errors/concurrency.
The acceptance script compares target-neutral plans, requires all 5,674 mandatory
cases and 104 tasks, introduces actual winner/persistence/lifecycle/HTTP/collaborator
mutations and checks broken/repaired replay. Manual requirements remain explicitly
source-reviewed, separate from native UNKNOWN.

The final grouped validation passed all 22 recorded commands on an unchanged
source tree. Merged Rust source lines are 27,315/27,889 (97.94%); LLVM functions
are 2,178/2,241 (97.19%). Both exceed the approved 97% gate. Raw LLVM instance
metrics remain separate; this is not complete branch or native-helper coverage.

The isolated acceptance passed all 5,674 mandatory cases and completed 104 tasks:
46 by guarded native adoption, 58 through explicitly source-bound inspections.
Both specs and semantic plans match the TypeScript reference. All five actual
production mutations were detected; broken/repaired replay produced FAIL/PASS.
The 25 native manual/suite obligations remain UNKNOWN; authored review is explicit,
source-bound and cannot override native failures.

Performance ran last, retaining 66 release workloads in total, including ten Rust
phases. On this machine the complete finite native run took 8.76/8.82 seconds
for first-project/repeated builds, with existing Cargo/toolchain caches. The
worker reported 7.02/7.10 MB peak RSS. The separately managed build took about
3.2 seconds; the largest reaped CLI/build process peaked around 258 MB, and
complete verification plus adoption took 10.16 seconds. Process RSS is not summed
simultaneous memory. These are single samples, not cold-cache guarantees.

Cargo cache reuse is compilation reuse, not a cached verification PASS. Every run
executes the mandatory cases and pins its own compiled binary. Worker hashes can
differ across captured crate paths; both hashes and actual inputs are retained.
New workloads and reduced sampling remain unassessed against the accepted
baseline, which was not promoted or changed.

Run the same grouped validation, with measurements at the end:

```sh
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 python3 scripts/validate.py \
  --coverage --example cinema --samples 1 --iterations 1
```

Retained acceptance, coverage and measurements (historical evidence is available in Git history)
include the exact commands, source hashes, full finite scope and benchmark record.
