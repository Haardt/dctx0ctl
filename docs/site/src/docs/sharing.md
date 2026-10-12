# History and sharing

Reviewed `.dctx0` files are the canonical shareable requirements. Commit them with application sources, authored adapters, fixtures, and appropriate profiles. Derived history belongs under the spec root's `.dctx0ctl/` and is ignored by Git.

## Snapshots and changes

```sh
dctx0 history <spec-root> --json
dctx0 diff <spec-root> <before-snapshot> latest --json
dctx0 impact <spec-root> <qualified-symbol> --snapshot latest --json
```

Selectors accept `latest`, full hashes, or unambiguous hexadecimal prefixes of at least eight characters. Diff facets distinguish contract, behavior, facts, and location. Raw source changes remain visible separately. Impact uses old and new graphs, including removals, and supplies deterministic explanation paths.

Qualified declaration names identify symbols within a snapshot. Genuine requirement changes create new adaptation/change work; old Done/PASS tasks remain history for their old requirements. History links are not scheduling prerequisites.

## Explicit identity changes

```sh
dctx0 rename <spec-root> <qualified-symbol> <new-name> --dry-run --json
```

Only explicit rename records continuity. Similarity does not merge ordinary remove/add edits. Data-only `stableId` does not become universal identity. Current rename covers declarations and scoped descendants; fields, parameters, and enum alternatives need dedicated future operations.

Dry runs write neither specs nor history. Real renames coordinate multi-file writes through a project lock and recovery journal; they recompile before publication and preserve unrelated prose/names.

## Share evidence responsibly

A saved native report describes its pinned inputs. Share the specification, scope, provenance, observations, and limitations with it. A historical PASS is not proof for a different implementation or changed contract.

The current project supplies local CLI workflows and this public documentation site. It does not supply a hosted context-pack registry, upload service, paid plans, or ranking backend.
