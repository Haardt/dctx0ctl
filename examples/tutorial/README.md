# dctx0 Schritt für Schritt: MiniBoard

16 eigenständige TypeScript-Anwendungen erweitern dasselbe kleine Task-Board.
Jedes Verzeichnis enthält die vollständige Spec dieses Schritts, echten
Produktionscode, unabhängige Tests und einen externen Prüfadapter. Vorherige
Anforderungen bleiben erhalten. Produktion importiert keine dctx0-Runtime,
Spec-Dateien oder generierten Prüfartefakte.

Der Abgleich mit den tatsächlichen Sprachregistern, AST-Knoten und typisierten
Prüfausdrücken belegt 158 Features: sämtliche registrierten Statements,
Kontextknoten, Builtins, Operatoren, Funktionen, Formate, Ausdrucksarten und
Zustandsprädikate. [features.json](features.json) führt den Umfang auf;
[validation/features.json](validation/features.json) nennt den ersten Schritt
und die Quellposition jedes Features. Worte in Kommentaren und Intents zählen
nicht als ausführbare Feature-Nachweise.

Alle 16 aktuellen Produktionsstände bestehen ihre endlichen nativen Pläne:
insgesamt 43.490 Pflichtfälle, passende quellgebundene Inspektionen, keine
unfertigen oder nicht bestandenen aktiven Tasks und 100 % gemessene
Produktionszeilen/-funktionen. Die Browseroberfläche wird zusätzlich in echtem
Chromium geprüft. [validation/mutations.json](validation/mutations.json)
belegt 21 negative Prüfungen einschließlich Lifecycle-/HTTP-Replay. Die
abschließende Workspace-/Performance-Abnahme bleibt bis zu ihrem gespeicherten
Gesamtbericht separat erkennbar.

## Voraussetzungen und erster Lauf

Rust baut die CLI; Bun 1.4.2 führt TypeScript und dessen Tests aus. Die kompletten
nativen Läufe benötigen Linux mit erreichbarem systemd-User-Manager für die
Begrenzung auf zwei Drittel des verfügbaren RAM ohne Swap. Diese Prozessgrenze
ist keine Sicherheitssandbox. TypeScript 6.0.3 und die Bun-Typen sind pro Projekt
im Lockfile gepinnt. Python ist für diese Produktbefehle und Tutorial-Skripte
nicht erforderlich; der gesamte Repository-Entwicklungslauf verwendet weiterhin
den vorhandenen Python-Runner.

```sh
cargo build --release -p dctx0-cli --all-features --locked
cd examples/tutorial-step-1-values
bun install --frozen-lockfile
bun run typecheck
bun run test --coverage
../../target/release/dctx0 verify complete dctx0 --project .
```

`verify complete` veröffentlicht einen aktuellen Check, führt den ausdrücklich
konfigurierten Plan aus und bindet anschließend vorhandene, überprüfbare
Inspektionen. Es genehmigt keine Architekturprosa automatisch. Native Fehler
können durch einen Review nicht überstimmt werden. Die Referenz enthält die
verfassten Inspektionen; eine neu bearbeitete Implementierung braucht neue
inhaltliche Prüfung, bevor deren Quellhashes aktualisiert werden.

## Einen Schritt selbst umsetzen lassen

```sh
./examples/tutorial/agent-step.sh 1
./examples/tutorial/agent-step.sh 16 -- codex exec --full-auto
```

Das Skript erzeugt eine neue temporäre Arbeitskopie mit aktueller Spec, Adapter,
Fixtures und Akzeptanztests. Der Produktionscode stammt vom vorherigen Schritt;
bei Schritt 1 ist er leer. Die vollständigen Referenzlösungen im Git bleiben
unverändert. Es kopiert weder SQLite-Historie noch alte Reviewfreigaben.
Der Agent muss tatsächlichen Code liefern, alle Tests ausführen und verbleibende
manuelle Anforderungen selbst prüfen. Für Schritt 16 sind auch der tatsächliche
Server-Start und die interaktive Webseite gefordert.

`DCTX0_BIN` wählt ein anderes Rust-Binary. Ein optionaler Agent-Befehl läuft in
der neuen Arbeitskopie und erhält die Aufgabe als eigenes Argument.

## Die Schritte

