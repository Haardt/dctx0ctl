# Agent instructions for this variant

Read the full spec and .dctx0/profiles/typescript/common.md before changing production code.

Use Hono and Awilix with strict CLASSIC injection and per-request scopes. Production behavior belongs in classes: no standalone function declarations, function expressions or arrow functions. Types, interfaces, constants, imports, class methods and bound method callbacks are allowed. Register services before resolving them and dispose request scopes and the root container. Preserve exact bigint integers.

Keep production independent of dctx0 and generated testing files. The external adapter must call real production behavior. Run bun run typecheck, bun run test --coverage and the Rust native verification commands documented in README.md. Do not treat profile prose as automatic PASS evidence.
