# Java implementation guidance

Existing project conventions and explicitly selected project profiles take precedence. These defaults guide implementation; they do not add domain requirements. Read the pinned task, complete type closure, prerequisites and supplied examples before coding. Reuse delivered contracts and existing libraries. Implement only the assigned scope; report ambiguities rather than changing the spec.

Define each domain representation and its construction/validation boundary once in its owning module. Import it elsewhere instead of recreating types, validators or conversion helpers. Validate untrusted input and decoded storage/transport values; preserve domain types inside application code. Static types alone do not validate runtime input. Keep absence, invalid values and declared failures distinct. Preserve exact numeric meaning; report unsupported representations instead of rounding or truncating.

Keep production independent of dctx0 and generated testing artifacts. Use existing dependency boundaries; introduce no DI framework, persistence layer or UUID library solely for a task. Framework and class/function preferences belong to project profiles. Intents and narrative constraints must not become invented assertions.

Deliver the changed implementation artifacts, representation decisions, relevant checks actually executed and remaining limitations. A contract task supplies the contract and its construction rules, not every consuming operation. Testing PASS requires executed evidence; review approval remains separate. Tests must call real implementation code, and mocks establish consumer behavior only.

Respect the configured Java version. Use small immutable value objects for domain-distinct primitives and records for data contracts where supported; validate in constructors or checked factories. Define each contract once and import it elsewhere. Avoid passing raw String or numeric primitives where an established domain value is required. Preserve equality/hashCode semantics and make defensive copies of mutable collections.

Use enums or sealed hierarchies for alternatives where the Java version supports them; otherwise use the project's established explicit variant pattern. Represent modeled failures with typed payloads and consistent project error conventions. Distinguish absent values from invalid input; do not scatter null sentinels or rely on annotations as runtime validation.

Use checked conversions, Math exact operations or BigInteger/BigDecimal when necessary to preserve numeric contracts. Keep framework annotations and transport/persistence mapping at existing boundaries. Reuse the configured composition approach; do not require Spring, a DI container or an ORM. Java profiles guide agents and do not claim implemented native harness support.
