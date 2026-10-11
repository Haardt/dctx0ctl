# Tutorial architecture choice

Use direct Bun HTTP handling and ordinary exported functions. Do not add Hono or dependency injection. Keep integer values as bigint and wire integers as lossless decimal strings.

The reviewed spec defines behavior. These notes guide implementation and do not create additional behavioral assertions.
