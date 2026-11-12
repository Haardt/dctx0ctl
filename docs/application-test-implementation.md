# Anwendungstest-Implementierung

Arbeitsbeginn: 2026-11-08. Ausgangs-HEAD:
`789454f22f8fb8bda3d203759ebfb28d3d10be92` (vor Umsetzung erneut prüfen).
Architekturentscheidungen und Referenzlisten bleiben in
[Konzept](application-test-concept.md) und [Migration](application-test-migration.md).

## Auftrag und unveränderliche Grenzen

P0–P7 bilden den Gesamtauftrag. Go kommt vor Bun und Rust; Java behält seine
bisherigen Fähigkeiten. Bestehende Adapterkonfigurationen bleiben unterstützt.
`ProgramPlan` und `case_manifest` bestimmen Inputs und Pflichtumfang für beide
Backends. Eine neue Backendwahl verändert weder Fall-IDs noch Seeds/Budgets.
Neue diagnostische Pläne oder importierte Urteile erhalten keine Übernahmeautorität.
Automatisches Skipping und allgemeine historische PASS-Wiederverwendung bleiben aus.

## Aktuelle Priorität des Nutzers (2026-11-08)

Der Nutzer hat die tatsächliche Umstellung des Cinema-Projekts als ersten Test
priorisiert und verlangt, die aktuelle Validierung vorab festzuhalten und danach
mit dem Umbau zu vergleichen. Entsprechend der akzeptierten Reihenfolge beginnt
die Ausführung mit `examples/cinema-go`; die gemeinsame Cinema-Spezifikation
bleibt unverändert. Der P0-Planexport allein erfüllt diese Umstellung nicht.
Die übrigen P0–P7-Ziele bleiben erhalten; die native Cinema-Strecke hat Vorrang.

Vor weiteren Änderungen an Rust-, Go- oder Skriptquellen wird die laufende
korrigierte vollständige Validierung abgeschlossen und als separate
Migrationsreferenz gesichert. Dies ist keine Promotion von
`benchmarks/baseline.json`. Die Referenz benötigt den vollständigen Messdatensatz,
Coverage-Rohmetriken und die tatsächlich erfassten Produktionsdateien, native
Go-/Rust-Abnahmeberichte, CLI-/Quellhashes sowie den reproduzierbaren Dirty-Stand
(Basisrevision, Patch und neue Quelldateien). Nach dem Umbau werden dieselben
Pflichtfälle, Seeds, Budgets, fachlichen Urteile, Mutationen und Replay sowie
Build-/Laufzeit, RSS, Artefaktvolumen und handgeschriebene Anschlussstellen
verglichen. Zusätzliche Konformitätsproben werden separat ausgewiesen.

Die ersten direkten Go-Testanschlüsse sind `domain.Validate`,
`domain.CandidateOrderWinner` und `persistence.Store.Invoke` mit tatsächlichem
SQLite-Store. `Store.Invoke` ist die bestehende Produktionsschnittstelle;
sie rechtfertigt keinen neuen projektspezifischen Worker-Dispatcher im Test.
Die Testbibliothek übernimmt Registrierung, Falliteration und Finalisierung;
der zentrale Rust-Bewerter behält Urteil, Setup-Datenfluss und Fortsetzungen.
Die bisherigen Adapterdateien bleiben für Vergleich und noch nicht umgestellte
Bereiche erhalten. Die vollständige Umstellung darf nicht als bestanden gelten,
solange Pflichtbereiche oder native Testformen unimplementiert bleiben.

## Übernommener Arbeitsstand

Vorhandene Änderungen: README, Stringgenerierung, Projektionsprüfung von
Setup-Platzhaltern, Operation-Argument- und Stringtests; neue
`projection/setup_arguments.rs` sowie beide Konzeptdokumente. Diese Änderungen
werden erhalten. Setup-Bindungen müssen auch im Export und bei Fortsetzungen
vollständig erhalten bleiben; tatsächliche Inputs werden nach Auflösung geprüft.

## Etappen und Abnahme

| Etappe | Ergebnis | Stand |
| --- | --- | --- |
| P0 | Versionierter gemeinsamer Export, eindeutige Gruppen, Konfiguration und Vergleichsbasis | Plan/Zuordnung und eingefrorene vollständige Vorherreferenz vorhanden |
| P1 | Go-Testbibliothek und zentrale reine Bewertungssitzung mit getrenntem Protokoll | Nativer Decoder-/Operationspilot vorhanden; verwaltete Gesamtabnahme offen |
| P2 | Zustand, Calls, Ressourcen, lokale Fortsetzungen, persistenter Lifecycle | Store-Pilot vorhanden; Lifecycle und weiterer Umfang offen |
| P3 | Collector, Quellbindung, Gruppenevidenz, Store-Guards, Replay/Mischbetrieb | Offen |
| P4 | Bun/Rust und vollständige Cinema-Abnahme | Bun-Pilot parallel auf Nutzerauftrag; vollständige Abnahme offen |
| P5 | Vollständiges Consumer-Paket `runner/internal/preparedplans` | Offen |
| P6 | Transitive Testanalyse und eingefrorene Auswahlvorhersagen, kein Skipping | Offen |
| P7 | Belegter freiwilliger Rückbau, Dokumentation und Distribution | Offen |

## Geprüfte Einstiegspunkte

- `manifest::case_manifest`: einzige zentrale Fallinventarisierung.
- `projection::ProgramPlan`, `OperationPlan::generated_setup`,
  `projection/setup_arguments`: vollständige Plan- und Setup-Daten erhalten.
- `verification_setup::VerificationConfiguration`: heutiges Format Version 1
  verlangt einen Adapter; bestehende Ausführungswege dürfen kein neues Format
  stillschweigend als Adapterkonfiguration behandeln.
- `verification_run::{execution,adoption}`: bestehende Manifest-/Übernahmeprüfung
  bleibt bis zur gesonderten Gruppenabnahme maßgeblich.

## Evidenz und technische Grenzen

Historischer P0-Zwischenstand: damals noch keine native Ausführung oder Gesamtabnahme.
Der aktuelle v2-Stand und seine verbleibenden Release-Gates stehen am Dokumentende.
Performance und mindestens 97 % Rust-Linien-/Funktionscoverage sind für die
integrierte Abnahme erforderlich; die akzeptierte Baseline wird nicht verändert.
Native Testquellen können Beobachtungen fälschen; der Auftrag verspricht keine
kryptografische Ehrlichkeitsprüfung beliebigen Anwendungscodes.

## Implementierte P0-Schnittstelle

`dctx0_verification::application_plan::application_test_plan` übernimmt den
unveränderten `ProgramPlan` und dessen zentrales Manifest. Exakte Pflicht-IDs
selektieren ganze Fälle. Jede Pflicht besitzt genau eine Gruppe; alle Pfade
eines Lifecycle gehören derselben Gruppe. Szenarien bleiben einzelne atomare
Manifestfälle. Identitätsfälle bleiben zentrale, aus tatsächlichen Quellfällen
abzuleitende Pflichten; die Zuordnung allein liefert keine Identitätsevidenz.

Die Testformen Decoder, Operation, StateChange, Scenario, Lifecycle, Assertion
und EntityIdentity werden ausschließlich für vorhandene Manifestfälle abgeleitet.
Eine Registrierung mit unpassender Testform wird zurückgewiesen. Native
Katalogenumeration und Ausführungsabschluss sind noch nicht implementiert.
Unterschiedliche Ressourcennamensräume sind Konfiguration, kein Nachweis echter
Isolation. Target-Namen in diesem diagnostischen Plan schalten keine Fähigkeit frei.

Der CLI-Zugang ist eine explizite Erweiterung von `verify plan`:

```sh
dctx0 verify plan SPEC --subject QUALIFIED_SUBJECT \
  --application-tests groups.json --test-catalog catalog.json --json
```

Beide Dateien sind jeweils auf 512 KiB begrenzt. `groups.json` hat diese Form
(die Fallmenge ist für den konkret veröffentlichten Plan vollständig anzugeben):

