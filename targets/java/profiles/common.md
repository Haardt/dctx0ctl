# Java implementation guidance

Existing project conventions take precedence. Use the configured Java version. Prefer small value objects (records where supported) and focused interfaces. Keep application contracts independent of framework adapters.

Use existing libraries and primitive representations where appropriate; do not introduce a dependency injection library or UUID dependency solely for this task. Intents explain decisions; they do not add domain requirements. Contract tasks publish contracts and their constraints, rather than implementing complete operations. Testing PASS requires executed evidence against the requirements and implementation; these architecture notes do not constitute that evidence.
