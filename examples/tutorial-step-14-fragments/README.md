# Schritt 14: Fragmente und mehrere Spec-Dateien

workspace.dctx0 enthält unverändert die vollständigen bisherigen Anforderungen.
copy.dctx0 erweitert die bestehende Domain mit `fragment Tutorial.Board`.
Der Compiler entdeckt beide Dateien, führt sie zusammen und löst Referenzen
im gemeinsamen Workspace auf. Ein Fragment erzeugt keine zweite Domain.

CopyTitle referenziert `Tutorial.Board.Title` ausdrücklich qualifiziert. Die
neue Copy-Oberfläche verwendet die Aliasdeklaration in ihrer Signatur. Ihre
Operation ruft die tatsächliche copyTitle-Funktion auf, die die normale
Title-Factory verwendet. Alle geerbten Refinements bleiben wirksam.

```sh
bun install --frozen-lockfile
bun run typecheck
bun test --coverage src
../../target/release/dctx0 verify complete dctx0 --project .
```

3.710 native Fälle bestehen. Beide Spec-Dateien sind Teil der Provenienz und
der quellgebundenen Inspektion. Die Produktion erreicht 100 % gemessene
Zeilen- und Funktionsabdeckung. Die vollständige Serienvalidierung und
Mutationsabnahme stehen noch aus.

`examples/tutorial/agent-step.sh 14` erstellt vom Repository aus eine frische
Übung mit beiden aktuellen Spec-Dateien und Tests sowie dem Produktionscode
aus Schritt 13. Ein unabhängiger Script-Test prüft gerade diese Dateiauswahl;
historische Freigaben werden nicht kopiert.