```json
{
  "schema_version": 2,
  "groups": [{
    "id": "year", "backend": "application-tests", "target": "go",
    "selection": ["TestReleaseYear"],
    "area": {"cases": [], "requirements": ["W.ReleaseYear"]},
    "resources": {
      "namespace": "year", "timeout_seconds": 60,
      "max_message_bytes": 1048576
    }
  }]
}
```

Der Katalog ist ein Array von Registrierungen mit `group`, `test`, `form` und
`cases` (exakte Fall-IDs aus dem zentralen Manifest). Beispielsweise lautet die
Testform für einen Decoder `"decoder"`. Jede ausgewählte native Testidentität
muss registriert sein; jeder ihr zugeordnete Fall wird genau einmal registriert.
Adaptergruppen haben `backend: "adapter"` und eine einzelne Adapteridentität in
`selection`; sie besitzen keine Anwendungstestregistrierungen.

| Format | Leser/Schreiber | Verhalten |
| --- | --- | --- |
| Bestehende `verification.json`, Schema 1 | Bestehende Setup-/Ausführungswege unverändert | Adapter; keine Migration |
| Gruppen, Schema 2 | Explizites `--application-tests` und reine Bibliotheks-API | Nur diagnostische Planung; unbekannte Versionen abgelehnt |
| Anwendungstestexport, Schema 1 | `ApplicationTestPlan` und `validate()` | Originalplan, Manifest, Konfiguration, Zuordnung, Testformen, Registrierungen |
| SDK 2 / Worker-Protokoll 1 | Bestehender Adapterpfad unverändert | Keine Protokolländerung |
| Run-/Store-Formate | Unverändert | Keine neue persistente Evidenz oder Übernahmeautorität |

`ApplicationTestPlan::validate()` berechnet Manifest, Testformen und Zuordnung
erneut. Ein editierter Export kann diese Ableitungen nicht ersetzen. Das ist
keine Quell-/Buildbeglaubigung: diese entsteht erst im Collector-/Store-Inkrement.
Die neue Konfiguration wird nicht automatisch aus `verification.json` gelesen;
bewachte Ausführung und ein gemeinsamer Konfigurationsleser gehören noch zu P3.

## Ausführbare Gegenproben

`crates/dctx0-verification/tests/application_tests.rs` prüft die vollständige
Cinema-Projektion mit direkten Bereichen für ReleaseYear, candidateOrderWinner
und FilmCatalogStore.save sowie einem unabhängigen Adapterrest. Originalplan,
Manifest, Budgets und offene Pflichten sind in beiden Konfigurationen identisch.
Dies ist Planvergleich, kein nativer Alt-/Neu-Ausführungsvergleich.

Weitere Gegenproben decken Lücken, Überschneidungen, unbekannte IDs,
Registrierungsduplikate, unpassende Testformen, geteilte Lifecycle-Pfade,
Versionsfehler und editierte Exportableitungen ab. Der Setup-Roundtrip wird in
`tests/operation_arguments.rs` einschließlich späterer Argumentauflösung geprüft.
Der CLI-Test prüft JSON/Markdown, Größenlimits und unveränderte History.

Der gemeinsame Messrunner enthält die Release-Phase `application-test-plan`.
Sie misst Export/Zuordnung nach vorbereiteter Projektion; RSS umfasst den Prozess
einschließlich Vorbereitung. Native Build-/Laufkosten und handgeschriebener
Anschlussaufwand sind erst nach P1 vergleichbar und bleiben offen.

## Vorinventar des festgelegten Consumers (nur lesend)

Am 2026-11-08 wurden die Root- und Runner-`AGENTS.md` sowie die öffentlichen
Grenzen von `runatron-business/packages/runner/internal/preparedplans` gelesen.
Der dabei gelesene Consumer-HEAD lautet
`1067ab10dae944865de99103ddf12e5f8e13ceee`; dies beglaubigt keinen unveränderten
Consumer-Arbeitsbaum. Im Consumer wurden keine Dateien geändert und keine
Prüfungen ausgeführt.

| Grenze | Vorhandene Implementierung/Anschluss |
| --- | --- |
| Öffnen und Bindung | `Open`, `OpenObserved`, `BelongsTo`; unveränderliche Registrierung |
| Datensatzspeicherung und Sicht | `Save`, `Snapshot`, `Get`; gesperrter Writer nach unbestätigtem Commit |
| Publikation | `Publish`, `Stage`, `Confirm`, `Fence`, `Receipt`, `Effective` |
| Persistenzport | `SnapshotStorage.Read/Commit`; tatsächliche Implementierung über `gitworkspace.NewSizedSnapshotFile` |
| Vorhandene Spec-Pflicht | `RunatronBusiness.RunnerRuntime.PreparedPlans.save` und Entity `PreparedPlanRecord`, `spec/runner-runtime/transaction-ports.dctx0` |
| Bestehender Adapter | `.dctx0/testing/go/prepared-plans.go`: `ensurePreparedStore`, `invokePreparedSave`, `observePrepared` |
| Echte Fixture | Eigener `gitworkspace.Root`, Snapshot-Limit 8 MiB, `OpenObserved` für Ressourcen und `Store.Snapshot` für Zustand |

Die vorhandene Save-Pflicht fordert tatsächliches Lesen/Schreiben, Einfügen,
`result == request` bei Erfolg und unveränderten Zustand bei Fachfehlern. Der
Adapter übersetzt `StateConflict` als deklarierten Fehler; Codecfehler bleiben
Adapterfehler und Storagefehler Umgebungsfehler. Diese Unterscheidung ist bei
der Migration zu erhalten. Die übrigen öffentlichen Go-Methoden sind nicht
automatisch eigenständige Spec-Operationen: ihre vorhandenen nativen Prüfungen
bleiben Pflichtumfang des vollständigen Pakets.

15 vorhandene native Tests verteilen sich auf `store_test.go`,
`publication_test.go`, `observed_boundaries_test.go` und
`reopen_boundaries_test.go`. Sie enthalten tatsächliches Wiederöffnen, verlorene
Commitbestätigungen, Duplikate/Parallelität, unbestätigte Publikation,
Cancellation, beobachteten Write-Fehler und manipulierte Snapshotdaten.
Die bestehenden Helpers `openDisk`, `publication`, `faultSnapshot` und
`cancellationSnapshot` sind Wiederverwendungskandidaten; sie wurden hier nicht
als vollständige neue Library-Anbindung abgenommen. Übergreifende Host-/Event-
und Configuration-Szenarien verwenden ebenfalls `preparedplans.Store` und
müssen bei der späteren Gruppenzuordnung als verbundene Fälle erhalten bleiben.

## Validierung dieses Arbeitsstands

Gestartet wurde `python3 scripts/validate.py --coverage --samples 1 --iterations 1`.
Das vollständige Log liegt in `target/application-test-validation.log`.
Ein Sample/eine Iteration sind Entwicklungsbedingungen; ein Vergleich gegen die
akzeptierte Baseline ist damit unbewertet. Die Baseline wird nicht promoviert.

Bereits erfolgreich: Workspace-Check, Build, rustfmt, Clippy mit `-D warnings`,
703 Workspace-Rust-Tests (Benchmarkkonformität folgt gemäß Runner am Ende),
CLI ohne Standardfeatures sowie Rust-only und Go-only, 46 Python-Tests,
46 Bun-Tests, native Go-Tests/vet und Go-Race. Die Workspace-Prüfung enthält
den vollständigen Cinema-Test mit Replay und HTTP-Mutation.

Die vollständigen bestehenden Go-/Rust-Cinema-Abnahmen bestehen ebenfalls:
jeweils 5.766 Fälle, 105 Tasks, fünf reale Mutationen und Replay. Dies ist
Kompatibilitätsevidenz des bisherigen Backends, keine Ausführung des neuen
Anwendungstestmodells.

