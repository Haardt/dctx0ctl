# Task-Erstellung: Analyse und Vorschlag

Implemented behavior is documented in [tasks.md](tasks.md); the text below records the earlier design and analysis.
Stand: 2026-10-09. Dies ist eine Analyse des lokal vorhandenen Originals und ein Entwurf für den nächsten Rust-Schritt. Task-Planer, Task-DAG und Task-CLI sind hier noch nicht implementiert. Der Interpreter wird auf Wunsch zunächst zurückgestellt.

## Präzisierte Anforderungen aus der Diskussion

Die Task-Erstellung ist fachlich und target-neutral. Sprache, Framework, Dateiprojektion und native Prüfung werden später als Umsetzungskontext ergänzt. Tasks beschreiben Ziele, Intents, Entscheidungen, Constraints, Beispiele und Akzeptanzkriterien; sie geben keine bereits ausgeführten Prüfungen vor.

Eine Welle enthält Aufgaben ohne gegenseitige Voraussetzungen. Die erste Welle ist fachlich vollständig parallel bearbeitbar. Spätere Aufgaben verwenden gelieferte Ergebnisse früherer Aufgaben. Eine Kontextabfrage wie `task_context(task_id)` liefert den gepinnten Auftrag, relevante Spec-Inhalte, begründete Voraussetzungen und Vorgängerergebnisse einschließlich Artefaktverweisen. Dies ist noch keine implementierte Agent-Funktion.

Arbeitsstand und Prüfergebnis sind getrennte Dimensionen:

| Dimension | Werte | Aussage |
| --- | --- | --- |
| Arbeit | Offen, Progress, Done | Umsetzung wurde begonnen oder als geliefert gemeldet. |
| Testing | Ungeprüft, PASS, FAIL | Tatsächliches Prüfergebnis für konkrete Anforderungen und einen Implementierungsstand. |
| Bereitschaft | Abgeleitet mit Gründen | Welche Voraussetzungen fehlen noch? |

Die Anzeige kann `Done · PASS` kompakt als PASS darstellen, muss die zugrunde liegenden Aussagen aber erhalten. Done impliziert keinen Testerfolg. Automatische Ausführung und Prüfungen bleiben ein späterer Schritt.

**Fachliche Änderungen erzeugen neue Aufgaben; abgeschlossene alte Aufgaben werden nicht wieder geöffnet.** Ein neuer Task beschreibt die Anpassung der bestehenden Umsetzung und verweist auf die relevanten historischen Tasks. Er nennt Basis-/Ziel-Snapshot, betroffene Subjects, geänderte Regeln/Beispiele und die Begründung des Zusammenhangs. Anforderungen eines historischen Tasks bleiben erhalten; Status- und Testergebnisse werden als nachvollziehbare Ereignisse erfasst.

Beispiel: T17 hat die Preisberechnung nach Spec A umgesetzt und PASS erhalten. Spec B ergänzt eine Rabattregel. T42 beschreibt diese Änderung und bezieht sich auf T17. T17 bleibt historisch PASS für A; T42 beginnt offen und ungeprüft für B. Der Agent erhält den alten Intent, das gelieferte Ergebnis und das konkrete Delta. Aus T17 folgt kein PASS für B.

Historienbeziehungen und Arbeitsabhängigkeiten bleiben getrennt. Ein Verweis „ändert T17“ blockiert T42 nicht automatisch. Falls T42 einen neuen Vertrag von T41 benötigt, ist dagegen T41 eine echte Voraussetzung. Beziehungen können mehrere alte/neue Tasks verbinden, etwa bei Aufteilung, Zusammenführung oder einer Änderung an mehreren fachlichen Bereichen.

Wiederholte Planung desselben aktuellen Auftrags erzeugt keine Duplikate. Unveränderte Aufgaben behalten ihre IDs. Ein späterer Rückwechsel zu früheren Anforderungen erzeugt hingegen eine neue Anpassungsaufgabe; identische historische Fingerprints beweisen keinen aktuellen Abschluss. Wird noch offene Arbeit überholt, kann sie aus dem aktiven Plan herausfallen. Ihr historischer Status bleibt erhalten; sie wird nicht fälschlich als erledigt oder erfolgreich getestet markiert.

Die vorgeschlagenen Details zu Granularität und Speicherung unten bleiben ein Entwurf; die oben beschriebenen Lebenszyklusregeln ersetzen jede Interpretation, die bestehende Tasks bei Spec-Änderungen wieder öffnet.

