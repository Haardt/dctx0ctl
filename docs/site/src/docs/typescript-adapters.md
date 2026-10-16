# TypeScript adapters

The connection to TypeScript is an external **testing adapter**, separate from the production application. The application owns its types, dependencies, composition root, and framework integration.

Technical configuration lives in `.dctx0/testing/verification.json`. The generated adapter contract defines the supported test interface.

## Testing area

```text
.dctx0/testing/
├── verification.json
└── typescript/
    ├── adapter.ts
    ├── contract.ts
    ├── examples.json
    └── worker.ts             # generated testing transport
```

`verify init --target typescript` creates missing artifacts while preserving authored files. Its unavailable scaffold deliberately cannot certify an application.

## Adapter responsibilities

The adapter describes its capabilities, starts isolated sessions, calls real production entry points, translates values, and exposes actual results and observations through the SDK contract. Read the generated `contract.ts` and the [Cinema adapter source](https://github.com/Haardt/dctx0ctl/blob/main/examples/cinema/.dctx0/testing/typescript/adapter.ts) for the current versioned types.

Independent cases receive fresh sessions. Lifecycle steps share a session. Declared collaborator replies can be injected through real dependency boundaries. Calls, events, resources, and states must come from actual observers; imports alone do not establish runtime behavior.

Do not reconstruct the expected business result in the adapter. That would test a second implementation of the specification rather than the application.

## Wire values and failures

Translate application-owned values to the language-neutral SDK representation. Preserve exact numbers and tagged outcomes. Unsupported native representations should be reported explicitly rather than rounded or guessed. Application exceptions and adapter/translation failures have different evidence meanings.

Examples require concrete typed values for symbolic inputs. These fixtures are separate from expected behavior: the specification supplies the assertions.

## Structural discovery

```sh
dctx0 verify discover <spec-root> --project <app-root> --target typescript
```

Tree-sitter helps inventory exports, imports, and candidate boundaries. It does not bind them automatically or prove semantic type conformance. Review candidates and wire the actual entry points in your adapter.

## Configuration and execution

```sh
dctx0 verify init <spec-root> --project <app-root> --target typescript \
  --subject <qualified-name>
dctx0 verify plan <spec-root> --project <app-root>
dctx0 verify run <spec-root> --project <app-root> --no-adopt
```

The configuration selects explicit subjects/tasks and deterministic budgets. An empty scope is not whole-application verification. Prepare Bun dependencies before execution.

For real storage and transport assurance, include persistence/HTTP checks. Mock replies establish consumer behavior under those replies, not the production provider's correctness. See [evidence](evidence.md), [workflow](workflow.md), and the [SDK/harness guide](typescript-adapters.md).

## Native dependency budget

Set the native resolver budget in `.dctx0/testing/verification.json`, independently of corpus `budgets` and process `execution` deadlines:

```json
"dependency_resolution": {
  "max_references": 10000
}
```

The default is **10,000 references**; supported integer limits are **1 to 1,000,000**, inclusive. Missing configuration uses the default. New `verify init` configurations write it explicitly; existing files are preserved. Zero, negative/fractional values, overflow, out-of-range values and unknown budget fields are rejected before application capture or worker launch. No CLI or environment override is applied.

Each reachable literal reference consumes one unit: repeated imports, cached results, builtins and unresolved specifiers all count. Dynamic expressions without a literal specifier retain their independent unresolved diagnostic. Exactly N references fit a budget of N; an additional reference triggers `DEPENDENCY_RESOLUTION_BUDGET`, naming the limit, processed count and first unprocessed source range. Previously recorded observations remain saved; provenance stays incomplete and automatic task adoption remains blocked even when all behavioral cases pass.

A run-local cache reuses native replies only for the same importer, literal specifier and import/require mode. Every reference still has an individual resolved-dependency entry and any required diagnostic. Failed native resolutions are cached without hiding their repeated occurrences; protocol/transport errors stop traversal. Cycles are visited once per source. Cancellation and run deadlines remain active on cache hits. Capture, output and process limits remain independent safeguards; raising this budget does not promise a complete run within them.

The run saves versioned `application/vnd.dctx0.dependency-resolution-limits+json` evidence with the effective maximum and resolver version, including omitted defaults. `application/vnd.dctx0.dependency-resolution-summary+json` records processed references, worker requests, cache hits, traversal completion and stop reason. Traversal completion means all selected references were considered; unresolved dependencies can still leave provenance incomplete. The structural inspection, dependency scope and resolved-dependency artifact formats remain unchanged. Go and Rust retain their compiler-selected closure verification.

Replay resolves dependencies again using the current application's configuration and reports a separate `dependency-resolution-limits` change alongside ordinary configuration changes. Historical runs without this artifact stay readable; their missing evidence is not retroactively filled. Replay never adopts tasks. Effective resolver limits participate in the native task implementation identity. Saved task definitions and previous results remain immutable.

Rust callers constructing `VerificationConfiguration` directly must initialize the new `dependency_resolution` field with `None` for the default or `Some(DependencyResolutionBudget { max_references: ... })` for an explicit limit. Saved verification-run schemas remain unchanged.

The reference-budget default intentionally increases from the former fixed 1,000 to 10,000. Set `max_references` to 1,000 explicitly when the previous boundary is required. No database migration is needed.

The contributor command `python3 scripts/dependency_resolution_acceptance.py --binary target/release/dctx0` couples real chain/cycle, repeated-reference and budget-stop checks to release measurements (one warmup and three samples). It is included in the final performance stage of `scripts/validate.py`. Reports retain whole-command elapsed time, managed-run elapsed time, sampled direct-engine VmHWM and worker-reported VmHWM separately. Both elapsed times include worker calls; they are not pure Rust CPU timings. Pure Rust parsing, compilation and generation remain separate workloads in the main suite. Sampling can miss the final process-exit interval, and unsupported memory values are null. Latest-20 reports are retained under `benchmarks/history/dependency-resolution/`; these new workloads have no accepted baseline.