Der erste Lauf wurde nach dem Coverage-Export vor den Release-Messungen beendet:
Der Standardfilter von cargo-llvm-cov hatte die neue Produktionsdatei
`application_tests.rs` wegen ihres Namens ausgelassen. Die nominellen 97,07 %
sind damit keine akzeptierte Coverage dieses Inkrements. Der reine Planer liegt
nun in `application_plan.rs`; die Gegenproben bleiben unter `tests/`.
`application_plan` ist auch der Name des neuen Rust-Moduls. Der separate
Abbruchbeleg steht in `application-test-validation-interrupted.json`, die
ursprünglichen Rohdaten unter `target/application-test-before-rename/`.

Die Wiederholung verwendet das im Repository dokumentierte serielle QA-Profil:
dev/test opt-level 1, keine Debugsymbole oder inkrementelle Kompilierung,
Debug-Assertions und Overflow-Prüfungen bleiben explizit aktiviert. Die
uninstrumentierten Release-Messungen bleiben separat.

Die korrigierte Vorher-Validierung ist abgeschlossen: alle 35 Schritte bestehen,
alle 89 Release-Workloads sind vollständig gemessen. Der Datensatz
`20261108T172542.565611Z-2544310.json` ist als Migrationsreferenz einschließlich
Quellrekonstruktion und eingefrorenem 5.766-Fälle-Plan unter
`benchmarks/application-tests/before-20261108T172542Z/` gesichert.
Coverage: 43.279/44.580 Quelllinien (97,0817 %) und 3.501/3.605 LLVM-Funktionen
(97,1151 %); beide neuen Produktionsdateien sind im LCOV-Inventar enthalten.
Quellhashes blieben während Validierung und Coverage jeweils unverändert.
Die neue Planungsphase misst 50,767851 ms und 39.628.800 Bytes Prozess-VmHWM
einschließlich Vorbereitung. Der gesamte gekoppelte Lauf dauerte 4.467.627,404 ms
mit 1.062.531.072 Bytes gesampeltem Prozessbaum-RSS. Die Messbedingungen sind
gegenüber der akzeptierten Baseline unbewertet; sie wurde nicht promoviert.

Damit ist die aktuelle Vorher-Validierung festgehalten. Die tatsächliche
Cinema-Anwendungstestmigration und ihr Nachher-Vergleich sind noch auszuführen.

## Gesicherte Vorherreferenz und nativer Pilot (2026-11-08)

Die vollständige korrigierte Vorher-Validierung ist abgeschlossen und unverändert
unter `benchmarks/application-tests/before-20261108T172542Z/reference.json`
gesichert. Zugehöriger Messlauf:
`benchmarks/history/20261108T172542.565611Z-2544310.json`.
703 reguläre Rust-, 46 Python- und 46 Bun-Tests sowie Go-Tests/Vet/Race und die
vollständigen nativen Go-/Rust-Cinema-Abnahmen bestanden. Coverage:
43.279/44.580 Zeilen (97,0817 %) und 3.501/3.605 LLVM-Funktionen (97,1151 %).
Die Rohmetriken, Quellenrekonstruktion, alte CLI, Programmeingaben und native
Berichte sind eingefroren. Die TypeScript-Ergänzung liegt daneben unter
`typescript-before-20261108T172542Z/reference.json`; deren vollständiger alter
Bun-Lauf enthält 5.766 erfolgreiche Fälle. Eine aggregierte UNKNOWN wegen
manueller Anforderungen wird nicht als fehlgeschlagener nativer Fall umgedeutet.
`benchmarks/baseline.json` wurde nicht promoviert.

Protokoll 1 für Anwendungstests ist getrennt von SDK 2/WorkerRequest implementiert.
Die reine `ApplicationSession` wählt die ursprünglichen Fälle aus und hält
Vertragsfehler auch nach abgefangenen Fehlern, fehlender Finalisierung oder
erneuten Beobachtungen fest. Go-Bibliothek 0.1.0 und Bun-Bibliothek 0.1.0 sind
lokal im jeweiligen Beispiel gepinnt. Go erhält rohe Bytes vor der Produktion;
Bun verwendet ausdrücklich die native Konstruktorgrenze. Konvertierungsfehler
sind BindingError/UNKNOWN. Float-Ausgabetexte bleiben in der Evidenz erhalten;
ihre Gleichheit folgt der vorhandenen Interpretersemantik, damit `e+308` und
`e308` denselben endlichen Float darstellen. Integer werden nicht über f64
verglichen. Der alte Go-Adapter gab bei Validierung das Eingabewire zurück;
der neue Pfad projiziert das tatsächliche Produktionsresultat.

`verify application-tests prepare` publiziert ausdrücklich einen Check und
schreibt den gemeinsamen Plan samt Datenmodell. `serve` prüft diesen gegen
aktuelle publizierte Anforderungen, Fixtures, Inputs und Budgets und bedient
einen begrenzten separaten Unix-Kanal. Native Logs sind kein Protokollkanal.
Die Ausgabe ist ausdrücklich **diagnostisch**, besitzt keine Übernahmeautorität
und ersetzt noch keinen erfassten Testbuild oder transaktionalen Gruppennachweis.

Ausführbare Pilotstrecken (jeweils mit Zeit-/RSS-Protokollen):

```
python3 scripts/memory_guard.py python3 scripts/application_test_acceptance.py --form decoder
python3 scripts/memory_guard.py python3 scripts/application_test_acceptance.py --form operation
python3 scripts/memory_guard.py python3 scripts/application_test_acceptance.py --form state-change
python3 examples/cinema/scripts/application-acceptance.py --form constructor
python3 examples/cinema/scripts/application-acceptance.py --form operation
python3 examples/cinema/scripts/application-acceptance.py --form state-change
```

Beide Sprachen bestehen 3.862 Datenvertragsfälle, 100 direkte Gewinnerfälle und
102 Speicherfälle mit echten SQLite-Dateien. Im Updatefall wird weiterhin der
geplante `Catalog.FilmCatalogApi.saveFilm`-Setup ausgeführt; tatsächlich injizierte
Store-Calls, SQL-Zugriffe und Events werden beobachtet. Ein Store.save-Shortcut
wird nicht als dieser API-Aufruf ausgegeben. Vorher/Nachher müssen in angeforderter
Reihenfolge, Phase und Scope vollständig sein. Klausel-Orakel haben eigene Tapes
und erfüllen keine Produktions-Callpflichten.

Die einzelnen Pilotpläne ordnen jeweils ihren Bereich neu zu und den vollständigen
Rest ausdrücklich dem alten Backend zu. Sie sind noch **kein gemeinsamer nativer
Ausführungsverbund**. Zwischenstände mit unvollständigen Anschlüssen werden in
`target/{go,typescript}-application-tests/` erhalten. Die dortigen Entwicklungs-
Zeit-/RSS-Werte sind keine vergleichbaren Release-Einsparungen: unterschiedlicher
Umfang und Debug-/Release-Umgebung verhindern eine solche Aussage. Beim Go-RSS
ist die wait4-Grenze (mögliche Vor-exec-Vererbung, kein gleichzeitiger Summen-RSS)
ausdrücklich im Bericht benannt. Bun misst separaten Prozessbaum-RSS.

Noch offen für die vollständige Cinema-Umstellung: gemeinsame Konfiguration und
Ausführung aller Gruppen, weitere Operations-/Mockanschlüsse, Szenarien,
Lifecycle, abgeleitete Identitätsnachweise mit tatsächlichen Quellfällen,
verbindliche native Unterfallcollection in Bun, erfasste Quellen/Testbuilds und
Gruppenevidenz, Store-Guards, Shrinking/Replay, abschließende vollständige Coverage
und vergleichbare Release-Messungen. Der Pilot ist keine Freigabe des neuen
Pfades als Standard für vollständige Projekte. Alte Adapter bleiben verwendbar.

