# Adaptive specification verification

Decision accepted 2026-10-10. This document separates the intended architecture
from implemented capabilities. The compiler, profiles and task workflows remain
compatible. Verification projects validated IR; it never alters AST/IR, semantic
fingerprints, snapshot fixtures or immutable task definitions.

## Intended architecture

Production uses its own types, frameworks and conventions. Introducing dctx0
requires neither a runtime import nor markers, generated DTOs, DI library, folder
layout or global application factory. Default generation creates harnesses,
mocks and testing artifacts only. Production generation is a deferred opt-in.

A reviewed, versioned project adapter under
`.dctx0/testing/typescript/adapter.ts` connects actual application code to the
harness. It may translate representations, provide instances and replace declared
collaborators; it must not reproduce business logic or expected answers. Setup
preserves existing files. Scaffolds explicitly report unavailable entry points.
Framework integrations provide testing access without prescribing production
architecture. Hono routes are explicit fixture/application wiring, never inferred
publication of internal surfaces. Awilix tests must respect registration order
and lifetimes.

Core owns lossless values, invocation/observation and capability/provenance
contracts. `dctx0-interpreter` evaluates typed expressions without I/O. Operation
calls suspend evaluation and resume from observations supplied by orchestration.
`dctx0-verification` owns plans, obligations, generation, seeds, shrinking, replay
and Rust's PASS/FAIL/UNKNOWN decision. Targets must not implement DSL semantics or
supply a trusted success flag. `dctx0-target-sdk` holds the versioned execution
contract separately from the unchanged `TargetProfileProvider`.

Targets inspect engine-supplied inventories using Tree-sitter TypeScript/TSX,
generate harness artifacts and describe execution recipes. Inspection reports
public declarations/imports/exports and ambiguous candidates, not semantic type
proofs or a complete call graph. LSP and an additional TypeScript Compiler API
analysis layer are out of scope. Normal compiler/build tools remain permitted.
Only engine adapters write files and launch processes. Store owns immutable runs
and atomic adoption; tasks owns pure eligibility and obligation mapping.

## Evidence invariants

1. Production imports no dctx0 runtime, spec or generated testing artifact.
2. Introduction does not require changing the application's architecture.
3. Observed behavior never changes the agreed specification to make a test pass.
4. Evaluation, generation, expectations, seeds, shrinking and replay stay in Rust.
5. Adapters call real implementation code and cannot replace business logic.
6. Mocks replace declared collaborators; consumer evidence does not verify the
   real production port adapter.
7. Static calls do not establish runtime counts. Opaque traces, dynamic unresolved
   relationships and absent observers remain UNKNOWN.
8. Narratives explain requirements; they do not create assertions automatically.
9. Reports name selected/unselected subjects, mandatory checks, executed cases,
   unsupported requirements and incomplete sections. A passing slice is scoped.
10. Provenance binds spec/check occurrence, implementation and dependencies,
    adapter/configuration, plan/cases/seed/budgets, harness, versions, runtime,
    build configuration and frozen profiles.
11. Missing mandatory cases, empty valid corpora, exhausted resources and absent
    capabilities prevent complete evidence; explicit examples are never dropped.
12. PASS covers all mandatory checks in the reported finite plan, not all inputs
    or free-form architectural notes.
13. Task definitions are immutable; results and work changes are append-only.
14. Adoption only handles eligible active Open, unreserved, unclaimed tasks with
    complete evidence and no unmet prerequisites. Historical Done/PASS survives.
15. Adoption of a slice is incremental; inaccessible areas remain open without
    requiring an application-wide redesign.

UNKNOWN is a verification result, never an additional task work state. Tasks
retain Open/Progress/Done and separate Unverified/PASS/FAIL testing evidence.

## Stages and acceptance gates

0. Record this decision and repository guidance.
1. Language-neutral values, pure expression evaluation and plan projection:
   builtins, aliases, records, variants/options/lists, modeled numeric/UUID
   refinements, pure calls, resolvable examples, invariants/forall, pre/postconditions.
   Unsupported expressions retain source spans and UNKNOWN reasons. Reference
   semantics live in `../runatron/packages/plugin-dctx0/src/dctx0/property-evaluation`
   and `property-testing`; differences must be documented.
2. TypeScript/Bun Tree-sitter inventory and generated harness. One spec and plan
   must work against both a direct pure export and a differently named Awilix
   service in Hono. Freeze production fixtures before adapters. Mutate business
   logic and actual HTTP wiring. Run FilmId, ReleaseYear and candidateOrderWinner
   from Cinema; retain independent handwritten reference tests.
3. Adaptive CLI and full provenance: `verify discover/init/plan/run/show/replay`,
   selectors by task or qualified subject and `run --no-adopt`. Discovery executes
   nothing. Empty configured scope never means the entire project. Use published
   snapshots; changed working specs require successful check first. Historical
   runs cannot adopt current work. Replay records changed environment comparisons.
