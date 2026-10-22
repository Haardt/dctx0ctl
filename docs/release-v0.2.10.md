## Searchable JSON reports and parallel CLI guidance

CLI JSON output now uses indented lines and a trailing newline. Large redirected
reports can be searched without treating the entire document as one line.
Verification GC preserves multiline formatting when rewriting reduced JSON exports.
Existing saved files are unchanged; internal hashes, native transport formats,
task definitions and SQLite schemas are unchanged. Consumers must parse JSON
rather than depend on the previous compact whitespace layout.

Embedded skills explain that independent dctx0 commands can run concurrently
against the same local SQLite database. WAL, serialized write transactions,
project locks and guarded task claims protect shared state; application and
specification file edits still need coordinated ownership. Skills also explain
how to preserve report line breaks and inspect old compact exports with `jq`.

Publication requires the normal CI validation and all five native platform gates.
No validation exception or benchmark baseline promotion is introduced.
