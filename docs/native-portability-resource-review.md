# Adapter resource inspection — 2026-10-15

This inspection follows the resource-release concern during native portability
acceptance. It does not grant a new task review approval or substitute for the
pending five-host execution evidence.

| Layer | Observed ownership and release |
| --- | --- |
| Bun transport | `end()` takes the session, rejects pending collaborator replies and awaits its `close()` hook. Request failures and input EOF also call `end()`. Technical faults are published after cleanup; protocol faults queue cleanup on the same execution chain. |
| Cinema TypeScript adapter | Each real session owns its SQLite database and Awilix scope/container. The adapter restores its SQL observer and closes the session in `finally`. HTTP uses Hono's in-process request interface; this adapter does not own a listening HTTP server. `CinemaSession.close()` closes SQLite in `finally`. |
| Go transport | `end()` takes the session, clears observations, rejects collaborator waiters and calls `Close()`. Request failures, shutdown and input closure reach cleanup. Protocol faults queue cleanup on the session owner goroutine, and errors are published after close completes. Cleanup panic remains a technical error. |
| Cinema Go adapter | `Close()` closes the owned `httptest.Server` before closing the SQLite store. HTTP response bodies use deferred close and requests have five-second timeouts. Failed store initialization closes the database. |
| Rust transport | The repaired worker closes the current session after terminal Result errors and before reporting adapter errors/panics. It takes ownership before cleanup, catches close panic and finishes the observation context. Cleanup failure remains technical failure. |
| Cinema Rust adapter | The session shuts down its owned HTTP server, then closes the owned SQLite store. Server owners also implement Drop. Built-in server shutdown signals its worker and joins it; Axum uses bounded graceful shutdown and joins its worker. Real production tests check repeated shutdown and rebinding the actual Axum port. |

The Rust transport gap was distinct from the browser test's stale focus callback.
Nine compiled-worker probes now observe the close hook, including malformed
input, EOF, duplicate IDs, adapter error/panic and failed/panicking cleanup.
See `native-portability-rust-cleanup-validation.json` for measured results.

A local process inventory during the concern found no leftover Cinema/Bun/native
worker processes from completed checks. This is a local observation, not proof
of every resource on every CI host. A stuck adapter close still relies on the
executor's deadline, direct-child termination and real wait. Direct-child mode
does not claim complete descendant-tree supervision. Successful process exit
alone is not used as evidence that the adapter's cleanup hook ran.
