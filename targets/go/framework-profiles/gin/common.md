# Go implementation guidance

Existing project conventions and explicitly selected project profiles take precedence. These defaults guide implementation; they do not add domain requirements. Read the pinned task, complete type closure, prerequisites and supplied examples before coding. Reuse delivered contracts and existing libraries. Implement only the assigned scope; report ambiguities rather than changing the spec.

Define each domain representation and its construction/validation boundary once in its owning module. Import it elsewhere instead of recreating types, validators or conversion helpers. Validate untrusted input and decoded storage/transport values; preserve domain types inside application code. Static types alone do not validate runtime input. Keep absence, invalid values and declared failures distinct. Preserve exact numeric meaning; report unsupported representations instead of rounding or truncating.

Keep production independent of dctx0 and generated testing artifacts. Use existing dependency boundaries; introduce no DI framework, persistence layer or UUID library solely for a task. Framework and class/function preferences belong to project profiles. Intents and narrative constraints must not become invented assertions.

Deliver the changed implementation artifacts, representation decisions, relevant checks actually executed and remaining limitations. A contract task supplies the contract and its construction rules, not every consuming operation. Testing PASS requires executed evidence; review approval remains separate. Tests must call real implementation code, and mocks establish consumer behavior only.

Use named Go types for domain-distinct primitives (`type FilmID string`, not a type alias), structs for records and small consumer-facing interfaces for ports. Define each type once in its owning package. Named types still permit conversions and zero values: checked constructors/decoders and boundary validation must enforce refinements; document whether zero is valid.

Return explicit errors and preserve modeled failure identity/payloads through typed errors and errors.Is/errors.As where appropriate. Do not panic for invalid external input. Make pointer/value, nil/empty collection and optional-field choices explicit; a zero value must not silently stand for absence. Use existing exact-number facilities when machine integers or floats cannot preserve the contract.

Pass context.Context through cancellable I/O boundaries when the project uses it. Avoid package globals for mutable test/application state. Define interfaces at the consuming boundary; do not generate an interface for every struct or force a DI container. Keep package dependencies acyclic and reuse the owning package's contracts.

## Gin HTTP integration

Use Gin 1.12.x with the project's configured Go toolchain. Compose explicit production routes in a constructor returning http.Handler; inject existing services and collaborators. Use gin.New rather than implicitly installing logging/recovery policies. Configure logging/mode once at composition; keep repeated test-session router construction quiet within the harness log budget. Configure middleware deliberately, keep logs off the harness protocol's stdout, and never retain a pooled gin.Context beyond the request. Propagate request cancellation through existing I/O interfaces.

Decode request bodies and path/query values using the owning domain constructors. Gin binding tags are transport checks, not the entire domain contract. Limit bodies and preserve exact integers (avoid decoding into float64). Specify error/status mapping, unknown-route and method behavior explicitly. Do not expose every operation automatically. Authentication, CORS, recovery and retries require project requirements.

Test the actual composed router using httptest; use a real loopback server for the project's HTTP harness path. Each test session owns its service, store and server and closes them on every exit. The testing adapter translates explicit routes and actual responses to the shared harness contract; it does not rebuild business rules or assert its own success verdict.
