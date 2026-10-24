# Implementation and adapter reference

Use `profile <target> create --project <app-root> --default --agents` to explicitly
publish editable defaults when appropriate. Existing Markdown/AGENTS.md is preserved.
Use the actual pinned task/profile context rather than copying old target playbooks.
Profiles express implementation choices; they do not automatically become assertions.

Native targets currently are TypeScript with Bun, Go with its module toolchain,
and Rust with Cargo. Java profiles remain available without native execution claims.
Frameworks are optional integration choices; the external adapter must expose actual
entry points, state and effects required by the selected plan.

`verify discover <spec-root> --project <app-root> --target <target>` reads structure.
`verify init` preserves existing adapters and creates an explicitly unavailable
scaffold. Native runs do not fabricate success for an unimplemented scaffold.
Technical configuration selects target, runtime, adapter, fixtures, scope and budgets.
An empty scope is not a request to verify every application feature.

TypeScript uses `.dctx0/testing/typescript/adapter.ts`; Go uses a sibling Go adapter;
Rust uses a separate testing Cargo package with adapter.rs and an authored manifest/
lockfile. Prepare dependencies explicitly before managed offline native builds.
Preserve exact numbers and report unsupported native representations rather than
rounding. Each independent case starts fresh; lifecycle actions share one session.
Record actual calls, events, state and accesses through real observers.

For TypeScript representation boilerplate, use `verify scaffold-codecs <spec-root>
--project <app-root> --target typescript --subject <qualified-name> --integer number
--option null --enum string`. A published current check is required. Pass
`--type <qualified-type>=bigint` (or another supported representation) for per-type
exceptions; `--config <file>` retains versioned defaults and CLI flags override them.
Use `--dry-run --json` to inspect the proposal. Existing files are preserved; use a
new `--output candidate-wire-codecs.ts` basename when regenerating edited codecs.
The generated `encode`/`decode` convert wire representations only; `decode`
requires the contract's structural shape. Use `decodeInput` for negative contract
tests so missing/extra fields, wrong primitive kinds and unknown enum names reach
the actual production validator. Keep conversion outside the application-call
exception handler: a codec failure is never production rejection. Unrepresentable
values still need an explicit custom mapping. A permissive validator must remain
observable as accepting an invalid input. Existing edited scaffolds need the v2
helper merged from a separately generated proposal; installation does not replace
them. Actual production validators and calls must still run.

Managed builds/workers have separate deadlines and resource limits. The optional
Linux/systemd collective cap can be disabled with DCTX0_MEMORY_GUARD=0. Native
execution is not a security sandbox. Unsupported toolchains, incomplete dependency
capture, cleanup failures or omitted mandatory cases prevent complete evidence.

`verify run --no-adopt` records evidence without delivery. Normal runs can adopt
eligible open/unreserved tasks with complete native provenance; historical, reserved,
running, review and retirement work are protected. `verify complete` additionally
binds authored inspection notes and checks active-task completeness. An imported
legacy PASS alone cannot authorize native automatic adoption.


Before broad task waves, establish a real service call and observe its persisted
state in a fresh session. Explicit empty entity lists are observed empty state;
omitted entity keys are missing observation. Use testing-only `AdapterError` and
`EnvironmentError` helpers where their causes are actually known. Ordinary
application exceptions carry no inferred infrastructure cause. Repair actual
contract FAIL in production; inspect unclassified UNKNOWN before choosing a repair.
Do not claim external sibling sources are captured unless the current inventory
actually binds them. A complete application inventory does not imply that scope.
