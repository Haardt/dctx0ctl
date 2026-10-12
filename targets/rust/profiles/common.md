# Rust implementation guidance

Existing project conventions and explicitly selected project profiles take precedence. These defaults guide implementation; they do not add domain requirements. Read the pinned task, complete type closure, prerequisites and supplied examples before coding. Reuse delivered contracts and existing libraries. Implement only the assigned scope; report ambiguities rather than changing the spec.

Define each domain representation and its construction/validation boundary once in its owning module. Import it elsewhere instead of recreating types, validators or conversion helpers. Validate untrusted input and decoded storage/transport values; preserve domain types inside application code. Static types alone do not validate runtime input. Keep absence, invalid values and declared failures distinct. Preserve exact numeric meaning; report unsupported representations instead of rounding or truncating.

Keep production independent of dctx0 and generated testing artifacts. Use existing dependency boundaries; introduce no DI framework, persistence layer or UUID library solely for a task. Framework and class/function preferences belong to project profiles. Intents and narrative constraints must not become invented assertions.

Deliver the changed implementation artifacts, representation decisions, relevant checks actually executed and remaining limitations. A contract task supplies the contract and its construction rules, not every consuming operation. Testing PASS requires executed evidence; review approval remains separate. Tests must call real implementation code, and mocks establish consumer behavior only.

Use newtypes for domain-distinct primitives, structs for records and enums for modeled alternatives. Define each contract once in its owning module. Keep refined newtype fields private and expose checked constructors or TryFrom; avoid public construction or unchecked Deserialize that bypasses validation. Derive only traits whose equality, ordering, copying and serialization semantics fit the contract.

Use Option for modeled absence and Result with typed errors for declared failures. Propagate errors without unwrap/expect on untrusted input. Choose ownership and borrowing to fit actual lifetimes; do not introduce Arc, Mutex, Clone or async solely to satisfy a template. Use focused traits at existing external boundaries. Wire serialization must preserve the spec and validate decoded values.

Choose integer widths and exact-number representations deliberately, check overflow and conversions, and avoid lossy casts. Reuse existing domain types and error conventions; avoid a new wrapper when the distinction is not modeled. No runtime dependency on dctx0 belongs in production.
