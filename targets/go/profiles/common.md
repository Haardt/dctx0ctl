# Go implementation guidance

Existing project conventions and explicitly selected project profiles take precedence. These defaults guide implementation; they do not add domain requirements. Read the pinned task, complete type closure, prerequisites and supplied examples before coding. Reuse delivered contracts and existing libraries. Implement only the assigned scope; report ambiguities rather than changing the spec.

Define each domain representation and its construction/validation boundary once in its owning module. Import it elsewhere instead of recreating types, validators or conversion helpers. Validate untrusted input and decoded storage/transport values; preserve domain types inside application code. Static types alone do not validate runtime input. Keep absence, invalid values and declared failures distinct. Preserve exact numeric meaning; report unsupported representations instead of rounding or truncating.

Keep production independent of dctx0 and generated testing artifacts. Use existing dependency boundaries; introduce no DI framework, persistence layer or UUID library solely for a task. Framework and class/function preferences belong to project profiles. Intents and narrative constraints must not become invented assertions.

Deliver the changed implementation artifacts, representation decisions, relevant checks actually executed and remaining limitations. A contract task supplies the contract and its construction rules, not every consuming operation. Testing PASS requires executed evidence; review approval remains separate. Tests must call real implementation code, and mocks establish consumer behavior only.

Use named Go types for domain-distinct primitives (`type FilmID string`, not a type alias), structs for records and small consumer-facing interfaces for ports. Define each type once in its owning package. Named types still permit conversions and zero values: checked constructors/decoders and boundary validation must enforce refinements; document whether zero is valid.

Return explicit errors and preserve modeled failure identity/payloads through typed errors and errors.Is/errors.As where appropriate. Do not panic for invalid external input. Make pointer/value, nil/empty collection and optional-field choices explicit; a zero value must not silently stand for absence. Use existing exact-number facilities when machine integers or floats cannot preserve the contract.

Pass context.Context through cancellable I/O boundaries when the project uses it. Avoid package globals for mutable test/application state. Define interfaces at the consuming boundary; do not generate an interface for every struct or force a DI container. Keep package dependencies acyclic and reuse the owning package's contracts.
