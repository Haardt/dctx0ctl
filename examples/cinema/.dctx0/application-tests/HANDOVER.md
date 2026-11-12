# TypeScript/Bun-Anwendungstests: aktueller Übergabestand

Bibliothek 0.2.0 / Protokoll 2 unterstützt Konstruktoren, Raw-Decoder,
Operationen, StateChange, Assertions, Szenarien, Lifecycle und zentrale
Identitätsableitung. CinemaSuite führt alle sieben zentral registrierten Formen
in einer Sitzung aus. Die neue native Fassade wird aus `Bundle.model.operations`
generiert: 26 direkte typgeprüfte Produktionsanschlüsse und Portdecorators aus
neun expliziten Owner-Anbindungen. Kein handgeschriebener Operation-Dispatcher
und kein alter Worker werden verwendet.

Die reale Fixture verwendet CinemaSession mit Awilix/Hono und eigener
Datei-SQLite-Datenbank pro Fall. Tatsächliche injizierte Calls, Publisher-Events,
SQL-Ressourcen mit explizitem asynchronem Besitzer und vollständige Entitydaten
werden beobachtet. Alle fünf Entityprojektionen prüfen physische SQLite-Keys.
Verbundene Schritte teilen ihre Fixture; Cleanup schließt echte Ressourcen.
Zusätzliche native Assertions prüfen DB-Schließen/Wiederöffnen und echte
HTTP-Aufrufe einschließlich Calls/Events und erneutem Öffnen.

## Vollständige native Abnahme

`target/typescript-application-tests/v2-all-complete/result.json` belegt:

- 5766/5766 Pflichtfälle PASS, verbleibender Adapterrest 0.
- Vollständige IDs, seed 42 / generated 100, Inputs, Argumente und fachliche
  Urteile sind mit der eingefrorenen TypeScript-Vorherreferenz identisch.
- Unabhängiger Bun-Katalog und JUnit enthalten genau CinemaSuite, weder
  übersprungen noch fokussiert; NativeRunner und Rust-Dienst enden erfolgreich.
- Native Laufzeit 6126.881 ms / 92069888 Bytes Prozessbaum-RSS; Dienst
  6978.919 ms / 151756800 Bytes. Zeiten überlappen, nicht addieren.
- Frozen/offline Dependencies, tatsächliche Fassadengenerierung und striktes
  Typechecking sind separate gemessene Phasen. Native TSC:
  1218.187 ms / 309379072 Bytes. Katalog: 212.508 ms / 30134272 Bytes.

Die Messungen sind Entwicklungsmessungen und gegenüber der akzeptierten
Benchmarkbaseline unbewertet. Sie wurde nicht geändert. Frühere Pilot- und
fehlgeschlagene Zwischenreceipts bleiben erhalten; sie ersetzen diesen
vollständigen Lauf nicht. Vollständige Workspace-QA/Release-Abnahme und der neue
managed Capture-/Build-Runner werden übergeordnet separat geprüft. Kein Task
wird durch die obigen diagnostischen Receipts automatisch übernommen.

## Reproduzierbarer nativer Pfad

Aus dem Repositoryroot mit einer aktuellen CLI:

```
python3 examples/cinema/scripts/application-acceptance.py --cli target/debug/dctx0
```

Default ist der volle Umfang (`--form all`); explizite begrenzte Formen bleiben
möglich. Der Runner verwendet isolierte Kopien, tatsächliche frozen/offline
Installation, generierte und typgeprüfte Fassade, Katalog, separaten Dienst und
Bun-JUnit. `native-run.json` beschreibt dieselben Phasen für den engine-managed
Capturepfad; Generator und Bibliothek bleiben innerhalb der erfassten Anwendung.
Die lokale Bibliothekskopie wird getrennt von Handtests publiziert und per
Datei-SHA256 gepinnt. Produktionsimporte, Spezifikationen und alte Adapter
bleiben erhalten. Die hier beschriebenen diagnostischen Receipts verleihen keine
Übernahmeautorität; das aktuelle managed `run`-Verhalten ist unten ergänzt.

Der eigenständige Katalog sammelt tatsächliche JS-Registrierungen durch eine
gemockte bun:test-API ohne Testcallback-Ausführung. Das ist kein offizieller
Bun-Listenbefehl und keine Einzelcase-Enumeration. Verschachtelte/parametrisierte
Suites, Hooks, concurrent und asynchron verzögerte Registrierung werden nicht
als vollständig unterstützt behauptet. Die tatsächlich verwendete CinemaSuite
ist eine einfache Root-Registrierung; deren JUnit bleibt unabhängig bindend.

## Input-Witness und Darstellungsgrenzen

Konstruktoren und Operationargumente melden tatsächliche native Input-Witnesses,
keine unverändert zurückgemeldeten erwarteten Inputs. Die zwei realen
Codec-Swap-Gegenproben erkennen invalid1887→invalid1800 sowie votes→[] als
INPUT_CHANGED, obwohl ein anderer Input sonst ebenfalls Ablehnung oder ein
zulässiges Resultat erhalten hätte. Witnessfehler verhindern Konstruktoraufrufe.

