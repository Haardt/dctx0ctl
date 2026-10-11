# Schritt 7: Module, Ownership und austauschbare Ports

Alle bisherigen Anforderungen bleiben erhalten. Das Modul `Catalog` übernimmt
mit `owns` die fachliche Zuständigkeit für Task und TaskSummary. `requires`
benennt seinen Port TaskReader. Public ist die Oberfläche des Moduls; TaskReader
ist der normale austauschbare Zugang zum gespeicherten Zustand.

Die Anwendung verwendet ein gewöhnliches TypeScript-Interface. Catalog erhält
es im Konstruktor. BoardReader ruft die tatsächliche Board-Instanz auf. Weder
eine DI-Bibliothek noch Spec-Imports oder generierte Produktionstypen sind nötig.
Die unabhängigen Tests prüfen die echte Verdrahtung und einen anderen Reader.

Der externe Testadapter prüft BoardReader auch direkt. Beim Aufruf des Consumers
beobachtet ein Wrapper die tatsächlichen Port-Aufrufe und Ergebnisse. Die Spec
verlangt den modellierten call-Effekt und vergleicht das öffentliche Ergebnis
mit den echten before.Task-Beobachtungen. Sie ruft den zustandsabhängigen Port
nicht erneut innerhalb einer Nachbedingung auf.

`PublicMessages` dokumentiert choose/because/avoid. Der Freitext-Constraint
beschreibt die austauschbare Speicherung. Diese Anforderungen werden nicht
automatisch in Assertions verwandelt: source-bound inspection-notes.json
enthält die konkrete Beurteilung der Klassen und ihrer Grenzen. Der native
Bericht behält diese zwei offenen manuellen Punkte; verify complete bindet
ihre Inspektionen als getrennte Evidenz. Native Fehler wären nicht überstimmbar.

```sh
bun install --frozen-lockfile
bun run typecheck
bun test --coverage src
../../target/release/dctx0 verify complete dctx0 --project .
```

3.158 native Pflichtfälle bestehen. Die unabhängigen Produktionstests erreichen
100 % gemessene Zeilen- und Funktionsabdeckung. Gesamtvalidierung und gezielte
Mutationsabnahme der Serie stehen noch aus.

`examples/tutorial/agent-step.sh 7` erstellt vom Repository aus eine frische
Übung mit dieser Spec und dem Produktionscode aus Schritt 6. Bestehende
Inspektionsfreigaben werden ausdrücklich nicht übernommen.
