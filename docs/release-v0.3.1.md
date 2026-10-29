## Delivery workflow and alpha improvements

DCTX0 0.3.1 bundles five installable agent skills. The new `dctx0-delivery`
skill plans substantial implementation around usable production workflows,
complete pinned task context and actual dependencies. It uses package deltas,
source-bound review changes, compact diagnostics and progress reports to avoid
repeated discovery and unnecessary broad validation. Delivery measures separate
first usable behavior, fully accepted delivery, repeated checks and effort.
Agents retain concrete alpha workflow findings in an application-owned
`DCTX0_IMPROVEMENT_WORKFLOW.md`; findings do not relax acceptance or grant PASS.

This release also includes the existing configuration skill, state-clause and
nested-call verification improvements, argument/generation fixes and opt-in
Rust/libtest coverage and test-analysis research. Experimental affected-test
reports are conservative suggestions: `can_skip_other_tests` remains false.
The research corpus is separate from specification/task evidence and does not
authorize automatic test skipping, PASS reuse or adoption.

Existing task definitions and the main SQLite schema remain unchanged. SDK
interface version 2 and transport framing version 1 are retained. Invocation-aware
observation adds a worker request; use the current generated testing transports
with the current CLI. Experimental analysis uses additive extension tables. All
required workspace validation and five-host distribution/native checks remain
release gates. Performance baselines are not automatically promoted.
