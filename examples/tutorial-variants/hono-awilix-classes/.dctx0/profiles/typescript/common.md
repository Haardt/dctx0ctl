# Tutorial architecture choice

Use Hono and Awilix with strict CLASSIC injection and per-request scopes. Production behavior belongs in classes: no standalone function declarations, function expressions or arrow functions. Types, interfaces, constants, imports, class methods and bound method callbacks are allowed. Register services before resolving them and dispose request scopes and the root container. Preserve exact bigint integers.

The reviewed spec defines behavior. These notes guide implementation and do not create additional behavioral assertions.