## Ablauf im Original

Die TypeScript-Pipeline verarbeitet Spec-IR, semantischen Graph, Readiness-Lücken, analysierte Bindings und Target-Konfiguration. Ein sprachabhängiger `ImplementationPlanner` erzeugt Task-Definitionen. Der gemeinsame Plan-Aufbau normalisiert Pfade, ergänzt Aufruf-Abhängigkeiten, serialisiert überlappende Schreibbereiche, validiert den DAG, berechnet Task-Vertragshashes und bildet Wellen.

Anschließend entstehen Task-Kontexte, Agent-Envelopes, Kontext-Indizes und Prompts. Der Service berechnet Zustände aus Implementierungsprüfungen, Run-Evidence und der letzten grünen Baseline. Der Workflow übersetzt offene Tasks in einen zweiten DAG mit Agent-Ausführung, Ressourcen, Gates, Wiederholungen und Integration pro Welle.

Die normale `task`-Abfrage liest inzwischen ein gespeichertes Inventar. Erst `--check-consistency` löst Erzeugung und aktualisierte Prüfungen aus. Diese bestehende Trennung ist sinnvoll und sollte erhalten bleiben.

Quellen:

- [Task-Typen und Planner-Schnittstelle](https://github.com/Haardt/runatron/blob/main/packages/plugin-dctx0/src/dctx0/implementation/types.ts)
- [TypeScript-Task-Erzeugung](https://github.com/Haardt/runatron/blob/main/packages/plugin-dctx0/src/dctx0/target/typescript/implementation-planning/implementation-planner.ts)
- [Gemeinsamer DAG-Aufbau](https://github.com/Haardt/runatron/blob/main/packages/plugin-dctx0/src/dctx0/implementation/plan.ts)
- [Generierung von Task-Artefakten](https://github.com/Haardt/runatron/blob/main/packages/plugin-dctx0/src/dctx0/implementation/generator.ts)
- [Service und Prüfzustände](https://github.com/Haardt/runatron/blob/main/packages/plugin-dctx0/src/dctx0/implementation/service.ts)
- [Gespeichertes Inventar](https://github.com/Haardt/runatron/blob/main/packages/plugin-dctx0/src/dctx0/implementation/inventory/stored-task-inventory.ts)

## Konkrete Befunde

### Sprachabhängige Planung und uneinheitliche Granularität

TypeScript erzeugt unter anderem einzelne Operation-Tasks, Port-Adapter, Conformance-Fixtures, Event-Publisher, Typverträge und Boundary-Tasks. Go und Java planen überwiegend ganze Surfaces. Damit wechseln Granularität und Task-Identitäten mit dem Target. Die Richtlinie im Original ordnet die Task-Topologie dem Core zu; im gelesenen Code erzeugen die sprachabhängigen Planner dennoch wesentliche Task-Mengen und Abhängigkeiten.

Vorschlag: Ein gemeinsamer Planer bestimmt fachliche Arbeitseinheiten und Abhängigkeiten ohne Target-Eingaben. Target/Profile liefern später normalisierte technische Informationen wie Pfade, Bindings und verfügbare Prüfungen an die Umsetzung und Verifikation. Dafür später erforderliche zusätzliche technische Arbeit wird ausdrücklich separat geplant.

### Erstellung, Prüfungen und Agent-Aufbereitung sind gekoppelt

`ImplementationPlanner` bündelt `createPlan`, Kontextaufbau, Vertragsfakten, Namensanalyse, Prüfgegenstände und asynchrone Implementierungsprüfungen. `generate()` plant zunächst ohne Kontexte, berechnet Zustände einschließlich Prüfungen und erstellt anschließend die Kontexte. Der Task-Kern braucht diese gesamte Pipeline nicht.

Vorschlag: Planen, Speichern, Lesen, Prüfen und Ausführen als getrennte Anwendungsfälle. Ein fehlender Interpreter oder native Compiler darf das Erstellen eines nachvollziehbaren Arbeitsplans nicht verhindern.

### Mehrere Gründe für Reihenfolge werden zu denselben DAG-Kanten

`plan.ts` ergänzt sowohl `semantic-call` als auch `write-conflict` in `dependsOn`. Ein gemeinsamer Schreibpfad wird dadurch zur dauerhaften Voraussetzung im Plan. Zusätzlich sperrt der Workflow dieselben Schreibbereiche und pauschal die jeweilige Domain. Ein Fallback-Pfad `.` überlappt mit jedem anderen Pfad.

Vorschlag: Drei Beziehungen getrennt halten:

- **Voraussetzung:** Eine Aufgabe benötigt das Ergebnis einer anderen Aufgabe.
- **Semantische Beziehung:** Eine Operation ruft eine andere auf oder verwendet ihren Vertrag.
- **Ressourcenkonflikt:** Zwei Ausführungen dürfen denselben Schreibbereich nicht gleichzeitig verändern.

Ein Aufruf benötigt häufig zuerst den Vertrag des aufgerufenen Elements; er verlangt nicht immer dessen abgeschlossene Implementierung. Gegenseitige Aufrufe im gültigen semantischen Graph dürfen deshalb nicht blind in einen zyklischen Task-DAG übersetzt werden. Gemeinsame Vertragsgruppen können echte zyklische Typabhängigkeiten aufnehmen. Verbleibende harte Zyklen müssen mit ihren konkreten Kanten erklärt werden; keine stillschweigende Auflösung durch Namenssortierung.

Unbekannte Schreibbereiche dürfen später nicht als konfliktfrei gelten. Die konservative Sperre ist sicher, kann aber Parallelität vollständig verhindern und muss als solche angezeigt werden.

### Globale Blockaden und Wellen erschweren lokale Arbeit

Im TypeScript-Planer hängen zahlreiche Task-Kategorien an sämtlichen menschlichen Entscheidungs-Tasks. Eine offene Entscheidung kann dadurch unabhängige Module blockieren. Die Task-Selektion betrachtet zuerst die erste unvollständige Welle; der Workflow integriert ebenfalls pro Welle.

Vorschlag: Entscheidungen nur mit betroffenen Aufgaben verbinden. Die ausführbare Menge aus aktuellen Voraussetzungen und Ressourcenkonflikten ableiten. Wellen können eine Darstellung der Topologie sein; sie sollten nicht automatisch eine globale Ausführungsbarriere werden. Beispiel: Ist B fertig und benötigt C nur B, kann C fachlich bereit sein, obwohl das unabhängige A noch offen ist.

Quellen: [Task-Selektion](https://github.com/Haardt/runatron/blob/main/packages/plugin-dctx0/src/dctx0/implementation/task-list-query.ts), [Workflow-Projektion](https://github.com/Haardt/runatron/blob/main/packages/plugin-dctx0/src/dctx0/surface/workflow.ts), [Workflow-Definition](https://github.com/Haardt/runatron/blob/main/packages/plugin-dctx0/src/dctx0/surface/workflow-definition.ts).

### Lokale Vertragshashes werden durch globale Cache-Bedingungen eingeschränkt

`taskContractHash` ist bereits auf einen Task zugeschnitten; Tests prüfen die Unabhängigkeit von anderen Operationen. Der Check-Cache verlangt aber zusätzlich identische Gesamtwerte für `dctx0Hash` und `planHash`. Eine unabhängige Spec-Änderung kann damit die Wiederverwendung aller Prüfungen verhindern. Einige Prüfgegenstände schließen außerdem den gesamten Projektpfad `.` ein.

Vorschlag: Task-Anforderungen, technische Projektion, relevante Implementierungsquellen und Prüfer-Version getrennt erfassen. Globale Snapshot-/Plan-Hashes bleiben Herkunftsnachweise. Wiederverwendung ist nur innerhalb der tatsächlich geprüften Reichweite zulässig: Ein kompletter Go-/Java-Build bleibt ein globales Prüfergebnis und darf ohne lokale Prüfgrundlage nicht in unabhängige Task-Beweise umgedeutet werden.

Der derzeitige Task-Vertrag enthält auch Beschreibung, Verifikationsanweisung, Risiko und Schreibpfade. Reine Anzeigetextänderungen sollten künftig keinen fachlichen Vertrag ändern; technische Pfadänderungen können dagegen ihre eigene Evidenz ungültig machen.

Quellen: [Vertragsprojektion](https://github.com/Haardt/runatron/blob/main/packages/plugin-dctx0/src/dctx0/implementation/task-contract.ts), [Cache-Gültigkeit](https://github.com/Haardt/runatron/blob/main/packages/plugin-dctx0/src/dctx0/implementation/check-cache/check-index.ts), [DAG- und Hash-Tests](https://github.com/Haardt/runatron/blob/main/packages/plugin-dctx0/src/dctx0/implementation/__tests__/dag-ownership.test.ts).

### Identität hängt teilweise an Namen und Dateien

TypeScript-Task-IDs bestehen aus einem begrenzten Slug und einem Hash von Art und Ziel. Typverträge werden nach Ausgabedatei gruppiert; ihre IDs verwenden den Dateipfad. Eine Dateiverschiebung kann deshalb einen neuen Task erzeugen. Menschlich lange fachliche Namen werden in der ID zudem abgeschnitten.

Vorschlag: Fachlichen Schlüssel, interne Task-ID und lesbaren Titel trennen. Der reine Planer erzeugt Schlüssel aus Aufgabe/Subject/Anforderungen; der Store ordnet jeder neuen Änderungsaufgabe eine neue persistente ID zu. Explizite Spec-Renames begründen den Zusammenhang der Aufgaben über die vorhandene Rename-Historie, ohne abgeschlossene Vorgänger umzuschreiben. Gewöhnliche Entfernen/Hinzufügen-Änderungen begründen keine Kontinuität. Gruppenteilungen oder Zusammenführungen werden ausdrücklich protokolliert. Kein neues universelles `stableId` in der DSL.

### Zustandsdimensionen sind sinnvoll, ihre Darstellung ist schwer verständlich

Das Original führt gleichzeitig Prüfstatus, Änderung gegenüber der letzten grünen Baseline, Readiness und Run-Evidence. Diese Dimensionen beantworten unterschiedliche Fragen. Ein Task kann zum Beispiel `READY` und zugleich `BLOCKED` beim Prüfstatus sein, wenn eine Implementierungsprüfung noch nicht verfügbar ist. Außerdem gibt es zusätzliche Promotion-Zustände für Wellen.

Vorschlag: Arbeitsfortschritt separat speichern; Readiness mit konkreten Blockierungsgründen ableiten. Eine Revision kann neu/geändert/entfernt sein, während ältere Arbeit abgeschlossen bleibt. Prüfungen erhalten ihre eigene Evidenz und Gültigkeit. Fehlende Prüfungen heißen unverified/unknown; manuelle Fertigmeldung ist kein automatisch bewiesenes `PASS`. Die CLI zeigt standardmäßig Titel, Subject, Arbeitsstand, Prüfstand, Revision und den wichtigsten Blockierungsgrund.

### Viele Dateien duplizieren Darstellungen

Bei vollständiger Kontext-Erzeugung entstehen bis zu fünf Dateien pro Task: `task.json`, `context.json`, `agent-envelope.json`, `context-index.json`, `prompt.md`. Die untersuchten historischen Artefakte enthalten:

| Gespeicherter Plan | Tasks | Task-Dateien | Wellen |
| --- | ---: | ---: | ---: |
| Kopiertes Cinema-Beispiel | 48 | 240 | 5 |
| `../runatron-next` | 513 | 2.565 | 5 |

Cinema enthält 13 Operation-Tasks, 12 Boundary-Operation-Tasks und weitere technische Aufgaben. Die Anzahl ist allein kein Fehler; jede Aufgabe braucht aber einen klaren Grund, eigenen Umfang und einen passenden Abschlussnachweis.

Dies sind vorhandene Referenzartefakte, keine frischen Planungen und kein Leistungsnachweis. Vorschlag: Definitionen, Kanten, Revisionen und Arbeitshistorie in SQLite speichern; Task-Kontext aus dem gepinnten IR bei Bedarf erzeugen. Markdown/JSON dienen als ausdrückliche Exporte, nicht als zusätzliche führende Zustandskopien.

## Empfohlener erster Rust-Schritt

Ein neues kleines `dctx0-tasks`-Crate bündelt `model`, `planner`, `dependencies`, `context` und `reconcile`. Es nutzt vorhandenes IR, Fingerprints und `petgraph`. Der Store erweitert SQLite mit einer versionierten Migration; die Engine orchestriert Planung und Veröffentlichung. Die CLI rendert die Ergebnisse. Neue Target-, Agent- oder Workflow-Abhängigkeiten sind dafür nicht erforderlich.

Als erste Granularität vorschlagen:

- Vertragsaufgaben nach fachlichem Besitzer und zusammengehörigen Deklarationen, statt nach jeder einzelnen Datei oder jedem Typ.
- Eine Implementierungsaufgabe je Surface-Operation. Port-Operationen sind Bestandteil des Port-Vertrags; ihre Implementierung gehört zur Adapteraufgabe.
- Eine Adapteraufgabe je Port.
- Integration je Modul und bei Bedarf der Anwendung.

Jeder Task nennt seine verantworteten Subjects, relevante Verträge/Regeln, Akzeptanzkriterien und die Begründung seiner Voraussetzungen. Exakte Quellpfade und native Prüfung werden später durch technische Projektionen ergänzt. Interpretation der Regeln ist dafür nicht notwendig; ihre Ausführung bleibt ausdrücklich ungeprüft. Aufgabengruppen müssen eindeutig sein, um doppelte Zuständigkeit zu verhindern.

Der erste Lieferumfang sollte sein:

1. Reproduzierbare Kandidatenplanung aus einem expliziten gültigen Snapshot einschließlich Planer-/Regelversion.
2. Separater Task-DAG mit erklärbaren Voraussetzungen und sauberer Behandlung semantischer Zyklen.
3. Atomare Speicherung versionierter Pläne; Reconciliation erzeugt neue Änderungs-Tasks, verknüpft sie mit Vorgängern, erhält unveränderte Tasks und bewahrt entfallene Arbeit als Historie.
4. Read-only-Liste und Task-Details mit kompaktem, aufgabenspezifischem Kontext; keine automatische Codegenerierung oder Verifikation.
5. Erklärung von Änderungen und Blockaden, plus Tests/Coverage und Speed/RAM für beide vollständigen Beispiele.

Vorgeschlagene, noch nicht vorhandene CLI:

```sh
dctx0 tasks plan <root> --snapshot latest --dry-run
dctx0 tasks plan <root> --snapshot latest
dctx0 tasks list <root> --json
dctx0 tasks show <root> <task-id>
```

`latest` bezeichnet den letzten veröffentlichten gültigen Check, nicht automatisch die gerade editierten Dateien. Die Anzeige nennt Basis-Snapshot und Planrevision. Ein neues `check` und erneute Planung sind ausdrückliche Schritte. Eine Liste gespeicherter Tasks darf keine Aktualität gegenüber ungeprüften Arbeitsdateien behaupten. Auswahl ausführbarer Tasks und manuelle Bearbeitungszustände können anschließend separat ergänzt werden.

Weitere Abfragen, Arbeitsaktionen und Review-Aufgaben sind im [Task-API-Entwurf](tasks-api.md) beschrieben. Diese Funktionen sind noch nicht implementiert.

## Akzeptanzfälle

- Beide vollständigen Beispiel-IRs liefern deterministische Pläne ohne Interpreter oder Target-Toolchain.
- Wiederholte Planung ist idempotent; Sortierung, Dateireihenfolge und Formatierung ändern keine fachlichen Task-Anforderungen.
- Eine lokale Vertragsänderung erzeugt neue Tasks nur für tatsächlich betroffene Anforderungen und nennt die geänderten Fakten. Alte Done/PASS-Aufgaben werden nicht automatisch wieder geöffnet.
- Explizite Renames erhalten nachvollziehbare Historienbeziehungen; Löschen/Wiederverwenden eines Namens begründet keine Kontinuität. Historische Aufgaben bleiben unverändert.
- Historienverweise werden nicht als neue Arbeitsabhängigkeiten interpretiert. Ein Rückwechsel zu einer früheren Spec erzeugt neue Arbeit statt ein historisches PASS zu übernehmen.
- Fehlende Voraussetzungen, doppelte Zuständigkeit und harte Zyklen liefern konkrete Diagnosen.
- Gegenseitige semantische Aufrufe erzwingen keinen ungültigen Arbeits-DAG.
- Aufgaben ohne verfügbare Prüfmethode bleiben unverified; keine automatische Fertigmeldung durch vorhandene Dateien oder erfolgreiches Spec-Parsing.
- Lesen schreibt keine Daten und startet keine Prüfungen. Fehlgeschlagene Planung erhält den vorherigen gültigen Plan; Verlauf abgeschlossener Arbeit bleibt nachvollziehbar.
- Jede Implementierungsvalidierung umfasst Coverage sowie Release-Speed und Peak-RSS. Neue Task-Workloads werden bis zur ausdrücklichen Baseline-Annahme als unassessed ausgewiesen.

Für diese Analyse wurden Quelltexte, vorhandene Tests als Spezifikation und gespeicherte Artefakte gelesen. Es wurden keine Tests ausgeführt oder Laufzeiten behauptet; Rust-Produktionscode und Beispiel-Specs bleiben unverändert.