Ergänzung zum Piloten: 101 ursprüngliche Assertionfälle laufen nun ebenfalls über
native lokale Orakelaufrufe. Die Tie-Break-Mutation (`>` zu `>=`) wird damit im
neuen Pfad als `ASSERTION_FAILED` erkannt. Je Sprache sind das **4.165 disjunkte
vorhandene Fälle**, verbleibend 1.601. Dies sind vier einzeln ausgeführte
Prüfbereiche, kein bereits abgenommener gemeinsamer Ausführungsverbund.
Die Testanalyse darf deswegen weiterhin kein allgemeines Skipping freischalten.

Der gemeinsame `scripts/validate.py --coverage`-Runner enthält die neuen
Go-/Bun-Helfertests, die vier positiven Piloten je Sprache, die relevanten echten
Negativproben und separate Release-Messungen für Vorbereitung, Go-Testbuild,
native Ausführung und zentralen Dienst. Katalog-/Publisherkurzläufe mit fehlendem
RSS-Sample bleiben ausdrücklich unbewertet; für veröffentlichte native
Messphasen wird ein echtes RSS-Sample verlangt. Der Go-Pilot misst jetzt wie
Bun Prozessbaum-RSS über den Projekt-Watchdog; ältere wait4-Werte bleiben separat
in den Zwischenberichten und sind nicht mit diesem Maß vergleichbar.

### Tatsächlicher Input-Witness vor der erneuten Gesamtabnahme

Der erste Nachher-Gesamtlauf wurde nach 375.174,688 ms kontrolliert beendet
(exit 130, Prozessbaum-RSS 929.480.704 Bytes), weil der Konstruktoranschluss den
ursprünglichen Wirewert statt des tatsächlich konvertierten Inputs meldete.
Dieser Zwischenlauf ist **nicht abgenommen**; der getrennte Abbruchbeleg steht
in `application-test-post-validation-interrupted.json`.

Der Bun-Konstruktorpfad benötigt nun eine unabhängige Projektion des tatsächlichen
nativen Inputs vor dem Produktionsaufruf. Konvertierungs- und Witnessfehler
bleiben BindingError/UNKNOWN. Die lokalen Go-/Bun-Operationsanschlüsse projizieren
auch ihre tatsächlich konvertierten Argumente in die beobachtete Invocation
zurück. Ein Codec, der einen ungültigen ReleaseYear 1887 durch 1800 ersetzt,
erhält nun INPUT_CHANGED statt eines unberechtigten PASS für dessen Ablehnung.
Auch ein ausgetauschter gültiger Votes-Input wird erkannt, selbst wenn das
Produktionsresultat den Rückgabevertrag noch erfüllt.

Die vorhandene Native-Abbildung hat eine explizite Grenze: 46 negative Fälle
gegen **Nicht-Option-Konstruktoren** kodieren Some(Unit) wie None als undefined
(Bun) bzw. JSON null (Go). Das ist derselbe tatsächliche Input wie vorher; die
ursprüngliche Formannotation bleibt Metadaten und beweist keine native
Unterscheidbarkeit dieser Formen. Die Receipts nennen die Fall-IDs. Bei echten
Option-Verträgen mit kollabierendem Some(Unit) ist ein explizites Tagged-Option-
Binding nötig; sonst bleibt die Fähigkeit UNKNOWN. Zur Erhaltung einer
PASS-Zahl werden weder Produktion noch Anforderungen angepasst.

Nach der Korrektur bestehen die vier Bun-Teilbereiche weiter mit insgesamt
4.165 Fällen; die beiden echten Codec-Swap-Proben scheitern nun gezielt an der
Inputbindung. Die separate Bun-Konformität umfasst 35 Tests (98,57 % Zeilen /
98,41 % Funktionen), striktes Typechecking besteht. Diese fokussierten Befunde
ersetzen weiterhin nicht die neu zu startende vollständige Nachher-Validierung.

Vor dem abschließenden Lauf wird außerdem die zentrale Inventarverletzung
`DUPLICATE_CASE_ID` (sowie `INVALID_OPERATION_CASE`) bereits beim neuen
Anwendungstestplan abgewiesen. Eine BTreeSet-Zuordnung darf zwei unterschiedliche
Pflichtvorkommen nicht zu einem scheinbar vollständigen Test zusammenfallen
lassen. Nicht ausführbare/manuelle Pflichten bleiben dagegen weiterhin im
Manifest sichtbar. Die gezielte Gegenprobe mit derselben Fall-ID und zwei
unterschiedlichen Inputs besteht.

Die acht realen Go-/Bun-Teilpiloten sind zusätzlich als echter Rust-CLI-
Integrationstest integriert. Dadurch laufen ihre Kommunikations-/Bewertungspfade
auch innerhalb von cargo-llvm-cov, nicht nur gegen die uninstrumentierte CLI in
äußeren Python-Phasen. Die fokussierte Integration besteht (33,19 s eigentliche
Tests; gekoppelter Build-/Testlauf 181.800,833 ms, 809.869.312 Bytes Prozessbaum-RSS).
Die erneute Gesamtabnahme beginnt erst auf diesem zusammenhängenden Stand;
unterbrochene Zwischenläufe werden nicht als erfolgreiche Nachher-Validierung
gezählt.


## Fortsetzung vom Pilot zur Release-Abnahme (Protokoll 2)

Der Nutzer verlangt die vollständige Umsetzung, ohne nach einem Zwischenlauf
aufzuhören. Der v2-Stand (Bibliotheken 0.2.0, SDK/Worker unverändert) führt
alle 5.766 ursprünglichen Cinema-Fälle in Go und Bun aus: 3.862 Verträge,
1.784 Operationen, 101 Assertions, 12 Szenarien, zwei Lifecycle-Fälle und fünf
zentrale Identitätsableitungen. Die vollständigen positiven Entwicklungsbelege
vergleichen Fallinventar, Seeds/Budgets, tatsächliche Invocation-Inputs und
Argumente mit der eingefrorenen Vorherreferenz. Das ist noch keine vollständige
Release-Abnahme.

Generierte typisierte Fassaden binden neun konkrete Produktions-Owner;
handgeschriebene Fixtures liefern echte SQLite-Datenbanken, Ports, HTTP und
Beobachter. Im Go-Projekt bleibt die bestehende generische Produktions-API
`Invoke` erhalten. Generierte Argumentprüfungen und feste Operationsaufrufe
ersetzen keine statisch typisierten Produktionsmethoden, die dort nicht existieren.

`ApplicationSession` verwendet den bestehenden Scenario-Bewerter,
OperationMockSession und einen eigenen Lifecycle-Treiber mit tatsächlichen
Vorher-/Nachher-Snapshots. Kollaborator-Anfragen gehören dem ausstehenden
Produktionsschritt. Orakel erfüllen keine Produktionscalls; Mock-Trace und
tatsächliche zentrale Anfragen müssen übereinstimmen. Identität referenziert
Operationsfälle derselben Sitzung und erhält keinen nativen Wahrheits-Callback.

Die Engine erhält `verify application-tests run`: erfasste Quellen, explizite
native Rezepte, private Bewertungskanäle, vorhandene Prozessüberwachung und
unabhängige Go-test2json-/Bun-JUnit-Collection. Ambient node_modules werden
nicht ausgeführt; frozen/offline Installation materialisiert sie im Capture.
Go vendort seine Abhängigkeiten im Capture. Quell-, Build-, materialisierte
Abhängigkeits- und native Artefakthashes werden gebunden und erneut geprüft.
Buns Katalog nutzt echte Registrierungsaufrufe über ein ersetztes bun:test-API;
dies ist keine offizielle Bun-Testlistenfunktion und keine native Einzelcase-
Enumeration. Unsupported Registrierungsformen bleiben sichtbar fehlerhaft.

Die gemeinsame Test-/Performance-Pipeline wird um vollständige Suiten und
verwaltete Läufe erweitert. Task-Übernahme, Store-Verbundautorisierung,
Rust-Bibliothek und Consumer-Migration bleiben nachfolgende Etappen; ein
importierter oder diagnostischer Gruppenbericht erhält keine solche Autorität.

