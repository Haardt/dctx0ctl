# Schritt 12: Lifecycles und zusammenhängende Sitzungen

Die vollständige bisherige Spec bleibt erhalten. Status erhält die zusätzliche
Alternative Archived. TaskLifecycle beschreibt Open → Done → Archived am
Feld status einer tatsächlich gespeicherten Task-Entity. Beide Übergänge rufen
den echten TaskWriter.replace auf; ihre Guards prüfen den typisierten request.

lifecycles.json liefert ausdrücklich die Schlüssel, tatsächliche Setup-Aufrufe
und geordneten Übergangseingaben für jeden verpflichtenden Pfad. Das Setup
fügt den Task über den normalen Produktionsport ein. Für den längeren Pfad
laufen beide Übergänge in derselben Sitzung. Ein unabhängiger Pfad beginnt
frisch. Der Rust-Prüfkern vergleicht echte Zustände und prüft die Verbindung
zwischen dem Nachher-Zustand und dem nächsten Vorher-Zustand.

TaskPresence zeigt `using presence`: Present ist der vorhandene Anfangszustand,
Removed der fehlende Endzustand. Der tatsächliche remove-Aufruf muss die Entity
mit dem ausgewählten Schlüssel entfernen. Die Prüfung betrachtet gespeicherten
Zustand, nicht die Unit-Rückgabe. Fehlende Beobachter, mehrdeutige Schlüssel
oder unvollständige Pfade erhalten keinen PASS.

```sh
bun install --frozen-lockfile
bun run typecheck
bun test --coverage src
bun test ./.dctx0/testing/typescript/resource-observer.test.ts
../../target/release/dctx0 verify complete dctx0 --project .
```

Der Checkpoint besteht mit 3.656 nativen Fällen einschließlich aller drei
Pflichtpfade und ohne offene Tasks. Die unabhängigen
Produktionstests bestehen mit 100 % gemessener Zeilen- und Funktionsabdeckung.
Gesamtvalidierung, Lifecycle-Mutationen und Replay der Serie stehen noch aus.

`examples/tutorial/agent-step.sh 12` erstellt vom Repository aus die aktuelle
Spec und Tests mit dem Produktionscode aus Schritt 11. Die überprüften
Lifecycle-Eingaben bleiben erhalten; frühere Inspektionsfreigaben werden nicht kopiert.
