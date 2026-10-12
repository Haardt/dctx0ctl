# Task-API: Funktionsentwurf

Implemented behavior is documented in [tasks.md](tasks.md); the text below records the earlier design and analysis.
Stand: 2026-10-09. Dies sind vorgeschlagene Funktionen, keine bereits vorhandenen Rust-Kommandos. Grundlage sind die Lebenszyklusregeln in `tasks-analysis.md`: fachliche Planung ohne Targets, neue Aufgaben bei relevanten Changes, historische Vorgänger und getrennte Arbeits-/Prüfzustände.

## Abfragen

Eine gemeinsame Listenabfrage deckt Kombinationen ab. Es werden keine separaten Funktionen für „alle offenen Operation-Tasks in Welle 2“ benötigt.

| Funktion | Ergebnis |
| --- | --- |
| `tasks.list(filters)` | Tasks nach Plan, Welle, Arbeitsstand, Prüfergebnis, Aufgabentyp und fachlichem Subject filtern. |
| `tasks.next(filters)` | Nächste startbare offene Aufgabe samt Auswahlgrund; reserviert nichts. |
| `tasks.waves(plan_id)` | Wellen mit Gesamtzahl, Offen/Progress/Done, Prüfergebnissen und Blockierungsgründen. |
| `task.show(task_id)` | Auftrag, Basis-/Ziel-Snapshot, Zustand, Voraussetzungen, Readiness und Ergebnisbeschreibung. |
| `task.context(task_id, selection)` | Intents, Constraints, Entscheidungen, Beispiele, Akzeptanzkriterien, geänderte Fakten und gelieferte Vorgängerergebnisse. |
| `task.history(task_id, query)` | Ereignisse und begründete Vorgänger-/Nachfolgerbeziehungen, einschließlich alter Anforderungen und Prüfergebnisse. |
| `task.dependencies(task_id, query)` | Direkte oder transitive Voraussetzungen bzw. abhängige Tasks mit Gründen und Ergebnisverweisen. |
| `tasks.diff(before_plan, after_plan)` | Neue, erhaltene und aus dem aktuellen Plan entfallene Tasks sowie ihre Change-Beziehungen. |

Beispiele für dieselbe Listenfunktion:

```text
tasks.list(wave=2)
tasks.list(work_state=Open)
tasks.list(wave=2, work_state=Open)
tasks.list(kind=Review)
tasks.list(check_status=FAIL)
tasks.next(wave=2)
task.show("T42")
task.history("T42", direction=Both)
```

Der Aufrufkontext nennt das Projekt. Ohne ausdrückliche Planangabe wird der aktuelle gespeicherte Plan verwendet. Historische Task-Details sind weiterhin über ihre IDs lesbar. Eine planübergreifende Liste ist eine ausdrückliche Option; entfallene alte Aufgaben erscheinen nicht versehentlich als aktuelle Arbeit. Eine Aufgabe kann in mehreren unveränderten Planständen enthalten sein; Planzugehörigkeit und Arbeitsstand sind verschiedene Eigenschaften.

Listen haben deterministische Sortierung und Cursor-Paginierung. Kontext-/Historienabfragen benötigen ebenfalls begrenzte Seiten, wenn ihre Inhalte groß werden. Rückgaben nennen tatsächlichen Plan und Snapshot. Sie behaupten keine Frische gegenüber ungeprüften Arbeitsdateien. Lesen plant, prüft, schreibt und reserviert nichts.

## Auswahl und Parallelität

`tasks.list(work_state=Open)` enthält auch wartende offene Aufgaben. `tasks.next()` liefert dagegen ausschließlich offene, startbare Aufgaben. Eine vorgegebene Welle schränkt die Auswahl ein; ohne Vorgabe wird die erste Welle mit passenden startbaren offenen Tasks verwendet. Eine unabhängige noch laufende Aufgabe erzeugt keine zusätzliche Voraussetzung.

Voraussetzungen werden durch die jeweils benötigten Ergebnisse erfüllt. Eine Entwicklungsaufgabe benötigt normalerweise ein geliefertes Ergebnis; ein erforderliches PASS ist eine ausdrückliche Prüfvoraussetzung und wird nicht pauschal unterstellt. Historische Change-Verweise sind keine Wartebedingungen.

Gibt es keinen passenden nächsten Task, liefert die Funktion einen strukturierten Grund: keine passenden offenen Aufgaben, fehlende Voraussetzungsergebnisse oder explizite fachliche Blockaden. Tasks in Progress werden nicht einem weiteren Agenten als neue Arbeit angeboten. Ein bereitstehender Task bedeutet fachliche Bereitschaft; später notwendige Ressourcen-/Dateisperren gehören zur Ausführung.

## Planung und Arbeitsaktionen

| Funktion | Zustandsänderung |
| --- | --- |
| `tasks.preview(snapshot_id)` | Plan und Changes berechnen, ohne Speicherung. |
| `tasks.plan(snapshot_id)` | Versionierten Plan atomar veröffentlichen; neue Change-Tasks mit Vorgängern verbinden. |
| `task.start(task_id, actor_id)` | Einen offenen aktuellen Task atomar übernehmen und auf Progress setzen. |
| `tasks.claim_next(filters, actor_id)` | Auswahl und Übernahme des nächsten Tasks in einer Transaktion; verhindert doppelte Agent-Zuteilung. |
| `task.release(task_id, reason)` | Eigene, noch nicht abgeschlossene Bearbeitung von Progress zurück auf Open geben; Versuch und Grund erhalten. |
| `task.complete(task_id, result)` | Geliefertes Ergebnis speichern und auf Done setzen; erzeugt kein PASS. |

