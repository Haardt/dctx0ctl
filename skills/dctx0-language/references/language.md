# Language and evidence reference

Use one workspace and top-level fragments to extend semantic owners across files:

```dctx0
workspace Shop {
  domain Commerce {}
}
fragment Commerce {
  scalar Year is Int {
    range 1888..2100
  }
  surface Catalog {
    operation next(input: Int) -> Int {
      pure
      ensures result == input + 1
    }
  }
}
```

Use the actual repository examples and `check` diagnostics for clause syntax;
do not assume another specification language's syntax. Data declarations include
aliases/refinements, records, variants, options/lists, entities and composite keys,
messages, events and declared failures. Typed expressions, examples, properties,
invariants, state effects and connected lifecycle sequences have distinct roles.
Check numerical integer/range boundaries and preserve exact integer/decimal values
at native boundaries. Not every compiler-accepted clause has executable meaning.

Intents, constraints and decisions are reviewed facts; free text does not create
additional assertions. Static syntax discovery yields candidates and ambiguity,
not semantic type proofs or runtime call counts. Mocks demonstrate consumer behavior
against declared collaborators; they do not certify actual production adapters.

Qualified names identify declarations within a snapshot. Explicit renames record
continuity; source hashes, semantic fingerprints, check occurrences, task event
sequences and native implementation hashes have different provenance roles.
Historical PASS is evidence for its historical requirements and implementation.
Spec changes create new adaptation/change work rather than rewriting completed tasks.
