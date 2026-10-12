# What Happens When Your Spec Gives an Entire AI Agent Team Its Work?

**Agents can help write the contract, work through its tasks in parallel, and have their implementation checked against it. That is the idea behind DCTX0.**

Imagine three coding agents working on your application.

One builds the API. One implements persistence. One writes the tests. Each has a clear assignment. Each reports success.

Then you connect their work.

The API treats an unknown customer as an error. The persistence layer returns an empty record. The tests accept the empty record because that is what the implementation produces.

The code runs. The parts disagree about the product.

Adding another agent to review the diff might find the mismatch. But someone still has to answer a more basic question: **where is the agreed meaning that all four agents should be working from?**

That question sits underneath several of today's AI coding discussions: context engineering, spec-driven development, parallel agents, and reliable evaluation.

I am building DCTX0 to connect those concerns. It turns a reviewed specification into a task graph, focused implementation context, and executable verification plans. The same contract helps determine what workers should build, what can run in parallel, and what the finished application must do.

## More agents create a coordination problem you can feel

With one agent, you can often repair a misunderstanding in conversation. With several, the same clarification has to reach every affected assignment. A decision made in one session can become an assumption in another.

Meanwhile, the amount of code grows. Every worker needs enough context to understand its task, but reading more files also means processing more implementation detail before reaching the relevant product rule.

These concerns already have useful approaches. GitHub's [Spec Kit](https://github.com/github/spec-kit) organizes development around specifications, plans, and tasks. Anthropic's [context engineering guidance](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents) treats context as a finite resource and describes loading relevant information as work progresses.

DCTX0 applies those ideas through a typed, executable model of the product. Its compiler checks declarations, references, and expressions. Its planner derives task prerequisites. Its verification core evaluates observations from the actual application against the modeled expectations.

That gives the specification a job throughout development. It remains useful after the planning conversation ends.

## Your agent can write the spec before it writes the application

An extensive specification sounds like a substantial writing assignment. The agent can take on much of that work.

You describe the product. The agent drafts contracts and examples. You discuss missing decisions and edge cases, then refine the model together. The language skill guides the agent through authoring and validation.

This can make a detailed specification much faster to draft than writing every declaration and example by hand. It also lets the team work through a broad description of the product before spending time on its implementation. The speed advantage depends on how much the domain still needs clarification; there is no measured universal multiplier here.

**The agent gets a phase in which its cognitive capacity can focus on the contract.**

It can examine whether two operations use the same definition of a customer, whether a failure case has been decided, and whether state transitions fit together. Database plumbing and framework configuration can wait until it implements a concrete assignment.

This changes what fits into context. A specification carries data contracts, behavior, dependencies, and decisions without all the code needed to realize them. More of the application's meaning can be considered together.

People benefit for the same reason. A product owner or developer can understand the rules before reading the implementation. Reviewing a decision such as “an unknown customer produces an error” is more accessible than discovering that decision inside a persistence adapter.

The human still owns the product choices. A compiler can catch a broken reference or an invalid expression; it cannot decide whether the business wanted the right thing.

## A shared decision in three lines

Return to the customer lookup.

The data-source operation is named `CustomerStore.find`. The public operation is `CustomerApi.getName`. We agree that an unknown customer must produce the business error `CustomerNotFound`.

A DCTX0 example records that decision:

```dctx0
example "unknown customer" {
  given CustomerStore.find("customer-42") returns None
  when result = CustomerApi.getName("customer-42")
  expect result fails CustomerNotFound
}
```

The first line supplies the situation: the data source finds nothing. The second invokes the public operation. The third states the required outcome.

The complete specification also declares these interfaces, their types, and the error. There is no hidden customer fixture needed to understand this example; the input is written directly into it.

Now the API worker, persistence worker, and reviewer have a common reference. Returning an empty name successfully would violate the agreed expectation, even if that behavior looked plausible in isolation.

Examples are one part of the language. Data contracts define valid values. Invariants express general rules. Lifecycles describe permitted state transitions. Intent, constraints, and review criteria preserve requirements that also need human or agent judgment.

The application implements these requirements in its own language. The spec supplies the behavioral boundaries.

## The same spec can feed an entire team

Once the specification is valid, DCTX0 derives implementation tasks with contracts, required types, examples, and acceptance criteria.

Those tasks form a directed acyclic graph: a **task DAG**. Every hard prerequisite identifies the result needed and explains why it is needed.

For a customer directory, lookup and rename work may proceed independently once their prerequisites are delivered. An integration task waits for the particular results it consumes. A semantic call between two operations does not automatically force their complete implementations into a serial sequence.

DCTX0 groups the work into dependency waves:

- The first wave has no open prerequisites.
- Tasks within a wave are independent in the prerequisite graph and can be worked on in parallel.
- Later tasks become ready when their own prerequisites are delivered, without waiting for unrelated earlier work.

**The specification provides both the product meaning and a usable structure for dividing the work.**

An agent can delegate available assignments to sub-agents. A team can share them among developers. An external orchestrator can use its preferred models and runtimes. DCTX0 supplies the plan and coordination commands; the executing system runs the agents.

Atomic claims keep two workers from receiving the same task. Each assignment refers to a pinned specification version, with required context and predecessor results available on demand. Workers can also reserve a dependency-closed slice leading toward a later task and progress through its prerequisites in order.

Shared-file edits still need coordination. Independent assignments do not make concurrent changes to a repository conflict-free. The DAG tells workers which results they need and which work can progress independently.

## Fewer errors starts with fewer decisions being silently reinvented

