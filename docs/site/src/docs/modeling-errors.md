# Modeling errors

A valid contract should express actual product meaning. Many diagnostics point to a missing relationship; many UNKNOWN results point to a missing execution capability or input.

## State effects without a port witness

A surface declares its state effects and the directly called port operations that witness them. If a store operation writes an entity, keep that effect visible on the surface and observe the actual mutation. Architecture dependencies alone do not establish runtime collaboration.

## Confusing `requires`, `call`, and `observe`

Module `requires` names architectural dependencies. An executable `call` effect describes application collaboration. `observe` names external architectural participation; it does not create a callable method or prove a provider interaction occurred.

## Opaque trace labels as proof

Accepted `calls Boundary.operation(...) exactly n` labels remain explicitly unmodeled. They must not be presented as resolved graph dependencies or runtime evidence. Use modeled call effects and actual observations, and retain unsupported obligations.

## Inventing state to satisfy the checker

A stateless permission check can succeed with Unit. Do not add state or result variants merely to force a constructive projection. State-dependent requirements need honest before/after observations. Missing state support should remain an explicit gap.

## Incorrect entity identity

Use one valid `key` declaration, including all fields of a composite key. Cinema's `Rating` uses `key filmId memberId`; omitting either field changes the identity contract.

## Narrative as automatic success

An intent explains a boundary. A constraint or decision preserves a requirement. A Review task makes non-executable obligations actionable. Authoring any of these is not proof that the implementation satisfies them. Approval and testing PASS remain separate.

## Non-generatable or empty plans

Refinements and preconditions must admit valid inputs within the chosen finite budgets. Supply reviewed typed example fixtures and supported constructive inputs. No valid cases, missing required cases, or an empty configured scope cannot become success.

## Adapter implementing the expectation

The external adapter must invoke actual production code. Returning canned expected results or reconstructing business logic in the adapter invalidates the intended test. Mock replies belong only at declared collaborator boundaries.

## Incomplete review delivery

An empty template, task-only summary, missing assigned subject, stale source inventory, or unresolved actionable finding prevents complete review evidence. Store authored reports outside the captured application inventory, then record them with the current guards. Renew changed reviews explicitly.

## Before calling the work complete

Review the meaning, compile the merged specification, inspect pinned task context and the verification scope, execute required native cases, run actual persistence/HTTP checks where applicable, and inspect every outstanding obligation. Repair actual failures and replay their inputs. Complete acceptance still requires current source-bound reviews and delivered work.

The [workflow](workflow.md) and [language reference](language.md) provide concrete commands and syntax. Repository [coverage/performance validation](contributing.md) is separate from application acceptance.
