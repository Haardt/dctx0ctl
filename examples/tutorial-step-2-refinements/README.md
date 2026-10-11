# Schritt 2: Refinements und Identität

Dieser Schritt behält die gesamte vorherige Spec und Implementierung bei.
`scalar` führt unrefinierte Labels ein. Titel zeigen `length`, ein ausführbares
`where` und eine Aliaskette. Die fünf Formate Email, UUID, Uri, Date und DateTime
werden durch tatsächliche Produktionsvalidatoren geprüft. Eine Listenlänge
begrenzt die Anzahl der Einträge. `stableId` gehört zu einer Datendeklaration;
es ersetzt keine explizite Rename-Historie.

Stringlängen verwenden UTF-16 wie TypeScript. Kalendertage werden auf reale
Monats- und Schaltjahrgrenzen geprüft; der Prüfkern übernimmt keinen
Date.parse-Rollover. UUID als Builtin und `format UUID` sind verschieden: die
formatierte Variante verlangt zusätzlich die bekannte Version/Variant-Struktur.

```sh
bun install --frozen-lockfile
bun run typecheck
bun test --coverage src
../../target/release/dctx0 verify complete dctx0 --project .
```

Die gespeicherten Inspektionen sind an die tatsächlich gelesenen Quellen und
die Spec gebunden. Nach Änderungen sind neue inhaltliche Reviews erforderlich.
Eine frische Agent-Übung erstellt `./examples/tutorial/agent-step.sh 2` vom
Repository aus. Sie erhält die neuen Tests und Spec, startet jedoch mit dem
Produktionscode aus Schritt 1.
