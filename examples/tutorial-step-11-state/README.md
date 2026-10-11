# Schritt 11: Tatsächliche Zustandsänderungen

Die bisherige Spec bleibt erhalten. Der neue Port Catalog.TaskWriter zeigt
alle Schreibmodi an echten gespeicherten Arrays:

- insert fügt einen Task ein und meldet TaskConflict bei einem vorhandenen Schlüssel.
- replace ersetzt einen vorhandenen Task und meldet MissingTask bei einem fehlenden.
- save verwendet upserts: vorhandene Zeile ersetzen oder neue Zeile einfügen.
- remove entfernt die passende Zeile und bleibt für fehlende IDs idempotent.
- saveNote verwendet den zusammengesetzten Schlüssel taskId und sequence.

`state inserts/replaces/upserts/removes` beschreibt die erwartete Veränderung.
`from request` bindet die echten Eingaben ein. Nachbedingungen verwenden
before und after und verlangen unter anderem unveränderten Zustand bei einem
fachlichen Fehlschlag. Die Produktion erstellt keine Zustände aus der Spec:
gewöhnliche Board-Methoden verändern ihre eigenen Arrays.

Der externe Adapter ruft diese Methoden auf. Der vorhandene Ressourcenbeobachter
protokolliert tatsächliche Reads und Writes; die Spec erlaubt sie ausdrücklich.
Vorher-/Nachher-Snapshots lesen dieselbe Board-Instanz. Jede unabhängige Sitzung
beginnt frisch mit den expliziten Testeingaben.

```sh
bun install --frozen-lockfile
bun run typecheck
bun test --coverage src
bun test ./.dctx0/testing/typescript/resource-observer.test.ts
../../target/release/dctx0 verify complete dctx0 --project .
```

3.636 native Pflichtfälle bestehen. Unabhängige Tests prüfen unveränderten
Zustand bei Fehlern, erhaltene fremde Schlüssel, beide Upsert-Zweige und
Rückgabewerte ohne gemeinsame mutable Speicherung. Die Produktion erreicht
100 % gemessene Zeilen- und Funktionsabdeckung. Gesamtvalidierung und gezielte
Mutationen der vollständigen Serie bleiben offen.

`examples/tutorial/agent-step.sh 11` erstellt vom Repository aus diese Spec und
Tests mit dem Produktionscode aus Schritt 10; historische Freigaben fehlen bewusst.

Explizite Zustandsprädikate ergänzen die Modi: exists, creates, updates,
preserves und removes. Die Assertions unterscheiden neue, veränderte und
unveränderte Schlüssel. Keyed after.Task-Zugriffe und zusammengesetzte
after.Note(taskId, sequence)-Zugriffe prüfen echte beobachtete Felder.
Ein erfolgreicher Insert auf einen vorhandenen Schlüssel ist eine bekannte
Vertragsverletzung; fehlende oder mehrdeutige Beobachtungen bleiben UNKNOWN.