Offene Gates vor Abschluss: verwaltete Läufe samt Gegenproben, native Replay-
Anbindung, mindestens 97 % Rust-Zeilen/Funktionen, vollständiger quellenstabiler
Validierungslauf und gesicherter Zeit-/RAM-Vergleich. Die akzeptierte globale
Benchmarkbaseline bleibt unverändert.

Weitere Release-Anbindung: Die Go-/Bun-Bibliotheken werden jetzt targetseitig
mit `include_str!` eingebettet und über `verify application-tests library`
ausgeliefert. Veröffentlichung prüft vorhandene Pins vor allen Schreibzugriffen,
verweigert geänderte oder fremde Dateien und publiziert den neuen Pin zuletzt.
Handgeschriebene Tests sowie Produktionsabhängigkeiten werden nicht umgeschrieben.
Die Go-Projektcodec-Metadaten gehören nun dem expliziten Clientkontext; globale
mutable Typ-/Operationsregister sind entfernt.

Native Replay übernimmt den unveränderten Originalplan und wählt ausschließlich
Fallzuständigkeiten neu. Identitätsreplay führt seine Quellfälle tatsächlich
erneut aus; Lifecycle-Pfade bleiben zusammen. Der gespeicherte Planhash muss
passen. Replay-PASS ist keine Wiederverwendung des historischen PASS und erhält
keine Übernahmeautorität. Die integrierte Gegenprobe prüft Original-PASS,
Produktionsmutation-FAIL und repariertes PASS im zuständigen nativen Test.

Abbruch vor der ersten vollständigen v2-Coverage: Der gemeinsame Runner setzt
einen längeren TMPDIR unter ~/.cache. Der aus Capture plus Artefaktverzeichnis
gebildete Unix-Socketpfad war dort 127 Bytes lang und damit unzulässig. Der
Lauf wurde nach 504.256,886 ms kontrolliert beendet (exit 130;
958.976.000 Bytes Prozessbaum-RSS), statt ihn als Abnahme zu zählen. Der
Nachweis steht in application-test-post-validation-interrupted.json. IPC nutzt
nun ein eigenes kurzes Verzeichnis mit Modus 0700, Socketmodus 0600 und
Inode-/Device-Guard für Cleanup; der Kontext verweist weiterhin explizit auf
den tatsächlichen erfassten Anwendungsroot.

Der echte Langpfad-Regressionslauf zeigte anschließend einen tatsächlichen
Gesamttimeout: 120 Sekunden auf dem persistenten btrfs-TMPDIR statt tmpfs;
5.509 Fallaufzeichnungen, 258 noch fehlende Fälle, Ergebnis UNKNOWN, kein PASS.
Die neuen Cinema-Skripte setzen deshalb ausschließlich
`ApplicationTestConfiguration.groups[*].resources.timeout_seconds` über den
expliziten neuen Prepare-Parameter `--group-seconds 300`. Der native Go-
Testbinary-Watchdog wird ebenfalls auf 300 Sekunden begrenzt, der äußere
Mess-Watchdog auf 420 Sekunden. Die bestehende Adapterkonfiguration mit ihren
bisherigen 120 Sekunden sowie Seeds, Corpus, Interpreterbudgets, Inputs und
Anforderungen bleiben unverändert.

Konfigurationsdiagnose folgt dem lokalen Skill
`/home/held/.codex/skills/dctx0-config/SKILL.md`. Der neue Dienst übernimmt
`execution.case_seconds` (sonst 5) und `execution.evidence_bytes` (sonst 4 MiB)
als explizit exportierte, bei Wiederlesen erneut geprüfte Fallgrenzen.
Kontextbezogene eingehende Beobachtungen werden vor ihrer Retention begrenzt;
Zeitlimits gelten auch beim Warten auf die nächste Beobachtung/Antwort.
Transport-/Fallfehler stoppen den verwalteten nativen Prozess und bewahren den
unvollständigen Präfix. Effektive Start-/Fall-/Gruppen-/Cleanup-/Nachrichten-/
Evidenz-/Loggrenzen stehen im nativen Receipt. Echte Gegenproben für Falltimeout
und Evidenzüberlauf bestehen.


## Bewachte Anwendungstest-Abnahme (2026-11-09)

`verify application-tests run` speichert nun native Evidenz und versucht über
dieselben transaktionalen Store-Guards wie `verify run` Task-Übernahme.
`--no-adopt` unterbindet Tasklieferung. `complete` verwendet den vorbereiteten
Check/Snapshot, aktuelle Profile und quellengebundene Autoreninspektionen und
die gemeinsame Abschlusslogik des bisherigen `verify complete`. Vorbereitung,
Dienstberichte, Bibliotheksabschluss, Import und Replay autorisieren allein
keine Übernahme; `adoption_authority` verlangt tatsächliche Tasklieferung.

Ein frischer Go-Komplettlauf hat alle 5766 Fälle und 105 Done/PASS-Tasks bestätigt,
darunter vier explizite nativ gestützte Autorenreview-Attestierungen. Eine bloße
Reviewfreigabe oder manueller Abschluss setzt weiterhin kein Test-PASS; bestehende
Done/Unverified-Reviews bleiben unverändert. Dies korrigiert die frühere Erwartung
von 101 PASS plus vier Unverified für einen frischen Autoren-Komplettlauf.
Vollständige aktuelle Workspace-QA, mindestens 97 % Rust-Zeilen/Funktionen und
gekoppelter Release-Zeit-/RAM-Vergleich bleiben offen. Frühere Diagnoseberichte,
fehlgeschlagene Versuche und unveränderte Benchmarkbaseline bleiben erhalten.


## Tatsächliche Projektabschlüsse und letzte Gegenproben (2026-11-09)

Beide tatsächlichen Projektstores, `examples/cinema/dctx0` (Bun) und
`examples/cinema-go/dctx0` (Go), enthalten jeweils 105 Done/PASS-Tasks.
`benchmarks/application-tests/root-completion-20261109T010744Z/reference.json`
bewahrt komplette Abschlussberichte und erneut lesbare native/Inspektions-Exporte.
Die vier Review-Aufgaben haben explizite quellengebundene, nativ gestützte
Inspektionsnachweise. Historische oder manuelle Statusänderungen genügen nicht.
Bun führt die gepinnte TypeScript-Prüfung direkt mit Bun aus; ein indirekter,
nicht gebundener Node-Aufruf über den TSC-Shebang entfällt.

Der letzte vollständige Diagnoseversuch ist unter
`benchmarks/application-tests/failed-after-20261109T024110Z/` erhalten:
738 Rust-Tests bestanden, aber 96,5952 % Rust-Zeilen und 96,2887 %
LLVM-Funktionen verfehlten die unveränderten 97-%-Gates. Zusätzlich kompilierte
die synthetische Go-Cleanup-Mutation wegen ungenutzter Variablen nicht; die
Gegenprobe erhält jetzt den ursprünglichen Ausdruck und erzwingt zusätzlich
den Fehler. Dies ist kein akzeptierter Nachher- oder Benchmarkbaseline-Lauf.
Gezielte zusätzliche Tests prüfen Zustands-/Protokollgrenzen, tatsächliche
Kollaboratortraces, reine Produktions-/Orakeleffekte und Abschlussfehler.
Beide nativen Sprachen wiederholen auch verbundene Lifecycle- und
Entity-Identitätsfälle; die Identitätsquellfälle werden tatsächlich neu ausgeführt.
Der vollständige quellenstabile Nachher-Lauf bleibt bis zum erfolgreichen
Coverage- und Messabschluss offen.


## Abnahme des Cinema-Releasepfads (2026-11-09)

