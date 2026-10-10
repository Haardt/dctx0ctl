# Use event

Use the configured Java version. Prefer small value objects (records where supported) and focused interfaces. Keep application contracts independent of framework adapters.

Consume the delivered event contract without redefining its requirements. Publish the complete event payload and delivery contract without adding unmodeled guarantees.
