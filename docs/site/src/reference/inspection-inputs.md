# Large source-bound review inputs

`verify inspect`, `verify attest` and `verify complete` read JSON incrementally with
one shared engine-owned reader. Their default wire limit is **64 MiB**. Set
`--max-input-bytes N` to change it; positive byte counts below `u64::MAX` are
accepted. The limit includes whitespace and trailing bytes. Oversized or malformed
JSON is rejected before publication. Regular files are checked before parsing;
the reader also enforces the bound if input grows during reading.

```sh
dctx0 verify inspect spec NATIVE_RUN --project . --file reviews.json
dctx0 verify inspect spec NATIVE_RUN --project . --file reviews.json --max-input-bytes 134217728
dctx0 verify attest spec NATIVE_RUN --project . --file attestations.json --max-input-bytes 134217728
dctx0 verify complete spec --project . --inspections authored-notes.json --max-input-bytes 134217728
```

Inspection groups remain JSON arrays of complete `ProgramInspection` records.
There is no implicit accumulation: submit the complete intended group against one
retained native basis. An imported effective report cannot become a new native
basis. Unknown obligations, duplicate subjects and mismatched snapshot, plan,
requirements or implementation vectors are rejected atomically. Native failures,
missing/duplicate cases and incomplete provenance cannot be promoted to PASS.
Unreviewed manual obligations remain UNKNOWN. Source judgements remain separate
from automatic observations and do not automatically adopt tasks.

Rust callers use `read_review_input`, `DEFAULT_REVIEW_INPUT_BYTES` and, when a
custom notes bound is needed, `Project::complete_verification_with_input_limit`.
The existing `complete_verification` method uses the same default. These resource
limits do not truncate mandatory contracts or native evidence.

## Reproduce the acceptance and measurements

```sh
python3 scripts/inspection_acceptance.py
```

The runner builds an uninstrumented release CLI and runs actual Bun native
verification against 543 production modules. Groups of 19 and 47 bound reviews
exceed the former 1 MiB cap. It checks every submitted effective outcome, retains
an unreviewed UNKNOWN, and rejects invalid bindings, conflicting duplicates,
malformed input, resource overflow and invalid native evidence. Separate complete
CLI fixtures also use authored notes larger than 1 MiB.

`benchmarks/history/inspection/20261017T095416.009809Z-20553.json` completes both
command stages against unchanged inputs. The 19-review group is 1,497,179 bytes:
93.13 ms and 21,987,328 peak RSS bytes. The 47-review group is 3,703,635 bytes:
122.58 ms and 30,572,544 peak RSS bytes. These expected UNKNOWN CLI results reflect
an intentionally unreviewed obligation, not failed input processing. Every
submitted review is separately checked as effective PASS over its fixture.

Memory is Linux `wait4` process peak RSS for each actual release CLI invocation;
inspection invokes no compiler or native worker. Compiler/test orchestration and
Bun fixture costs are recorded separately. These are single-sample development
measurements with warm toolchain caches, not an accepted regression baseline.
Latest 20 reports are retained transactionally under `benchmarks/history/inspection/`.
The groups are acceptance fixtures, not professional reviews of Company Manager
or claims that its remaining implementation gaps are resolved.

The first corrected-candidate CI completed all individual workspace checks and
reported 97.56% merged source-line and 97.09% LLVM function coverage. Its aggregate
result correctly failed the source-change guard: recursive harness JSON discovery
had included Cargo `target` fingerprints created by the native tests. The input
inventory now excludes `target` and `node_modules` cache directories while keeping
authored harness JSON and embedded UI assets. A regression test checks both cache
creation stability and detection of an actual configuration change. Fresh complete
CI remains required before publication; no coverage or performance gate is waived.