Der vollständige quellenstabile Lauf
`benchmarks/history/20261109T060938.334343Z-3865530.json` besteht alle 99 Schritte
und 126 Release-Zeit-/RAM-Workloads. Quellhash vor/nach:
`5ad0adc52bdfdbb7140d15a9ccce385b0b9755fb01cfb900ef5a3dd8dc623a87`.
773 All-Feature-Rust-Tests und 47 Python-Tests bestehen. Rust-Coverage:
47.437/48.889 Quelllinien (97,0300 %) und 3.798/3.914 LLVM-Funktionen (97,0363 %).
Die unveränderten 97-%-Gates sind erfüllt; Rohmetrik-/Helper-Grenzen stehen in
der Coverage-Referenz. Kein Branch-Vollständigkeitsanspruch folgt daraus.

Beide nativen Sprachen liefern alle 5766 Pflichtfälle und 105 Done/PASS-Tasks,
einschließlich vier expliziter nativ gestützter Review-Attestierungen. Alle sieben
Go- und 18 Bun-Mutationen sowie die sechs verwalteten Nichtübernahme-Gegenproben
bestehen. Der Assertion-Replay folgt PASS → Produktionsmutation FAIL → Reparatur
PASS. Verbundener Lifecycle-Replay führt zwei Fälle aus, Identitäts-Replay 239
Fälle samt tatsächlichen Quellbeobachtungen. Replay bleibt ohne Übernahmeautorität.

Der gekoppelte Gesamtlauf benötigte 9.448.047,515 ms mit maximal
1.575.497.728 Bytes gemessenem Prozessbaum-RSS. Dieser Gesamtaufwand enthält
einen größeren Prüfumfang als die Vorher-Referenz und ist kein gleichartiger
Workloadvergleich. Verwaltete vollständige Release-Ausführung: Go 33.125,35 ms /
1.041.174.528 Bytes; Bun 25.703,94 ms / 718.131.200 Bytes. Die Einzelstichproben
bleiben gegen die akzeptierte Baseline unbewertet; sie wurde nicht verändert.

`benchmarks/application-tests/after-cinema-native-20261109T060938.334343Z/`
enthält Vorher-/Nachhervergleich, Rohdaten, Quellenrekonstruktion und native
Nachweise. Der tatsächliche Projektabschluss bleibt separat unter
`root-completion-20261109T010744Z` erhalten. Dieser Abschluss betrifft Cinema
unter Linux mit Go/Bun; backendübergreifende Verbundübernahme, Rust-Anwendungstest-
Bibliothek, Runner-Consumer und allgemeines Test-Skipping sind damit nicht abgenommen.

## Runatron migration continuation: preserve existing evidence

The user requested freezing the dirty Runatron model before full native-test
migration and explicitly prohibited deleting sources/history to obtain approval.
A consistent source archive and three online SQLite backups are retained under
`../runatron-business/reports/dctx0-application-migration/baseline-20261109T062250Z/`.
The first direct PreparedPlans test exercises actual Store.Save, observed resources,
real SnapshotFile persistence and reopen; existing adapters and tests remain.

Before saving new native runs we found automatic pruning in Store publication.
The new explicit `history_retention: "preserve-all"` setting adds an immutable,
non-authorizing retention marker before pruning in the same transaction. A linked
hold preserves the entire database, including orphan bytes and writes from other
project keys. Existing schema versions and default minimal retention remain.
GC/export pruning and verification compaction/install are blocked while held.
See `docs/history-preservation.md`. Focused tests and subsequent complete
consumer/tool acceptance must prove this increment; the earlier accepted Cinema
reference remains historical evidence for its own source hash.


## Runner: maximal hybrid scope (explicit user correction, 2026-11-09)

Keep the existing PluginHost migration. Do not migrate Identity/Access. In the
Runner preserve functional adapters; connect only unfinished work and missing
tests through application tests. The new Runner assignment retains its entire
35,777-case manifest and unchanged inputs/seeds/budgets: 35,676 legacy cases,
101 application-owned PreparedPlans.save cases. Existing optional 645-case
native suite and evidence remain retained. Scope evidence is in the consumer's
`reports/dctx0-application-migration/runner-hybrid-scope-20261109/`.

`hybrid_assessment::assemble_hybrid_cases` is pure domain validation, not a
native execution proof or task delivery entry point. It checks exact disjoint
ownership, fresh group execution windows, native origin, common source/spec/
configuration/task pins and budgets, target/protocol identity, mandatory case
kinds and PASS observations. It rejects imported/historical or missing groups.
Actual runtime/build receipts and independent transactional Store validation are
still required before this can authorize a combined run.

`Project::run_adapter_execution_group` adds diagnostic execution of centrally
assigned legacy cases. The original full ProgramPlan remains unchanged; case
selection is checked against the current program before execution. Contract,
operation, assertion, scenario, lifecycle and derived identity iterations skip
unassigned cases. Existing whole-plan entry points pass no selection and retain
their behavior. A partial group explicitly retains HYBRID_COHORT_REQUIRED and
cannot claim project PASS or adopt tasks. This is not yet the released hybrid
cohort orchestrator. The Store still requires the complete original manifest.

Next integration: execute both backends in one managed window, bind their actual
native receipts and source inputs, derive cross-group entity identity from real
source observations, then independently recheck the complete cohort in Store
before existing guarded task delivery. Never assemble old saved PASS reports to
supply a group. Preserve all existing histories and foreign claims.

### Managed hybrid implementation checkpoint (2026-11-09)

`Project::run_hybrid_verification` now executes one application group and one
adapter group freshly in a single occurrence. It pins the task/profile context
before both groups and binds the same original source inventory. Generated
adapter replacements remain separately bound by build/harness artifacts. The
full central manifest is unchanged; native cases seed the combined assessment
before legacy cases and derived entity identities. Saved reports are not inputs.

`HybridExecutionProof::validates` verifies the two assignments, actual native
receipt and common execution window. The Store independently validates the
content-addressed proof, complete manifest, receipt phases, process supervision,
runtime identities and limits. Its mutation/rollback tests do not replace real
native execution acceptance.

The real Bun/Bun integration test
`hybrid_execution_runs_both_real_backends_with_one_complete_current_case_inventory`
passes the complete 39-case diagnostic cohort, with native-owned B explicitly
rejected by the old adapter if incorrectly dispatched. Retained measurement:
`hybrid-two-backend-native-test-v3.log`, 32,508 ms and 860,729,344 bytes peak RSS
(includes Cargo preparation). This is a focused development measurement, not a
comparable release benchmark or Runner acceptance. Store-backed delivery,
Done/Unverified publication, end-of-cohort runtime freshness and the consumer
wrapper are being completed next. The installed CLI has not been upgraded for
these changes. Identity/Access remain outside this migration.

Subsequent focused evidence now includes actual Store delivery for both real
backends (34,103 ms / 865,284,096 bytes RSS), the real CLI JSON/text path and
report-preservation check (95,942 ms / 874,713,088 bytes), and atomic fresh
Done/Unverified plus Open publication with historical result/review retention
(98,622 ms / 868,532,224 bytes). Measurements include compilation/lock waiting
and are development evidence, not compatible release baseline comparisons.

The complete Runner diagnostic has started with an explicitly copied development
CLI, isolated PostgreSQL/filesystem/Git-LFS fixtures and no task delivery. Its
first native group has 101/101 PASS; this is not a complete cohort verdict. The
system installation and consumer toolchain pin remain unchanged.

Complete hybrid acceptance must also resolve manual requirements from actual
current authored inspections before native publication. The new locked hook
recomputes the central effective assessment; Store independently reproduces the
inspection and assessment bindings. Imported inspected reports still have no
native adoption authority. Full real integration and final validation of this
hook remain pending at this checkpoint.

### Fresh hybrid evidence for completed tasks

The hybrid entry point captures active Done/Unverified Contract and Operation
state versions before either backend starts. The engine checks eligibility using
the pinned requirements and actual case-to-group mapping; historical Done
prerequisites remain delivered for scheduling. The Store rechecks the complete
run, profile and task pins, versions, claims and reservations in the same
transaction that publishes Open deliveries and appends native checks to eligible
Done tasks. Existing task results, definitions and reviews are retained. One
receipt and actual run links cover the combined batch; the older Open-only API
and receipt format remain readable. Adapter, integration, retirement, review and
adjustment tasks need their own evidence.