4. Additive SQLite writer schema 4; read-only schema 1–4 readers. Persist runs,
   artifacts, mappings and idempotent adoption receipts. Legacy record-check stays
   compatible but cannot authorize adoption. A technical verifier actor publishes
   start/complete/check events and profile binding in one guarded transaction.
   Successful producers can be delivered topologically in the same transaction.
   Reject stale plans/check occurrences/events/sources/profiles, claims and
   reservations without losing the executed run. Review/retirement are excluded;
   integration/production adapters require actual dedicated complete checks.
5. Port `given` scenarios, centrally evaluated real call events/counts/arguments,
   controlled effects and properly scoped Hono/Awilix sessions. Only supported
   observers can establish reads/writes/emits/state, time/random/ID control.
6. Before/after states and lifecycle sequences with fresh independent sessions,
   prerequisite-preserving shrink/replay, achieved/open transition reporting.
   Then Java/Spring, Go and Rust use the same conformance contract. Spring wiring,
   proxies, transactions and security require actual ApplicationContexts.

Complete runatron-next native acceptance and full application implementation are
later goals. Both complete specs remain mandatory compiler acceptance cases.

## Determinism, protocol and resources

Implemented defaults: seed 42; 100 generated cases per obligation; collection size 4;
recursion depth 8; canonical value size 64 KiB; 100 shrink steps. Explicit examples
come first, followed by boundaries, small exhaustive domains and deterministic
bounded samples. Bounded selection is reported; aborted mandatory execution is
UNKNOWN. Integer and decimal representations are lossless; unsupported native
representations must be rejected explicitly.

The implemented versioned JSON-Lines protocol distinguishes invocations, responses,
observations and technical failures. Application logs cannot masquerade as
protocol output. Limits: initialization 30s, case 5s, run 120s, logs 256 KiB,
message 1 MiB. Separate managed native processes are resource controls, **not a
security sandbox**. Timeout/cancellation terminates managed children and reaps
processes. Every case creates and cleans a fresh session. Missing runtime or
capability is UNKNOWN; violated executed contracts/application exceptions are
FAIL; build, adapter and protocol errors remain distinct. Failed setup/cleanup
cannot produce PASS. Linux is the first supported process platform.

Only identical relevant provenance permits cache reuse; cached evidence points
to the original execution and does not invent a new duration. Verification reports
start at schema 1; existing task reports remain schema 2.

## Validation and performance

Use `PATH=/home/held/.cargo/bin:$PATH python3 scripts/validate.py --coverage`.
Maintain 100% merged Rust source lines and LLVM functions, preserving raw LLVM
metrics; report native helper/generated harness coverage separately. Test limits,
unsupported expressions, bad representations, determinism, replay/shrink,
mutation detection, protocol corruption, timeout/cancellation/cleanup and
transactional adoption races. Do not refresh compiler goldens to hide regressions.

Add separate release measurements for discovery, planning, generation,
initialization, execution, mocks, shrink, replay and adoption as those capabilities
arrive. Separate Rust and native costs and cold/warm workloads. Preserve the last
20 runs and accepted baseline. New workloads stay unassessed without explicit
baseline promotion. The earlier runtime deviation is not claimed resolved.

## Current implementation (2026-10-11)

See [verification-foundation.md](verification-foundation.md) for implemented APIs,
reference limitations and measured core workloads.

The Rust interpreter and verifier provide wire values, budgeted typed expression
evaluation, suspended real calls, deterministic input/rejection generation,
IR-to-plan projection, shrinking and replay. The TypeScript target provides
Tree-sitter inspection and a generated Bun worker. Engine owns managed native
processes, captured sources, observed mocks/effects/states and coherent lifecycle
sessions. The six verification CLI commands and immutable SQLite schema-4
history are implemented. The private native execution path supports guarded
automatic adoption; imported reports remain evidence without that authority.

Current complete Cinema evidence records all 5,552 mandatory native cases PASS
and complete captured provenance. A whole-plan integration run adopts 41 fully
proved tasks while retaining 63 Open/Unverified tasks. These facts do not close
manual reviews, free-form constraints or named proof/test suites, whose
executable meaning remains unsupported. The complete report is UNKNOWN.
Store independently checks task requirement-to-case mappings and re-evaluates
discharged state-precondition hints from pinned snapshots, immutable plans and
actual before-state observations inside the delivery transaction.

See [current scope and remaining gates](cinema-verification-progress.md) and
whole-plan adoption evidence (historical evidence is available in Git history).
Final full workspace coverage and release speed/RSS assessment are still open;
the accepted benchmark baseline is unchanged. Other language targets retain
their own capability claims and cannot inherit TypeScript's native acceptance.