This is where parallelism and stability meet.

The lookup worker should not invent its own definition of a missing customer. The rename worker should not quietly introduce a different name contract. A downstream task should not assume a prerequisite is finished because a conversation sounded encouraging.

DCTX0 records the relevant rules, pins them to assignments, and tracks delivered results. Each worker receives a focused task while retaining the contracts it must respect.

That structure addresses several practical sources of error: inconsistent assumptions, overlooked requirements, duplicated assignments, and premature handoffs. It also gives a new worker a place to recover the actual requirements after a session restart or context compaction.

The intended result is a more stable development process and fewer defects caused by requirements being lost between steps. The mechanism is inspectable: shared contracts, explicit prerequisites, guarded ownership, and recorded evidence. The size of the improvement across different projects still needs comparative measurement.

There is one more connection to make: the application has to be checked against the contract.

## The test expectation exists before the implementation

An agent can write code and then write tests that faithfully reproduce its own misunderstanding. A reviewed specification gives the test expectation an earlier, explicit source.

For the customer example, a separate testing adapter supplies the prescribed data-source reply and invokes the real application operation. A test program for the application's language reports what happened.

DCTX0's central Rust core compares that observation with `CustomerNotFound`.

If the implementation returns success with an empty name, the example fails. A reviewer does not have to rediscover the product decision from the diff. The failure points back to the agreed expectation.

**The worker reports observations. The central core determines whether they satisfy the contract.**

The same approach supports examples, generated contract cases, properties, and supported lifecycle checks. Production code remains independent of DCTX0, with the adapter kept in a separate testing area.

This connection needs care. The adapter must invoke the correct code and report accurately. A prepared reply from a data source checks consumer behavior under controlled conditions; actual database access and HTTP wiring need real integration checks as well.

## A green result should survive an awkward question

Ask a completed task: “What exactly did you verify?”

DCTX0 keeps work progress separate from testing evidence. **Done does not imply PASS.** Review approval has its own meaning and does not turn into an automatic test result.

Verification distinguishes `PASS`, `FAIL`, and `UNKNOWN`. A PASS applies to the mandatory requirements of the selected finite plan. A FAIL identifies a violation. UNKNOWN keeps a missing required proof visible.

The core owns deterministic case generation and assessment. With the same inputs and settings, the cases can be reproduced. Supported failing inputs can be reduced through shrinking, making the cause easier to understand. Replay runs saved cases against a newly captured implementation and records the comparison.

That gives an agent a concrete repair target: a requirement, an input, an observation, and an expectation. The earlier failed run remains part of the history.

Finite testing cannot establish correctness for every possible input. Architecture judgments and narrative requirements also retain explicit review steps, with inspections tied to requirements and source artifacts. The aim is to make the scope of a result clear enough to act on.

## The product changes. The old green check stays in the past.

Suppose we change the customer lookup rule. Unknown customers should now return an optional result instead of a business error.

The old implementation might have passed every earlier check. It still needs new work.

DCTX0 tracks changes to contracts, modeled behavior, and descriptive requirements, then follows dependencies to explain their impact. Relevant changes create new adaptation tasks linked to previous results and the requested change.

Historical tasks retain their original requirements and evidence. They help the next worker understand the existing implementation without pretending that an old PASS proves the new behavior.

This is particularly useful across several agents and sessions. The change becomes an explicit assignment with a reason. Task history and verification reports persist outside the conversation, alongside the reviewed specification files.

## A portable contract needs a real application behind it

DCTX0 currently has native verification targets for TypeScript/Bun, Go, and Rust. The language-neutral core owns the expectations and assessment; target workers execute application code and report observations. Java currently provides editable implementation profiles, with native verification still outstanding.

The larger reference project is a shared movie-night application: a film catalog, comments, ratings, and voting. Its documented TypeScript, Go, and Rust acceptances each cover 5,674 mandatory finite cases against the common behavioral plan. They also exercise real persistence and HTTP, deliberate defects in production code, and replay after repair.

These are [documented application checks](https://github.com/Haardt/dctx0ctl#cinema-acceptance), rather than a comparative benchmark of agent productivity. They demonstrate the verification scope; they do not yet establish a universal reduction in development time or defect rate.

The project includes a native language server, local editor extensions, and three agent skills covering the workflow, specification language, and implementation. Editable language and framework profiles give workers additional technical guidance.

DCTX0 is an experimental Rust CLI. Managed native verification currently requires Linux. Installation, platform boundaries, and a step-by-step tutorial are in the repository.

## Give your agents a contract that keeps working after the prompt

The attractive part of AI coding is how quickly an idea can become an implementation. As the work grows, the harder part is keeping decisions consistent across workers, changes, and verification.

A specification can carry more of that responsibility.

Agents help create it quickly and at substantial breadth. People review its meaning without first reading all the implementation details. A planner derives bounded tasks and parallel waves. The verification core checks the real application against the same expectations. Product changes create traceable follow-up work.

That is the direction of DCTX0: **one contract connecting product understanding, coordinated implementation, and evidence.**

If you already use agents or sub-agents, try it on a bounded feature. Agree on the behavior, inspect the derived tasks, implement them, and deliberately break one rule. See whether the resulting failure tells the next worker exactly what needs repairing.

[DCTX0 on GitHub — specifications, tutorial, and runnable examples](https://github.com/Haardt/dctx0ctl)

*Make intent executable.*

---

*Project status: October 12, 2026. DCTX0 is experimental and has not yet been optimized for token efficiency.*
