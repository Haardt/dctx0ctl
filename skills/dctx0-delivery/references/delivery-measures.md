# Delivery expectations, KPIs and alpha learning

## Expectations for each delivery package

Name the user action, actual entry point, expected observable result, covered task
IDs and remaining obligations before implementation. Establish an executable
production path early; explicitly label controlled agents/resources or fixtures.
Retain complete requirements and the starting plan/event pin. End with usable
behavior, guarded task reconciliation and all required package evidence, or an
explicit incomplete result with concrete blockers. Final project gates remain
mandatory on the final relevant source state.

Use the existing compact package record for decisions, changed paths, checks,
evidence IDs and next action. Reuse packages and change reports rather than
repeatedly loading all tickets. Every repeated broad successful check needs a
relevant input change or a named unresolved concern. Every consecutive checkpoint
without functional progress needs a recorded critical-path decision. These are
workflow expectations, not claims of automatic enforcement by dctx0.

## Measure outcomes before optimizing numbers

Record UTC timestamps, package IDs, task plan/event pins and relevant source/run
identities. Compare equivalent scopes and package types with unchanged acceptance.
Set a measured baseline before numerical improvement targets. Agree any numerical
targets in the delivery plan; do not invent universal speed percentages or hard
time budgets that truncate required work. Measure these separately:

| KPI | Definition | Desired direction |
| --- | --- | --- |
| First usable path latency | Package start to first actual production entry-to-result demonstration; retain command and result | Earlier |
| Accepted delivery latency | Package start to usable delivery with complete mandatory evidence and required gates | Shorter without scope reduction |
| Evidence-to-delivery latency | Complete current passing evidence to its guarded delivery; report pending items separately | Less administrative delay |
| Unjustified identical repeats | Repeated check with identical relevant inputs and no recorded diagnostic or required reproducibility reason | Zero avoidable repeats |
| First-pass yield | Comparable packages accepted on first complete attempt divided by packages with a complete attempt | Higher, without withholding failures |
| Post-closure rework | Packages reopened for missing implementation or acceptance obligations, with reasons and observation window | Lower |
| Effort per accepted package | Measured agent tokens and check wall time, with concurrent durations identified; record peak RSS separately | Lower for comparable work |

Use `tasks progress` from the saved starting plan/event pin for newly Done and
PASS transitions, new/removed work and lost active PASS. Those are historical
transitions, not accepted feature counts or current source freshness. Count
functional delivery only with a demonstrated user path and its mandatory evidence.
Keep early demonstration and full acceptance timestamps distinct.

Read [workflow measurement](workflow-measurement.md) when collecting usage or
execution receipts. If the repository supplies `scripts/workflow_metrics.py`,
use it for supported receipt metrics rather than rebuilding calculations. The
script is repository tooling, not a guaranteed dependency of an installed skill.
Unknown token counts, timestamps or provenance stay null/unmeasured, never zero.
External receipts do not become native proof. Peak RSS measurements must name
the platform/method; elapsed user time is not active agent work time.

## Learn from alpha use

When repeated friction or a meaningful workflow failure is observed, create or
update `<app-root>/DCTX0_IMPROVEMENT_WORKFLOW.md`. This is a compact improvement
record, not another ticket system or a new mandatory log for every action. Honor
local placement rules and keep it outside captured verification inputs where
possible. If it is captured, writing it can stale source-bound evidence; update
before final capture and respect the resulting freshness checks.

Each entry contains:

- Observation, date and concrete evidence: package/task/run IDs, command or source
  identities where available; never fabricate missing measurements.
- Cause, clearly distinguishing confirmed findings from hypotheses.
- Proposed adjustment, expected KPI effect and unchanged acceptance obligations.
- Classification: local workflow, skill guidance or dctx0 product/tool limitation.
- Trial result with comparable before/after measurements where available, tradeoffs,
  and state: proposed, trial, retained, rejected or superseded.

Read existing entries when planning relevant work. Apply authorized reversible
workflow improvements within project rules, evaluate the result and retain failed
trials too. Record product gaps without silently turning application delivery into
dctx0 development. An entry grants no permission to skip gates, rewrite tasks,
relax contracts, manufacture PASS or send external messages. Promote a finding
into shared skill guidance only through an intentional reviewed change, supported
by real usage; alpha observations are not automatically general truths.
