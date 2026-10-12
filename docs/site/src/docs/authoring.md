# Author with your agent

**The coding agent can write the specification.** You supply the product intent and make the business decisions; the agent translates those decisions into typed contracts, examples, and explicit boundaries.

## Concentrate reasoning on the contract

During specification work, the agent can set aside database queries, algorithm bodies, transport wiring, and framework choices. Its cognitive capacity can go into understanding the domain, connecting requirements, and identifying contradictions or missing decisions.

The specification contains no application implementation. That makes space for more of the product's behavior in the same context window. A person can also understand and review that behavior without first reading the implementation. Later tasks bring in the particular technical details they need.

This separation is a workflow advantage, not a measured universal improvement in model performance. It makes assumptions explicit and gives implementation a reference that can be tested.

## A useful request

> Help me specify a workshop booking service in DCTX0. First model the vocabulary, public operations, visible failures, ownership, and examples. Ask about genuinely unresolved business rules. Keep implementation algorithms and framework wiring out of the specification. Compile the contract before moving to application work.

Use the bundled `dctx0-language` skill for syntax, composition, identity rules, and validation. Use `dctx0-implementation` when moving into task execution or adapter work.

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
