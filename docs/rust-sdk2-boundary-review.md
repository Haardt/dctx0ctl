# Rust Cinema SDK-2 testing-boundary inspection — 2026-10-25

The implementing Codex session inspected the complete external adapter, the
contract delta, its translation boundary and its calls to production validators.
This is self-inspection. Neither a changed actor name nor a separate session
establishes independent review. Native execution remains separate evidence.

The delta replaces string errors at adapter/session boundaries with typed
`TechnicalError`. Explicit adapter/environment causes are retained; ordinary
strings remain unclassified. Missing state observers and strict argument/result
conversion errors are attributed to the adapter. The real `domain::validate`
receives successfully decoded input; a decoder error returns a technical error
and cannot be counted as production rejection. The helper contract retains this
application's original message limit and transport version. Production sources,
model semantics, database behavior, HTTP behavior and specifications are untouched.

Existing production-source judgements remain source-bound to their unchanged
files. Only these testing-source bindings in the authored fixture are renewed:

| File | Previous SHA-256 | Inspected SHA-256 |
| --- | --- | --- |
| `.dctx0/testing/rust/adapter.rs` | `e32b3711d6f511dd2830d7ff9a3dc614e25fbb70a62e95d7ed493d812960ced3` | `567552d9d95622ac642fce117fc5f80f6968dfec7713e8257dbdd10ceb767299` |
| `.dctx0/testing/rust/contract.rs` | `84e89050e3878c8dc85b889132e58c65a35636b9851b2934283921bb8ca671da` | `d5802b4227dd93f575fb8ed88cb996dbbf73de3d60f8f7a071505d45a7f9af2a` |

The fixture records the scope and self-inspection limitation. Acceptance scripts
continue to reject source drift; they do not automatically refresh these hashes.
The first coupled run failed this guard because the prior note referenced the old
adapter/contract. It remains recorded as failed, with incomplete measurements.
New native/HTTP, mutation and replay results must be read from fresh validation;
this inspection is not a substitute for them and does not alter historical PASS.
