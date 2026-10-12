# TypeScript implementation guidance

Existing project conventions and explicitly selected project profiles take precedence. These defaults guide implementation; they do not add domain requirements. Read the pinned task, complete type closure, prerequisites and supplied examples before coding. Reuse delivered contracts and existing libraries. Implement only the assigned scope; report ambiguities rather than changing the spec.

Define each domain representation and its construction/validation boundary once in its owning module. Import it elsewhere instead of recreating types, validators or conversion helpers. Validate untrusted input and decoded storage/transport values; preserve domain types inside application code. Static types alone do not validate runtime input. Keep absence, invalid values and declared failures distinct. Preserve exact numeric meaning; report unsupported representations instead of rounding or truncating.

Keep production independent of dctx0 and generated testing artifacts. Use existing dependency boundaries; introduce no DI framework, persistence layer or UUID library solely for a task. Framework and class/function preferences belong to project profiles. Intents and narrative constraints must not become invented assertions.

Deliver the changed implementation artifacts, representation decisions, relevant checks actually executed and remaining limitations. A contract task supplies the contract and its construction rules, not every consuming operation. Testing PASS requires executed evidence; review approval remains separate. Tests must call real implementation code, and mocks establish consumer behavior only.

Use the configured TypeScript version and strictness. For domain-distinct primitive values, prefer branded types with a shared project-owned brand helper using a unique-symbol key (for example `Brand<string, "FilmId">`). Define each named brand once and reuse it in fields, parameters and results. Do not brand every ordinary primitive or invent a new domain distinction absent from the spec.

Create branded values through checked factories/decoders. Keep unavoidable assertions inside that validated construction boundary; avoid scattered `as FilmId`, `as any` or double casts. Brands are compile-time distinctions, not runtime validators. Decode JSON as unknown before validation, and serialize the underlying value without brand metadata. Reuse existing equivalent domain types in adopted projects.

Use discriminated unions for modeled alternatives and failures, with exhaustive handling. Use readonly properties/collections for value contracts and explicit mutation for modeled entity state. Use bigint or an existing exact-number representation when number cannot preserve the contract; keep transport conversion explicit. Follow existing async and error conventions without mixing domain failure with unexpected exceptions.
