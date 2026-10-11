# Agent instructions for this variant

Read the full spec and .dctx0/profiles/typescript/common.md before changing production code.

Use direct Bun HTTP handling and ordinary exported functions. Do not add Hono or dependency injection. Keep integer values as bigint and wire integers as lossless decimal strings.

Keep production independent of dctx0 and generated testing files. The external adapter must call real production behavior. Run bun run typecheck, bun run test --coverage and the Rust native verification commands documented in README.md. Do not treat profile prose as automatic PASS evidence.