46 ursprüngliche Rejectionfälle gegen Nicht-Option-Ziele (z.B. Film, FilmId→UUID,
ReleaseYear→Int) verwenden Some(Unit)→undefined und None→undefined. Der Witness
rekonstruiert Unit aus dem tatsächlichen undefined und erhält lediglich die
angeforderte falsche Formannotation. Keine unterscheidbare native Option-Payload
wird behauptet; IDs stehen in `declared_native_representation_limits`. Bei echten
Option-Zielen bedeutet present→undefined BindingError/UNKNOWN: tagged Options
sind erforderlich. Anforderungen/Produktion wurden hierfür nicht verändert.

## Gegenproben und Sprachmessung

Vollständige isolierte Mutationsreceipts liegen unter
`target/typescript-application-tests/v2-all-<probe>/`: fehlender/zusätzlicher Write,
beide Codec-Swaps, falscher Tie-Break, abgefangener Vertragsfehler, falscher Scope,
unvollständige Statebeobachtung, physisch falscher Key, Cleanupfehler sowie native
Assertion und `throw undefined` nach zentralem PASS. Sie verändern ausschließlich
isolierte Quellen und bewahren vollständige 5766-Fall-Inventare. Ein bloßes
zentrales PASS genügt bei nachfolgendem nativen FAIL nicht.

50 Helfertests bestehen (435.769 ms / 79822848 Bytes Prozessbaum-RSS). Rohes
LCOV der sieben in-process Module: 328/335 ausführbare Zeilen (97.91 %) und
48/50 Funktionen (96.00 %). Buns Anzeige „All files“ ist ein ungewichteter
Dateidurchschnitt (98.80 % / 98.64 %) und kein Ersatz für diese Summen. Die
Sprachmetriken sind keine Rust-Grenzwerte. In-process Coverage wird mit rohem
LCOV unter `target/typescript-application-tests/v2-coverage/lcov.info` gespeichert.
`catalog.ts` wird in zehn echten Subprozessgegenproben geprüft; Bun aggregiert
seine Child-Prozess-Zeilen nicht. Diese Quelle ist ausdrücklich ungemessen,
nicht aus dem Messinventar verschwiegen. Fassaden-/Fixture-/HTTP-Nachweise sind
separate native Abnahme, keine Behauptung vollständiger Rust- oder TS-Coverage.


Die vollständige Matrix `target/typescript-application-tests/v2-full-acceptance.json`
fasst 16/16 erkannte echte Mutationen zusammen. Zusätzlich zu den oben genannten
Proben: falscher deklarierter Fachfehler ergibt FAIL/Undeclared failure;
technische Ausnahme ergibt UNKNOWN/APPLICATION_EXCEPTION; fehlender Callbeobachter
(None) ergibt UNKNOWN/RESOURCE_OPERATION_UNOBSERVED, ein ausdrücklich leerer Trace
bei tatsächlichen Mockrequests ergibt FAIL/MOCK_OBSERVATION_MISMATCH. Jeder Lauf
behält alle 5766 Fälle und endet im nativen Runner mit Fehler. Die ersten zwei
zusätzlichen Probeversuche hatten eine zu strenge FAIL-Erwartung für echte
UNKNOWN-Ausgänge; sie bleiben als fehlgeschlagene Gegenproben erhalten, die
korrigierten Receipts tragen `-expected-unknown`.

Die Bibliothek ist jetzt im TypeScript-Target eingebettet und über
`application_test_library()` ohne Repository-Publisher auslieferbar. Das Paket
bleibt private und wird nicht zu npm publiziert. Der CLI-Publisher schützt
geänderte bestehende Eigentümerdateien anhand ihrer Pins. Der bisherige lokale
Fixture-Publisher bleibt für reproduzierbare Referenzkopien verfügbar.


## Bewachte Task-Übernahme (2026-11-09)

Das aktuelle managed `run` speichert frische native Evidenz und versucht
transaktional bewachte Task-Übernahme; `--no-adopt` bleibt diagnostisch.
`complete` ergänzt aktuelle quellengebundene Autoreninspektionen mit derselben
Abschlusslogik wie der alte Adapterpfad. Ein frischer Go-Komplettlauf bestätigt
5766 Fälle und 105 Done/PASS-Tasks, darunter vier ausdrücklich nativ gestützte
Review-Attestierungen. Bloße Reviewfreigabe oder manueller Abschluss setzt kein
PASS; bereits Done/Unverified abgeschlossene Reviews werden nicht aufgewertet.
Die vollständige Workspace-/Coverage-/Release-Abnahme steht weiterhin aus.
Die oben referenzierten historischen Diagnoseberichte erhalten nachträglich
keine Autorität. Siehe README.md und die zentrale Workflowdokumentation.
