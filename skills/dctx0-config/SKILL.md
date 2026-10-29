---
name: dctx0-config
description: Inspect, explain, and adjust DCTX0 project configuration, native verification timeouts, corpus and memory budgets, target/runtime selection, and editable profiles. Use for setup and resource-limit diagnosis; business contracts and adapter implementation belong to their respective skills.
---

# DCTX0 configuration

Identify the installed CLI version, specification root, application root and exact
command that failed. Read its help before choosing flags. Follow the project's
existing authorization and settings; changing configuration does not authorize
application execution, task adoption or changes to business requirements.

Read [configuration and limits](references/configuration.md) for file locations,
required fields, defaults, and diagnosis. Inspect existing files before editing;
change the setting responsible for the observed limit and preserve unrelated
configuration, adapters, fixtures, profiles and agent instructions.

For a timeout, obtain the case ID, exact issue/message and recorded native limits.
Separate worker initialization, whole-case execution, overall worker lifetime,
build time and cleanup. A `:value:` case tests a native factory/validator, so do
not assume it timed out inside the business operation. Use the recorded input
and observations to locate expensive adapter work, nested tools or unavailable
external dependencies before deciding whether a larger limit is appropriate.

Distinguish configured values from limits actually recorded for that execution.
Explicit API callers and self-verification can supply their own limits. Report
which file/field is changed, its previous/effective value, the reason, and which
bounds remain in force. Do not infer a setting from a generic timeout message.

Keep mandatory cases, selected requirements, seeds and assertions intact while
diagnosing resource limits. A smaller corpus or different scope is an explicit
partial experiment, not an equivalent repair. UNKNOWN is not PASS; increasing
a timeout supplies time, not evidence. Never invent a restart/continue-on-timeout
option or use a review to discharge an unexecuted case.

After an authorized edit, parse the JSON and inspect the selected plan. If native
execution is within scope, run a focused diagnostic or replay and then the
required scope. `verify run --no-adopt` records evidence without task delivery;
`verify plan` performs no application execution. Configuration/adapter changes
need fresh evidence and may invalidate source-bound reviews. Save source pins
and before/after elapsed time and memory when evaluating a resource adjustment.

Use [the query guide](references/query.md) for saved-run inventory and individual
diagnostics, and [workflow measurement](references/workflow-measurement.md) when
recording work measurements. Route specification changes to `$dctx0-language`
and implementation/adapter repairs to `$dctx0-implementation` when available.
