# Overview

DCTX0 is an executable specification language and toolchain for AI-native software development. Product owners and coding agents develop a typed contract together. That contract drives implementation tasks and checks against actual application behavior.

## Four connected models

| Model | Question | Examples |
| --- | --- | --- |
| Domain | What business concepts exist? | Values, entities, messages, events, failures |
| Control | Who owns them and what is called? | Modules, ownership, ports, surfaces, effects |
| Behavior | What must happen? | Examples, invariants, pre-/postconditions, lifecycles |
| Verification | What evidence is required? | Suites, reviews, policies, finite plans |

The specification contains typed expressions and observable requirements. Application algorithms, SQL, HTTP clients, and framework wiring stay in application code.

## Who writes the specification?

Your agent can author and refine it with you. People own product decisions. With implementation details set aside, the agent can devote its cognitive capacity to the domain and its relationships. More of the application's meaning fits into context, and people can understand the system before reading its technical machinery.

The [language skill](authoring.md) guides authoring and compilation. The [implementation skill](tools.md) guides focused application work, testing adapters, and native verification.

## One workspace, multiple source files

Exactly one workspace forms the logical specification boundary. A product, an independently validated service, or a bounded context can each be a workspace. It need not match a Git repository or a package-manager module.

<!-- dctx0:check -->
```dctx0
workspace CommercePlatform {
  domain Identity {
    module Users {
      intent "Own user identity and lifecycle behavior."
    }
  }
  domain Billing {
    module Subscriptions {
      intent "Own subscription and renewal behavior."
    }
  }
}
```

Top-level domains and fragments can contribute across files. The compiler discovers sources deterministically, composes them, resolves references, and produces validated IR.

## Current scope

The Rust CLI provides compilation, snapshots and diffs, explicit renames, task planning and claims, native verification, immutable evidence, replay, and guarded task delivery. TypeScript/Bun, Go, and Rust have executable targets; Java currently provides profiles.

The CLI does not invoke an LLM. External agents use its commands and installed skills. Generated artifacts are for testing; production-code generation remains future work.

A compiled specification is not evidence that an application works. Read [evidence](evidence.md) for the distinction between automatic checks, finite tests, and authored reviews.

## Where to go next

- [Build the CLI and check a first specification](getting-started.md)
- [Author with your agent](authoring.md)
- [Build WorkshopHub's contract](tutorial.md)
- [Learn with 16 executable MiniBoard applications](miniboard.md)
- [Explore the language reference](language.md)
