## Breaking change: native SDK interface 2

Version 0.3.0 starts the new minor series for the native adapter API change.
Rust adapter/session entry points now return `TechnicalError` rather than
`String`. Use `TechnicalError::adapter(error)` for explicit translation failures
and `TechnicalError::environment(error)` for explicit unavailable infrastructure.
Ordinary strings remain unclassified. Go uses the `unclassified` category for
unattributed errors and explicitly attributes a missing state observer to the
adapter. Existing TypeScript error helpers retain their meaning.

The SDK interface version is 2; transport framing remains independently versioned
at protocol 1. Native transport callers use `PROTOCOL_VERSION` for envelopes.
Compiler IR, SQLite schema 7, query views 2 and historical outcomes remain
unchanged. No automatic history conversion or reclassification is introduced.

`verify bindings plan` derives required native roles and the type closure from
published requirements without application execution. Strict binding-description
models are available, but native signatures and runtime readiness remain separate
claims. See [binding foundations](site/src/reference/verification-bindings.md).

This is the validated initial checkpoint of the larger workflow plan. Generated
standard adapters, preflight, continuations, external source registration,
existing-run completion and the actual consumer migrations remain outstanding.
Version 0.3.0 does not claim those features are complete.

The pre-version-bump checkpoint passed the complete local workspace validation,
97% coverage gates and 82 release speed/RSS workloads. Exact scope and raw metrics
remain in `docs/verification-bindings-validation.json`. The version bump receives
separate focused local installation/smoke evidence in
`docs/release-v0.3.0-validation.json`; it does not relabel the previous coverage as
a new version-specific run. Public release publication still requires normal CI
and all five native distribution hosts.
