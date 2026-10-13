# Author with your agent

**You can start without learning to author DCTX0 yourself. The coding agent can write and validate the specification with you.** You supply the product intent and make the business decisions; the agent translates those decisions into typed contracts, examples, and explicit boundaries.

## Concentrate reasoning on the contract

During specification work, the agent can set aside database queries, algorithm bodies, transport wiring, and framework choices. Its cognitive capacity can go into understanding the domain, connecting requirements, and identifying contradictions or missing decisions.

The specification contains no application implementation. That makes space for more of the product's behavior in the same context window. A person can also understand and review that behavior without first reading the implementation. Later tasks bring in the particular technical details they need.

This separation is a workflow advantage, not a measured universal improvement in model performance or token cost. It makes assumptions explicit and gives implementation a reference that can be tested.

## A useful request

> Use the dctx0-language skill to draft a specification under dctx0/ for a film catalog. Films have a title and a release year from 1888 through 2100. Users can retrieve a film by ID; an unknown ID must return FilmNotFound. Ask me about unresolved product rules instead of choosing silently. Validate the specification with dctx0 check, explain its rules and examples in plain language, and wait for my review before implementation.

Use the bundled `dctx0-language` skill for syntax, composition, identity rules, and validation. Use `dctx0-implementation` when moving into task execution or adapter work.

## Read and review the draft

You do not need to memorize the grammar to review a first contract. Types, ranges, operation signatures, and given/when/expect examples expose rules in a form developers can often read directly. More involved constructs, such as lifecycles and collaborator replies, deserve an explanation from the agent or the [language reference](language.md).

For the film-catalog request, review these concrete questions:

- Does the year range include both 1888 and 2100?
- Does an unknown film ID produce FilmNotFound?
- Which rules are still undecided, such as whether duplicate films are allowed?

Ask the agent to show the relevant declarations and examples beside its explanation. Resolve open decisions and correct the draft before authorizing implementation. Later, if a native check observes a different outcome, investigate the application and adapter against that agreed rule. Missing evidence remains visible in the verification report.

## Develop the contract together

1. Name the workspace and its cohesive domains.
2. Introduce business values, entity identity, messages, and visible failures.
3. Define who owns mutable state and which boundaries provide it.
4. Write important success and rejection examples.
5. Generalize rules with refinements, invariants, contracts, or lifecycles.
6. Preserve qualitative requirements as constraints and explicit reviews.
7. Compile and inspect the verification plan; review its missing inputs and unsupported obligations.

`dctx0 parse <file-or-root>` checks syntax. `dctx0 check <spec-root>` resolves and validates the merged specification and publishes a snapshot. `verify plan` projects the selected requirements without executing the application.

## Review meaning before implementation

Check that the examples express the product you want. Valid syntax cannot decide whether a business policy is desirable. A wrong assumption can be encoded consistently in a specification, so people still review the meaning.

For implementation, give the agent the pinned task, acceptance criteria, required types, and selected profiles. Test the real application against the agreed contract. Compiler checks, native cases, meaningful mutations, and replay help expose different kinds of errors.

An implementation failure is a reason to investigate or repair the application. Changing the expected behavior requires an explicit product decision; a checker limitation alone is not a reason to weaken the contract.

Continue with [WorkshopHub](tutorial.md), [modeling errors](modeling-errors.md), or [skills and editors](tools.md).
