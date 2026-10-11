# Schritt 6: Entities, Schlüssel und öffentliche Messages

Die vorherige Spec und Implementierung bleiben erhalten. `Task` besitzt den
Schlüssel `id`; `Note` den zusammengesetzten Schlüssel `taskId, sequence`.
Die echten Produktionsfactorys validieren alle Felder. Die Board-Instanz
verhindert doppelte Schlüssel bei ihrer Erstellung.

`TaskSummary` ist eine öffentliche Message mit eigener Factory. Sie enthält
nur id und title. Ein interner Task mit zusätzlichem Status ist daher kein
zulässiger TaskSummary-Eingabewert.

Die neue Oberfläche `Tasks` bietet list, find und notes. Ihre `state lists`-
und `state observes`-Klauseln beziehen sich auf echte gespeicherte Entities.
Die Nachbedingungen verwenden `before.Task` und `before.Note`. Der externe
Adapter liest die tatsächlichen Board-Snapshots; er berechnet keine erwarteten
Zustände. `initial-state.json` enthält ausdrückliche Testeingaben. Jede Sitzung
erstellt daraus eine frische Board-Instanz über den normalen Konstruktor.

```sh
bun install --frozen-lockfile
bun run typecheck
bun test --coverage src
../../target/release/dctx0 verify complete dctx0 --project .
```

Der native Checkpoint besteht mit 3.126 Pflichtfällen. Unabhängige Tests prüfen
zusätzlich Schlüsselkonflikte, Treffern/Fehlschläge und isolierte Snapshots.
Die gesamte Produktion erreicht 100 % gemessene Zeilen- und Funktionsabdeckung.
Schreiboperationen und beobachtete Effekte folgen in späteren Schritten;
dieser Schritt behauptet keinen Nachweis für eine Produktionsdatenbank.

`examples/tutorial/agent-step.sh 6` erstellt vom Repository aus die neue Übung
mit der vollständigen aktuellen Spec und dem Produktionscode aus Schritt 5.
