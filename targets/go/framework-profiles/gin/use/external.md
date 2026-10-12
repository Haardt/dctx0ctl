# Use external

Use the configured external adapter and its resource/failure contract. Supply explicit test configuration when exercising it; do not silently reach production services or substitute mocks for integration evidence.

Use the configured connection and explicit test resources. Bound calls and clean up responses/sessions; do not invoke external production services implicitly. Follow the framework rules in common.md.
