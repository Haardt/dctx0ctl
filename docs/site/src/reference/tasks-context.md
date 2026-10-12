# Task-Kontext und eigenständige Arbeitsslices

Implemented behavior is documented in [tasks.md](tasks.md); the text below records the earlier design and analysis.
Stand: 2026-10-09. Entwurf auf Basis der Task-Diskussion; die beschriebene Rust-API ist noch nicht implementiert.

## Kontext schrittweise bereitstellen

Eine Task-Definition bleibt ein zusammengehöriger Auftrag. Ihre Darstellung wird in gezielt abrufbare Abschnitte aufgeteilt. SQLite hält den gespeicherten Stand; die API erzeugt Ansichten daraus, ohne weitere führende Kontextdateien zu benötigen.

Der Erstkontext enthält:

- Ziel, fachlichen Subject, aktuellen Arbeits-/Prüfstand und Basis-/Ziel-Snapshot.
- Relevante Intents, verbindliche Regeln, den aktuellen Änderungsauftrag und Akzeptanzkriterien.
- Den eigenen Operationsvertrag, einschließlich Eingaben, Ergebnissen, Fehlerfällen und Bedingungen.
- Kurze Übersichten der Voraussetzungen: ID, Ziel, Zustand, Abhängigkeitsgrund, gelieferter Vertrag/Ergebnisüberblick und Detailreferenz.
- Kurze historische Bezüge: Vorgänger-ID, damaliger Zweck, Change-Grund und Ergebnisreferenz.
- Einen kleinen Katalog weiterer erforderlicher/verfügbarer Abschnitte und deren Umfang.

Eigene verbindliche Regeln dürfen nicht allein durch vage Zusammenfassungen ersetzt werden. Lange Beispiele, Typdefinitionen, Lifecycle-Regeln oder weitere notwendige Inhalte können auf mehrere Seiten verteilt werden. Die Antwort muss dann ausdrücklich nennen, welche Pflichtinhalte noch nachzuladen sind. Eine unvollständige Übersicht gilt nicht als vollständiger Arbeitsvertrag.

Historische Aufgaben und Vorgänger werden zunächst nicht vollständig eingebettet. Ein Agent lädt einzelne Aufgaben oder Abschnitte über dieselbe Kontextfunktion gezielt nach:

```text
task.context("T42", view=Brief)
task.context("T42", sections=[Examples, Types])
task.context("T17", sections=[Intent, Result])
task.history("T42", view=Overview)
```

Ein Vorgänger in der Historie begründet den aktuellen Änderungsauftrag. Eine Voraussetzung im Arbeits-DAG liefert benötigte Ergebnisse. Die Übersicht kennzeichnet diese unterschiedlichen Beziehungen.

Referenzen und Cursor gehören zum gepinnten Plan-/Snapshot-Stand. Ein bestehender historischer Kontext bleibt abrufbar, auch wenn inzwischen ein neuer Plan veröffentlicht wurde. Für aktuelle Arbeit dürfen neue und alte Anforderungen nicht unbemerkt gemischt werden. Über Budget liegende Inhalte werden ausdrücklich paginiert, statt still abgeschnitten.

## Gemessene Tokens der Referenzartefakte

Die Analyse zählt vorhandene historische Artefakte: 48 Cinema-Tasks und 513 Tasks aus `../runatron-next`. Als Referenztokenizer dient `tiktoken 0.12.0` mit `o200k_base`. Gezählt wird der tatsächliche Text der gespeicherten Dateien, ohne Systemprompt, Chat-Wrapper, Toolschemas, Produktcode oder spätere Tool-Antworten. Das ist kein modellübergreifend exakter Verbrauchswert.

| Operation-Tasks | Cinema (13) | runatron-next (177) |
| --- | ---: | ---: |
| Gespeicherter Startprompt: Median | 2.605 | 2.727 |
| Gespeicherter Startprompt: 90. Perzentil | 2.809 | 3.339 |
| Gespeicherter Startprompt: Maximum | 2.820 | 5.221 |
| Vollständige formatierte `context.json`: Median | 32.398 | 68.908 |
| Vollständige formatierte `context.json`: Maximum | 39.704 | 490.663 |
| Kompakter Überblick-Prototyp: Median | 98 | 96 |
| Fachlicher Kern-Prototyp: Median | 1.108 | 2.349 |
| Fachlicher Kern-Prototyp: 90. Perzentil | 1.866 | 4.999 |
| Fachlicher Kern-Prototyp: Maximum | 2.117 | 6.655 |

Der Überblick-Prototyp enthält ID, Typ, Subject, ursprüngliche Beschreibung, Welle, Zahl der Voraussetzungen und Detailreferenz in kompaktem JSON. Der Kern-Prototyp enthält die eigene Operation, Akzeptanzkriterien und alle im gespeicherten Kontext bereitgestellten Intents, Constraints, Entscheidungen, Beispiele, Invarianten und Lifecycles. Typdefinitionen werden in diesem Prototyp nur referenziert; die erforderlichen vollständigen Typdetails kosten zusätzliche Tokens. Die Projektion ist ein Größenexperiment, keine implementierte oder abschließend auf Vollständigkeit geprüfte Task-Darstellung.

Das Original liefert bereits begrenzte Start-Envelopes und abschnittsweisen Kontext. Die großen `context.json`-Zahlen dürfen deshalb nicht als automatisch gesendeter Erstprompt interpretiert werden. Die gespeicherten Original-Envelopes sind zudem nicht automatisch vollständige fachliche Aufträge: Sie enthalten unter anderem begrenzte Faktenvorschauen und Abschnittsreferenzen.