| Schritt | Erweiterung |
| --- | --- |
| [1](../tutorial-step-1-values/README.md) | Werte, endlicher Bereich, reine Operation, Beispiel und forall |
| [2](../tutorial-step-2-refinements/README.md) | Aliase, Refinements, alle Formate und stableId |
| [3](../tutorial-step-3-data/README.md) | Records, Varianten und alle Builtins mit eigenen Repräsentationen |
| [4](../tutorial-step-4-expressions/README.md) | Operatoren, Konstruktionen und Operationsaufrufe |
| [5](../tutorial-step-5-collections/README.md) | Collection-, String- und exakte Dezimalfunktionen |
| [6](../tutorial-step-6-entities/README.md) | Entities, einfache/zusammengesetzte Schlüssel, Messages und Zustandslesen |
| [7](../tutorial-step-7-boundaries/README.md) | Module, Ownership, Ports, Surfaces, Constraints und Decisions |
| [8](../tutorial-step-8-mocks/README.md) | given, echte Consumer unter Mocks und exakte beobachtete Aufrufzahlen |
| [9](../tutorial-step-9-failures-events/README.md) | Echte Fehler, Event-Payloads und exakte Emissionszahlen |
| [10](../tutorial-step-10-effects/README.md) | Tatsächlich beobachtete Reads, Calls, Emits und externe Clock |
| [11](../tutorial-step-11-state/README.md) | Writes, alle Zustandsmodi/-prädikate und vollständige keyed before/after-Zugriffe |
| [12](../tutorial-step-12-lifecycle/README.md) | Verbundene Status-/Presence-Lifecycles mit typisierten Guards |
| [13](../tutorial-step-13-external/README.md) | Explizite externe HTTP-Grenze und realer Client/Handler |
| [14](../tutorial-step-14-fragments/README.md) | Mehrere Spec-Dateien, Fragmente und qualifizierte Auflösung |
| [15](../tutorial-step-15-reviews-policy/README.md) | Alle benannten Suiten, Review-Kriterien/-Fragen und Release-Policy |
| [16](../tutorial-step-16-http-replay/README.md) | Tatsächlicher HTTP-Server, interaktives MiniBoard, Mutation und Replay |

## Alles prüfen

Vom Repository-Verzeichnis aus:

```sh
bun examples/tutorial/verify-series.ts
bun examples/tutorial/mutation-acceptance.ts
```

Der erste Befehl installiert eingefrorene Abhängigkeiten, prüft TypeScript und
alle Produktionsmodule mit LCOV, verlangt 100 % Zeilen/Funktionen für jede
Produktionsdatei, führt die nativen Pläne aus und prüft alle Pflichtfälle und
aktiven Tasks. Er erhält tatsächliche Worker-RSS-Messungen und Quellhashes.
Mit `bun examples/tutorial/verify-series.ts 12 16` lässt sich ein Ausschnitt
prüfen; der Bericht behauptet dann keine vollständige Serienabnahme.

Der zweite Befehl verändert ausschließlich neue temporäre Kopien. Er testet
spezifische Fehler in jeder fachlichen Etappe, eine nachgebaute Adapterantwort,
falsche HTTP-Inserts/Replacements und die vier Review-/Provenienzschutzfälle.
Er verlangt tatsächliche zentrale FAIL-Ergebnisse für Code-Mutationen.
Lifecycle- und HTTP-Replay verwenden dieselben gespeicherten Eingaben; geänderter
Code wird als neuer Vergleich mit eigener Provenienz ausgewiesen. Es ändert
keinen Task-Fortschritt der Referenzprojekte.

Der Browsercheck und seine V8-Abdeckung stehen im
[letzten Schritt](../tutorial-step-16-http-replay/README.md). Die gekoppelte
Repository-Abnahme führt die langen Prüfungen gemeinsam aus und misst Release-
Phasen am Ende; die akzeptierte Baseline bleibt unverändert:

```sh
PATH="$HOME/.cargo/bin:$PATH" python3 scripts/validate.py --coverage --example cinema --tutorial
```

## Das Board ansehen

```sh
cd examples/tutorial-step-16-http-replay
bun run start
```

Öffne <http://127.0.0.1:4173/>. Aufgaben lassen sich anlegen, als erledigt markieren
und wieder öffnen. Die echte API verwendet die bestehenden Board-Methoden.
Ein Neustart stellt die drei Beispieldatensätze wieder her; die kleine App nutzt
bewusst Arbeitsspeicher. `PORT=4180 bun run start` wählt einen anderen Port.

## Was PASS aussagt

PASS belegt sämtliche Pflichtprüfungen des ausgewiesenen endlichen Plans,
zusammen mit ausdrücklich getrennten quellgebundenen Inspektionen. Größere
Eingaberäume bleiben deterministisch begrenzt: Seed 42, höchstens 100 generierte
Fälle pro Obligation und Collection-Größe 4. Diese Grenzen sind keine heimliche
Kürzung expliziter Pflichtfälle. Es ist kein Beweis über alle möglichen Eingaben
oder beliebige Architekturprosa. Beobachtete Mocks belegen ihre echten Consumer,
nicht automatisch einen Produktionsadapter.

Die gemessene TypeScript- und Browser-Abdeckung ist keine vollständige Branch-
Abdeckung. Browserzeilen werden anhand der innersten V8-Bereiche an ausführbaren
Zeilenanfängen bewertet; die Rohbereiche bleiben erhalten. Unmodellierte Labels,
fehlende Beobachter, Ressourcenabbrüche und unvollständige Pläne bleiben offen.
Absichtlich negative Prüfkopien werden niemals als erfolgreiche Anwendung
übernommen. Bewusste Semantik-Unterschiede dokumentiert
[der Sprachhinweis](../../docs/tutorial-language-semantics.md).
