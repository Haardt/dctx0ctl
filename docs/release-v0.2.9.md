## Editable wire codecs and resumable task packages

`verify scaffold-codecs` generates editable TypeScript testing codecs from a
published specification. Configure integer, option and enum representations through
CLI flags or a versioned configuration file, with explicit per-type overrides.
Existing files are preserved. Strict `decode` and the separate `decodeInput` path
keep representation failures separate from production rejection in negative tests.
Production validators still run against actual inputs.

`tasks package-apply` reconstructs and validates a complete work package from a
saved package and its delta without opening project history. Retain the resulting
package for the next handoff; packages do not authorize claims or prove freshness.

`verify review-changes` compares current source bindings with the latest assigned
review, following explicit review-renewal ancestry when needed. It identifies the
historical source and supplies an empty current template; it never copies approval,
refreshes review hashes or changes task state. Updated embedded skills explain all
three workflows.

## Compatibility and examples

All example families were inspected for migration requirements. Existing adapters,
wire contracts, task definitions and SQLite schemas are unchanged; existing examples
do not require regeneration or migration to these optional commands. Custom codec
scaffolds remain editable and are never overwritten automatically.

## Validation

Local targeted follow-up validation passes workspace check/build/format/Clippy,
38 Python tests, seven focused Rust tests, strict TypeScript checking and 79 Bun
cases, with 81 release speed/RSS workloads and separate native measurements.
The guarded follow-up peaked at 929,284,096 bytes process-tree RSS. Earlier full
coverage evidence and the local ARM64 Go race limitation remain explicitly scoped
in the retained reports. Publication requires fresh full CI validation and all five
native platform gates; no exemption or benchmark baseline promotion is added.

See [codec scaffolds](https://github.com/Haardt/dctx0ctl/blob/v0.2.9/docs/site/src/reference/codec-scaffolds.md),
[workflow efficiency](https://github.com/Haardt/dctx0ctl/blob/v0.2.9/docs/site/src/reference/workflow-efficiency.md)
and the retained `docs/workflow-tools*-validation.json` reports.