`tasks.next` dient der Anzeige. Parallele Agenten verwenden `claim_next`, damit zwei Agenten nicht dieselbe Aufgabe starten. Übernahme, Zustandswechsel und Abschluss benötigen Versions-/Übernahmeprüfungen, um konkurrierende Änderungen zu erkennen. Die konkrete Agent-Ausführung bleibt ein späterer Baustein.

Ein Ergebnis enthält mindestens eine verständliche Zusammenfassung; relevante Artefaktverweise und Umsetzungsentscheidungen werden ebenfalls gespeichert. Vorgänger-Kontext muss dieses Ergebnis abrufen können. Ungültige Übergänge oder ein Start eines überholten bzw. abgeschlossenen Tasks liefern einen Fehler. Es gibt keine pauschale Status-Setter-Funktion und keine automatische Wiederöffnung historischer Done/PASS-Aufgaben.

Überholte offene Arbeit wird durch Planzugehörigkeit/Supersession erkennbar, ohne sie fälschlich fertigzustellen. Wiederholte Planung desselben Auftrags erzeugt keine Duplikate. Spätere relevante Changes erzeugen neue Aufgaben, einschließlich Reparaturaufgaben für historische abgeschlossene Arbeit.

## Spätere Ergänzungen

| Funktion | Zweck |
| --- | --- |
| `task.record_check(task_id, evidence)` | Tatsächliches PASS/FAIL mit Anforderungen, Implementierungsstand, Prüfmethode und Bericht festhalten. |
| `task.record_review(task_id, report)` | Review-Feststellungen, Schweregrade, betroffene Subjects und Begründungen speichern. |
| `task.add_note(task_id, note)` | Zusammenarbeit und zusätzliche Begründungen nachvollziehbar dokumentieren. |

Ein fehlender Checker bleibt ausdrücklich ungeprüft. Ein historisches PASS gilt für den damals geprüften Auftrag/Implementierungsstand und wird nicht auf neue Tasks übertragen. Eine manuelle Meldung oder LLM-Einschätzung ist kein automatischer Testnachweis.

## Aufgabentypen und Reviews

Für den gemeinsamen Kern bieten sich `Contract`, `Operation`, `Adapter`, `Integration`, `Review` und bei ausdrücklichem Bedarf `Clarification` an. Die fachliche Granularität bleibt ein Entwurf. Die Sprache der Umsetzung wird nicht zum Aufgabentyp.

Im gelesenen Original gibt es keinen regulären `review`-Eintrag in `ImplementationTaskKind`. LLM-Review läuft als separater Workflow-/Review-Schritt mit Agent-Aufruf:

- [Originale Task-Arten](https://github.com/Haardt/runatron/blob/main/packages/plugin-dctx0/src/dctx0/implementation/types.ts)
- [Review im Workflow](https://github.com/Haardt/runatron/blob/main/packages/plugin-dctx0/src/dctx0/surface/workflow-definition.ts)
- [Aufruf des Review-Agenten](https://github.com/Haardt/runatron/blob/main/packages/plugin-dctx0/src/dctx0/review/reviewer.ts)

Vorschlag: Review als reguläre Aufgabe behandelbar machen, wenn die Spec oder Projektregel eine konkrete Prüfung verlangt. Der Auftrag nennt Prüfgegenstand, Kriterien und erwarteten Bericht. Der Prüfgegenstand kann ein einzelnes Ergebnis oder die Integration mehrerer Ergebnisse sein. Ein Review benötigt die gelieferten Gegenstände und liegt dadurch in einer späteren Welle. Ob Mensch oder LLM das Review übernimmt, ist Umsetzungskontext. Die Planung braucht keinen LLM-Aufruf.

Feststellungen führen bei notwendiger Nacharbeit zu neuen, begründeten Aufgaben mit Verweisen auf Review und betroffene Vorgänger. Kein Review für jede Aufgabe pauschal erzeugen. Review-Befund und automatisches Testing-PASS bleiben getrennt.

Kontextaufteilung, gemessene Referenztokens und Slice-Vorschauen für spätere Aufgaben sind im [Kontext-Entwurf](tasks-context.md) beschrieben. Transitive Voraussetzungen liefern zunächst Übersichten; vollständige Vorgängeraufträge werden gezielt geladen. Eine mögliche spätere `claim_slice`-Aktion reserviert eine geschlossene Aufgabenmenge, erhält aber die individuellen Zustände und Voraussetzungen.

## Erste Umsetzung

Die erste Version sollte Listenfilter, Wellenübersicht, Show/Context/History/Dependencies, Next, Preview/Plan und die normalen Arbeitsübergänge anbieten. Bei paralleler Übernahme gehört `claim_next` dazu. Planvergleich nutzt die ohnehin erforderliche Reconciliation. Prüfer-/LLM-Ausführung, Review-Automatik, Kommentare und zusätzliche Komfortfunktionen können danach folgen.

Rust-Produktionscode wurde für diesen Entwurf nicht verändert. Sobald diese Funktionen implementiert werden, gelten die vorhandenen Tests/Coverage- und Speed/RAM-Pflichten für beide vollständigen Beispiele.