When `complete_hybrid_verification` receives explicit authored inspections, the
locked fresh-run hook validates the current source/spec bindings and runs the
central effective assessment against the original cases and plan. It adds the
inspection and assessment artifacts to that same native occurrence only for a
complete PASS; absent or insufficient notes do not imply approval. The Store
independently loads the content-addressed hybrid proof, full plan, native receipt,
process audits and inspection artifacts and recomputes the assessment against
the saved snapshot and cases. Imported assessment reports remain diagnostic and
cannot authorize native task publication. A note may name a subset of reviewed
files; their recorded hashes must match the captured inventory before the run
and are checked again under the project lock. The full captured implementation
inventory binds execution identity, not a claim that every captured file was
manually reviewed. The Engine checks the current source and runtime under the
project lock; Store artifact checks do not independently inspect the filesystem
or establish who authored a note.

Focused development tests `hybrid_done` run both actual Bun backends and check
Done/Open atomic delivery, historical-result preservation, an uninspected
manual obligation remaining UNKNOWN, and explicit inspection leading to a fresh
native assessment. `hybrid-done-inspections-engine-retry.log` records 31,995 ms
and 870,801,408 bytes peak process-tree RSS, including compilation. Store proof
and adversarial inspection tests are recorded in
`hybrid-done-inspections-store.log` (37,778 ms, 771,903,488 bytes). These fixture
runs do not establish Runner application acceptance or a comparable release
benchmark; the installed CLI and accepted benchmark baseline are unchanged.


### Runner hybrid integration checkpoint (2026-11-09)

The user narrowed consumer migration: preserve Identity and Access, preserve
already migrated PluginHost, and finish Runner with the maximum compatible hybrid
scope. Existing working adapters remain in use; new application tests cover
unfinished connections. No historical PASS supplies an execution group.

Three fresh full Runner diagnostics have been retained without task delivery.
The latest executed 31,286 of 35,777 required cases: 30,982 PASS, 103 FAIL,
201 UNKNOWN, and 4,491 not executed. Its native PreparedPlans.save group passed
101 cases; actual Docker and Kubernetes each passed 607 cases. These numbers
are diagnostic evidence, not whole-Runner acceptance. Follow-up corrections and
a direct Publish connection require a new complete cohort against final sources.
The installed CLI and consumer pin have not yet been updated.

The integrated explicit inspected-Done path preserves old task definitions,
results and reviews. Exact task requirements, current reviewed sources and
profiles, explicit findings, and fresh complete native evidence are checked in
one guarded publication transaction. Manual fact findings and actual passing
case mappings remain separate. Status requires the corresponding guarded event
and report hash; copying a report does not establish authority. Schema-6 readers
return unknown for unavailable schema-7 inspected receipt metadata without
migration; garbage collection preserves linked artifacts. Focused schema-6 and
schema-7 adversarial tests pass; final workspace validation remains outstanding.

The first subsequent full validation was intentionally interrupted before
coverage and performance when nested measurement writers were found not to
inherit preservation. Its logs and caches are retained under
`benchmarks/artifacts/hybrid-validation-interrupted-20261109T120847Z/`.
All 227 history JSON files present at the checkpoint remained unchanged.
All measurement writers now inherit the explicit history hold, and validation
uses unique retained artifact directories. Re-run
`python3 scripts/validate.py --coverage --preserve-history` after source integration.
The accepted baseline and both 97% coverage thresholds remain unchanged.

### Runner follow-up and validation checkpoint (2026-11-09)

The selected assignment now contains 201 native cases (101 PreparedPlans.save
and 100 AttemptWorker.Execution.publish) and 35,576 adapter cases. The subsequent
complete attempt recorded 26,329 PASS, zero FAIL and one UNKNOWN from Host
provisioning timeout; 9,447 mandatory cases were not executed. Its preserved
report is `runner-full-hybrid-201-prepared-20261109` in the consumer's migration
reports. This is not complete acceptance, and partial runs cannot be assembled
into a passing cohort.

The user-approved CanonicalFingerprints clarification now explicitly retains
the existing payload digest with independently checked attempt, epoch and
sequence identity. The reviewed plan at check 114 preserves every case, input,
seed, budget and backend assignment; only that governing manual finding and
snapshot provenance change. Its previous bytes remain archived. Normal task
reconciliation and fresh evidence remain required; historical results were not
rewritten. The separate proposed Client.connect reconnect clause remains
unapproved and unapplied.

Keyed state-result projection is integrated. It permits the typed result key to
select a modeled entity in actual observed state, while retaining independent
payload, scope, other-row and write checks. Other result references and
unsupported projections remain incomplete. Budget checks precede cloning, and
collection binders retain lexical scope. The 39 focused Rust tests pass,
including wrong-key/write, shadowing, Variant/Trace and depth/step-budget probes.
This capability alone does not repair the unchanged `single` reconnect clause.
The implementation lives in
[`projection_within_budget` and `bind_changed_row_keys`](../crates/dctx0-verification/src/state_projection.rs),
called by the independent state-result assessment in
[`operation_state.rs`](../crates/dctx0-verification/src/operation_state.rs).
Regression coverage is retained in `state_effects.rs`,
`keyed_projection_adversarial.rs` and `keyed_projection_passthrough.rs` under
`crates/dctx0-verification/tests/`.

The fourth full validation completed unsuccessfully and retained its raw
artifacts and measurements in
`target/validation-runs/20261109T125016.264458Z-1615360-q3944u9g/` and
`benchmarks/history/20261109T153351.367197Z-1615360.json`. Its source hash was
unchanged during checks. A test probe incorrectly expected planned tasks after
the source-review guard had already rejected a changed reviewed recipe. The
integrated correction keeps that guard and tests genuine native catalog/build
failures separately; four focused Python regressions pass. A separate Go Cinema
performance acceptance failure is under retained-fixture investigation. Neither
this run nor its partial coverage instrumentation establishes a passing release.

Consumer corrections include actual constructor-return projection, separate
pure input construction and stored authority bindings, and a Local shutdown fix
that confirms supervisor/helper exit before journaling cessation. The previously
failing WSS families pass 21 tests across three repetitions; six Go packages,
targeted race detection and Runner lint pass. Authorized Artifact HTTP routes
are implemented without Identity/Access package changes. Focused coverage of
the eleven Artifact production files, including the shared definition/artifact
transfer lane, reaches 425/425 lines and 90/90 functions in one instrumented run
with 25 passing tests. The broader Runtime/Artifact/Client/Host suite passes 43
tests. Whole-consumer coverage and complete fresh hybrid evidence remain
open. MCP Home configuration isolation and exact negotiated revision checks are
also open; staged transport prototypes do not establish production support.

All of these findings precede a final shared source capture, current authored
reviews, guarded task publication and local binary/pin upgrade. The installed
CLI is still the older version. No benchmark baseline was promoted and no
coverage threshold was reduced. Sixteen tracked benchmark records removed by
earlier history rotation were restored byte-for-byte from Git, preserving the
accepted baseline and all existing files.

The Go/Rust native performance compatibility fix is integrated. It uses separate
short on-disk workspaces, retains original reviews and databases under the hold,
publishes receipts atomically and reads a unique binary-bound report rather than
a stale fixed pathname. The Linux measurement owner also reaps identified
descendants that create separate sessions, on interruption and unexpected CLI
exit. Unrelated control processes remain untouched in regression tests. These
conditions are explicit native workload settings; established core workloads and
the accepted baseline remain unchanged. Forty-two focused Python tests pass.
Separate complete Go and Rust runs passed with the preceding owner implementation;
the final PID-start recheck and error paths have focused actual-process tests.
The next full workspace validation remains required for the integrated source.

## Selected authority assessment correction (2026-11-09)