Große Integrations-/Bindings-Kontexte enthalten besonders viel wiederholte Information. Im historischen runatron-next-Inventar erreicht die größte vollständige Kontextdatei ungefähr 6,84 Millionen Referenztokens; der zugehörige Startprompt bleibt begrenzt. Das unterstreicht die Bedeutung gezielter Projektion.

Als anfängliche Planungsgröße bieten sich ungefähr 100 Tokens pro Überblick und etwa 2.000–6.000 Tokens für einen gewöhnlichen fachlichen Erstkontext an. Komplexe Aufgaben können darüber liegen. Verbindliche Regeln bestimmen den Inhalt; das Budget bestimmt Darstellung und Nachladeverfahren. Benötigte Definitionen, Produktcode und der gesamte Agent-Dialog kommen hinzu. Der Gesamtverbrauch einer Umsetzung lässt sich aus diesen Dateien allein nicht bestimmen.

Alle Einzelzählungen, Artefakt-Prüfsummen, Zusammenfassungen, Projektionsbeschreibungen und der verwendete Analysecode stehen in task-context-sizing.json (historical evidence removed during repository cleanup). Die Analyse verändert weder Specs noch Rust-Code und ist kein Test-/Speed-/RAM-Benchmark.

Quellen im Original: [Envelope und Voraussetzungenübersicht](https://github.com/Haardt/runatron/blob/main/packages/plugin-dctx0/src/dctx0/agent-context/task-projector.ts), [Kontextabschnitte](https://github.com/Haardt/runatron/blob/main/packages/plugin-dctx0/src/dctx0/agent-context/task-context-projector.ts), [Seitenausgabe](https://github.com/Haardt/runatron/blob/main/packages/plugin-dctx0/src/dctx0/agent-context/page-projector.ts).

## Ein späterer Task als Slice-Anker

Ein späterer Task kann den Einstieg in eine zusammenhängende Arbeitseinheit bilden. Dazu wird die transitive Hülle seiner echten Voraussetzungen berechnet. Es werden nicht sämtliche Aufgaben früherer Wellen und nicht automatisch sämtliche historischen Vorgänger geladen.

Der erste Abruf zeigt alle erforderlichen Aufgaben bis zu den Wurzeln als Übersichten mit Kanten und Gründen. Anschließend werden zwei Mengen unterschieden:

- Bereits gelieferte Voraussetzungen bilden den Rand des Slices. Der Agent benötigt ihren gültigen Vertrag, ein Ergebnisüberblick und Artefaktreferenzen, bei Bedarf Details.
- Noch offene Voraussetzungen bilden zusammen mit dem Anker die zu bearbeitende Teilmenge. Der Agent kann deren vollständige Aufträge gezielt laden.

Eine strukturelle Slice-Vorschau nennt Aufgaben, interne Wellen/Reihenfolge, externe Voraussetzungen, fehlende fachliche Informationen und Aufgaben, die bereits anderen Bearbeitern gehören. Sie prüft, ob alle Voraussetzungen entweder enthalten oder durch passende bestehende Ergebnisse geliefert sind. Eine Domain-Einschränkung darf externe Voraussetzungen nicht verschweigen; andernfalls ist der Slice nicht eigenständig bearbeitbar.

Die mechanische Vollständigkeit sagt noch nichts über die Fähigkeiten des Agenten oder erfolgreiche Implementierung aus. Der Agent beurteilt anhand der vollständigen benötigten Aufträge, ob er den Slice übernehmen kann.

Ein Größenbeispiel aus dem historischen runatron-next-DAG: Ein Operation-Task in Welle 2 hat einschließlich seiner transitiven Voraussetzungen 13 Aufgaben. Ihre kompakten Übersichten benötigen zusammen 1.120 Referenztokens; alle gespeicherten Einzelprompts zusammen 36.226 Tokens. Dieses Beispiel belegt den Unterschied zwischen Übersicht und vollständigen Aufträgen, nicht die fachliche Autarkie des alten Slices. Es ist in der Messdatei mit Anker-ID dokumentiert.

## Ausführung eines Slices

Die vorhandene vorgeschlagene `task.dependencies`-Abfrage kann um transitive Übersichten und Slice-Metadaten ergänzt werden. Eine ausdrückliche Bundle-Übernahme wie `tasks.claim_slice(anchor, actor, preview_revision)` wäre ein späterer Verwaltungsfall.

Sie reserviert die benötigte offene Teilmenge atomar und erkennt konkurrierende Bearbeiter sowie Planänderungen. Reservierung und Arbeitsstand bleiben getrennt: Spätere Tasks bleiben zunächst Open; sie wechseln erst beim tatsächlichen Start nach Progress, wenn ihre Voraussetzungen geliefert sind. Im Slice wird in topologischer Reihenfolge gearbeitet. Ein Agent kann so mehrere aufeinander aufbauende Aufgaben erledigen.

Die individuellen Tasks bleiben erhalten. Jeder Abschluss speichert sein eigenes Ergebnis, seinen Arbeitsstand und seine späteren Prüfberichte. Ein Bundle wird nicht pauschal Done/PASS gesetzt; historische Aufgaben werden nicht wieder geöffnet. Unabhängige Slices können fachlich parallel bearbeitet werden, während spätere technische Ressourcenkonflikte weiterhin durch die Ausführung behandelt werden.

Lesen oder Betrachten eines späteren Tasks startet keine Arbeit und gibt keine zusätzlichen Aufgaben automatisch frei. Für die erste Task-Version sind kompakter Kontext und transitive Voraussetzungenübersichten ausreichend; eine atomare Slice-Reservierung kann darauf folgen.
