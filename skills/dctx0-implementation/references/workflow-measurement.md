# External workflow measurement

Use `scripts/workflow_metrics.py` with exported progress, packages and per-response
usage. Save the initial plan/event pin. Task totals and historical PASS do not
measure current implementation acceptance.

Each response has a unique `response_id` and exported `package_id`. Optional
`invocation_id`, `execution_id`, `run_id` and `correlation_id` are caller-supplied
attribution; the metrics script does not create or validate native provenance.
Do not fabricate historical IDs. Missing IDs and token counts remain null.

Set `activity` explicitly when known: `spec-work`, `evidence-administration`,
`adapter-infrastructure`, `product-implementation`, `other-testing-debugging`,
`mixed`, or `unknown`. Absence means `unknown`; do not infer activity from commands.

Input fields are `input_tokens`, `cached_input_tokens`, `output_tokens`, and
optional `reasoning_tokens`. Legacy cache is included in input and reasoning is
included in output. A provider reporting separate counts must declare
`cache_semantics: separate-from-input` or
`reasoning_semantics: separate-from-output`. Retain that declaration with the
response. Reasoning is shown separately and never added again to output.
Unknown counts propagate to aggregates and prevent a token-normalized yield.

External execution receipts retain their own source/currentness limitations.
Neither usage joins nor receipt timestamps establish native PASS, independent
review, task completion, current sources or billing costs.