The Runner onboarding audit found two concrete central-assessment errors without
changing Identity/Access. A real successful token operation remained UNKNOWN
when its result fields were constrained inside `all(gate in
before.InvocationAuthority ...)`. Conversely, successful Invite/Enroll captures
with only the observed authority key changed from `current` to a foreign key
could incorrectly pass through the existing unrelated-result fallback.

`operation_state.rs::has_independent_result_field_constraint` now recognizes
the bounded, unshadowed universal guard only for exactly one actual Before row
matching the complete state selector and the same entity. Regular postconditions
still evaluate actual returned values and actual pure-call continuations. Empty,
unrelated, nested or vacuous scopes do not acquire projection authority. The
existing direct result-field fallback also requires an actually observed matching
key; additional unchanged rows remain supported. Entity/option/list projection,
state-preservation checks, finite inputs and backend assignment remain intact.

The Rust positive regression uses structurally different entity/result records
so it exercises this fallback. The identical case is UNKNOWN on the previous
Root implementation and PASS with the correction. Real private PostgreSQL/Host
captures separately exercise receipts, onboarding successes, denials, replay,
expiry and actual policy continuations. They are selected diagnostic invocations,
not fresh executions of the complete Runner manifest or task acceptance.
Manipulated registration/tenant values, absent or wrongly keyed authority,
changed After state, insufficient budgets and shadowed binders remain FAIL or
UNKNOWN. All 41 targeted tests pass on the integrated Root source in 15.680 s,
with 545,083,392 B sampled process-tree peak RSS.

The previous workspace validation (QA5) was deliberately interrupted through its
measurement owner before changing sources, after the incorrect-PASS behavior was
confirmed. It exited 130 and remains incomplete: its artifacts, source archive,
8,304,606.795 ms elapsed measurement and 1,566,564,352 B peak RSS are retained.
It cannot authorize release or adoption. A complete new validation with coverage
and release speed/RAM measurements is required. No task status, specification,
library protocol, installed CLI, Consumer pin or accepted baseline was changed
by this correction.

## Incremental evidence checkpoint (2026-11-10)

The user requested guarded reuse of database evidence to avoid repeated full
Runner execution. This extends the earlier exclusion of automatic reuse; it does
not make historical PASS automatically authoritative.

Implemented: `incremental::{program_units,binding,plan_reuse,validate_proof}` in
the verification crate, read-only `Project::verification_incremental_plan`, and
`verify incremental-plan SPEC RUN_ID --project APP`. The CLI compares actual
stored plan/inventory artifacts and current sources/configuration without
execution or task changes. The store and native task proof validator explicitly
reject incremental publication/adoption while external-input binding is absent.

**This is not completed incremental acceptance.** No native cases are currently
skipped or reused. Existing workers supervise processes without establishing a
complete boundary for external filesystem, network, time or fixture inputs.
An invented receipt or unchanged sources alone would not supply that proof.
Partial execution, compiler-selected dependency units, parent retention and
independent transactional validation of a versioned incremental hybrid cohort
remain necessary. The old fresh hybrid proof retains its meaning.

The [incremental verification reference](site/src/reference/incremental-verification.md)
records the exact command, invariants and remaining scope. Targeted tests cover
compatibility changes, invalid units, imported evidence, altered copied records,
database publication/adoption rejection and read-only CLI/engine comparison.
No full validation, Runner cohort, installed binary update or benchmark baseline
promotion is implied by this checkpoint.

## Reuse of unchanged repository validation (2026-11-10)

`scripts/validate.py --coverage` now implements actual reuse of its own complete
successful workspace validation, separately from native case adoption. The
append-only SQLite receipt cache in `target/validation-cache` binds repository
inputs (including untracked/deleted paths and linked file content), installed
JavaScript dependencies, selected toolchain binaries, Cargo configuration,
environment, baseline and options. Evidence records, LCOV and raw LLVM JSON
are hashed and checked on lookup. Inputs are rechecked before returning a hit.
Only a complete measured validation with passing checks and performance gate,
matching before/after source and cache identities can publish a receipt.
Failed, interrupted and partial runs cannot populate it.

A hit prints `REUSED` with the original timestamp and evidence paths, executes
no tests or benchmarks and makes no new task or performance claims. `--force`
executes again; `--no-reuse` disables cache use; an optional maximum age supports
freshness policy. Corrupt or unavailable caches fall back to execution. No
existing failed/interrupted report is retrospectively accepted. The first
complete success is therefore still necessary. External system/service state
is not isolated; explicitly force execution to retest changed external conditions.
Native case reuse and incremental hybrid task delivery remain separate open work.

## Consumer binding repairs (2026-11-10, acceptance still active)

`GeneratedPreparation.argument_bindings` and
`OperationPlan::generated_argument_bindings` retain typed setup-to-setup and
setup-to-final argument flow. Empty bindings retain historical serialization.
The native executor and application session resolve inputs from actual setup
returns; `validate_recorded_operation_arguments` independently checks the stored
invocations. Replay preserves reviewed templates rather than previous runtime
IDs. Missing bindings or substituted inputs do not establish PASS.

Repeated identical scenario givens are separate required interactions: the next
unexercised matching preparation is selected; conflicting outcomes still fail,
and unused preparations stay incomplete. A real empty trace remains distinct
from an absent observer.

Read-only state checks preserve all actual rows before assessing their result.
Mixed observed authority plus a mutating state clause checks the result at the
mutation. An independent receipt field can acknowledge a collection input, but
a response containing the observed entity still requires its real projection.
Conditional/vacuous constraints, missing observers and extra writes cannot
satisfy this rule.

Rejection generator `dctx0-rejections-4` adds complete-list key duplicates with
valid elements and cross-field record candidates with individually valid field
values. Only the complete outer contract's actual rejections enter the corpus.
This is an intentional generator identity change; old runs retain their pins.
Consumer plans must be regenerated centrally and compared, never edited to
remove failing inputs. Explicit Float literals retain Float semantics; exact
Decimal values are not rounded to force native compatibility.

The current focused core evidence is 52 passing scenario/state/rejection tests
(27,644ms, 560,824,320B sampled process-tree RSS). This checkpoint is not a new
complete workspace validation or complete Runner acceptance. Consumer source
reviews, current mixed evidence and guarded task delivery remain separate gates.

## Runner consumer integration checkpoint (2026-11-10, acceptance active)

Identity/Access retains its existing verification backend. Runner uses the
reviewed mixed assignment of 35,817 mandatory cases: 201 application-test cases
and 35,616 adapter cases. The assignment, IDs, seeds and budgets are preserved.
The latest cancelled run recorded 29,341 PASS, zero FAIL, one interrupted case
and 6,475 unexecuted cases. It did not deliver tasks or seed historical reuse.

That run exposed avoidable fixture cost: generated handles without setup were
provisioning actual CSI resources before production rejected their absent journal
entry. Local and Kubernetes adapters now invoke the real controller/backend with
an empty observed durable journal for those calls. Physical API use in this path
fails explicitly. Prepare and reviewed setup retain real physical fixtures.
All 500 original handle tuples per backend pass focused fresh-session regressions
without input substitution or journal writes. Nine real CSI cases pass; the
subsequent backing-file guard was also checked through a real physical upgrade.
Corrupt or unexpected retained state blocks calls, observations, upgrade and
Close. Cleanup retires only the actual journal owner matching the supplied handle.
These focused results do not replace a current full central run.

Consumer `complete-cache.py` version 4 conservatively binds inputs and the task
database, preserves immutable reports and requires complete PASS plus owned
fixture cleanup. Its observation switch and sampled outer watchdog startup RAM
budget are excluded from identity; enabled supervision, configured native limits
and other environment inputs stay bound. No partial result is eligible. The
normal Runner flow can verify a real new-process `--reuse-only` hit immediately
after saving a fresh receipt. The current full native run and first real receipt
proof remain pending. Strict consumer Go coverage and full project acceptance
are separate open gates; neither a SOURCE review nor a native group PASS clears
them.
