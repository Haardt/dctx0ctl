## Architecture

- TypeScript with Hono
- PsqlLite
- DI (awivix)
- Public surfaces declare `exposes requestReply` and are bound as Hono routes in `dctx0.hono.bindings.ts`

Use bun
