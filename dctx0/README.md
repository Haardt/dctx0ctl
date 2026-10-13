# DCTX0's own specification

The reviewed testing-only Rust adapter invokes actual compiler, evidence,
task/history/profile, native verification, CLI and LSP implementations.
`dctx0 verify self .` builds and executes an isolated captured source tree.
Architecture reviews stay separate from native PASS.

See [self-verification](../docs/site/src/docs/self-verification.md) for commands,
source/build receipts, bounded corpus, isolated state, report interpretation and
coupled mutation/performance acceptance.

## Inspect and plan

From the repository root, using a current source-built executable:

```sh
cargo build -p dctx0-cli --all-features --locked
./target/debug/dctx0 check dctx0
./target/debug/dctx0 verify plan dctx0 \
  --subject Dctx0.Compiler --subject Dctx0.Evidence
```

To inspect the full described product:

```sh
./target/debug/dctx0 verify plan dctx0 --subject Dctx0
./target/debug/dctx0 tasks plan dctx0 --project . --target rust
./target/debug/dctx0 tasks list dctx0
```

`check` validates and publishes this specification. `verify plan` projects
requirements and finite cases without executing the implementation. Task
planning creates work for the declared contracts and reviews; it does not mean
the existing implementation needs to be generated again. Generated work must
be bound to actual existing implementation/evidence before delivery.

## Execute

```sh
./target/debug/dctx0 verify self . --json
python3 scripts/self_verification_acceptance.py --checks --mutations
```

The full configured scope contains all eight product areas. The adapter creates
fresh projects for independent cases and retains session state only within ordered
examples. Task claims/versions, snapshots, check occurrences and native run IDs
come from real results. Guard rejections, failed publication, profile preservation,
profile adaptation, adoption, capability absence, mutations/replay and versioned
LSP observations are explicit examples.

`ApiInt` declares the actual Rust i64 representation range for test-facing APIs.
The corpus is finite and bounded; architecture and distribution reviews remain
explicit authored inspections. Native UNKNOWN is not rewritten into native PASS.
See the guide for source capture, build receipts, partial-scope reporting,
Linux process cleanup, review delivery and latest-20 performance evidence.

## Independence limits

The Rust compiler necessarily compiles this specification. That is useful
self-hosting coverage, but it is not an independent proof of compiler correctness.
Keep negative compiler fixtures, reference semantics, native target conformance
and mutation tests. A bug shared by the planner and verifier can escape a
self-test; independent expectations and review are still required.
