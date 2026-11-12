# Umbauprüfung: Anwendungstest-Bibliotheken für dctx0

Stand: 2026-11-08. Zugehöriges [Konzept](application-test-concept.md). Status: Quellcodegestützte Architektur- und Migrationsprüfung zur beschlossenen Richtung: neuer Standard für neue Anbindungen, bestehendes Backend kompatibel weiterführen und beide innerhalb eines Projekts kombinieren. Die Migration und der neue Mischbetrieb sind noch nicht implementiert oder abgenommen.

## 1. Untersuchungsbasis und Grenzen

Geprüfter dctx0ctl-HEAD: `789454f22f8fb8bda3d203759ebfb28d3d10be92`. Der Arbeitsbaum enthielt bereits Änderungen in `crates/dctx0-verification/src/generation/strings.rs`, `projection/mod.rs`, den Tests `operation_arguments.rs` und `string_generation.rs` sowie die neue Datei `projection/setup_arguments.rs`. Diese Arbeit wurde nicht verändert. Die Prüfung berücksichtigt die sichtbaren Schnittstellen im Arbeitsbaum; der HEAD allein identifiziert deshalb nicht sämtliche gelesenen Inhalte.

Aus der vorangegangenen Untersuchung wurden die runatron-business-Adapter, Produktionsdecoder, Journale, native Statusausgabe und historischen Prozessbefunde einbezogen. Dessen aktueller Arbeitsbaum ist ebenfalls kein eingefrorener Abnahmestand. Frühere Taskzahlen werden hier nicht als aktuelle Lieferquote wiederverwendet.

Die Untersuchung deckt die Architekturflächen aller Workspace-Crates, Targets und der angrenzenden Auslieferungswege ab. Schlüsselstellen wurden direkt gelesen; unterstützende Komponenten anhand ihrer Module, Schnittstellen und Referenzdokumentation eingeordnet. Das ist kein behaupteter vollständiger Zeile-für-Zeile-Sicherheitsreview sämtlicher Dateien. Insbesondere sind neue Bibliotheken, Compilerinstrumentierung, End-to-End-Prototyp und Performancevergleich noch nicht gebaut.

`AGENTS.md` enthält historische Abschnitte, die heute implementierte native Verifikation noch als geplant beschreiben. Die referenzierte `docs/site/src/reference/verification-architecture.md` fehlt im Checkout. Für aktuelle Aussagen wurden deshalb vorhandener Code, die neueren Referenzen und datierte Validierungsberichte herangezogen. Historische Dokumente werden durch diese Prüfung nicht rückwirkend geändert.

## 2. Zentrale Befunde

1. **Die Bewertungslogik ist bereits weitgehend von I/O getrennt.** `operation.rs`, `operation_state.rs`, `operation_effects.rs`, `scenario.rs`, `lifecycle.rs` und `entity_identity.rs` arbeiten auf Plan und Beobachtungen. Das ist der wichtigste Wiederverwendungsansatz.
2. **Das Pflichtmanifest ist unabhängig vom Worker.** `manifest.rs::case_manifest` legt Fälle, Zuordnung und offene Pflichten fest. Ein Anwendungstest darf diese Autorität nicht übernehmen.
3. **Die gegenwärtigen Scaffolds invertieren die Ausführung noch nicht.** Go erzeugt eine Fixture und startet den zentralen Plan; TypeScript `runPlan` startet die CLI. Ein Wrapper um diese Funktionen spart den universellen Adapter nicht ein.
4. **Bindings enthalten genau den teuren Vermittlungsapparat.** `core/bindings.rs` modelliert Session-Factory, Cleanup, Invoke-Hooks, Parameterprojektionen, Failure-Mapping und Observer. Ein großer Teil kann im neuen Pfad durch direkte Testaufrufe ersetzt werden; Konvertierung und Beobachtung verschwinden nicht vollständig.
5. **Store und Task-Übernahme erwarten konkrete native Evidenz.** `store/tasks/native_proof.rs` prüft Manifest/Plan und reproduziert unter anderem Entity-Identitätsnachweise. Ein neuer Import von Test-PASS darf diese Kontrollen nicht umgehen.
6. **Fehlerisolation ist eine eigene Änderung.** `verification_run/execution.rs` bricht nach nativen Fehlern mit Worker-Abbruch ab. Das neue Gruppenmodell verlangt zusätzliche Ausführungs- und Cleanup-Semantik, nicht nur neue Assertions.
7. **Die Testanalyse ist eine gute Grundlage, aber noch kein sicherer Selektor.** Definitions-Hashes sind unbekannt; neue/unbeobachtete Änderungen führen konservativ zu breitem Scope. `can_skip_other_tests` ist bewusst false.
8. **Build-/Quellbindung ist ein wesentlicher Aufwandstreiber.** Das neue Modell darf diesen Aufwand nicht einfach aus dem Vergleich ausblenden. Entwicklerausführung und beweiskräftige Collector-Ausführung müssen getrennt sichtbar sein.

## 3. Modulmatrix

Aufwand ist relativ: klein = begrenzte Anpassung; mittel = mehrere gekoppelte Komponenten; groß = neue öffentliche Schnittstelle plus Konformität; sehr groß = Forschung bzw. semantisch schwierige Migration. Das sind keine Zeit- oder Tokenzusagen.

| Bereich / konkrete Quellfläche | Zieländerung | Aufwand / Abnahme |
| --- | --- | --- |
| `dctx0-language`: Lexer, Parser, Composition, Resolution, Typprüfung | Behalten; kein DSL-Rewrite für den Pilot | Klein; bestehende Sprachgoldens unverändert |
| `dctx0-core`: AST/IR, Values, Verifikationsmodelle | Separate Modelle für Testplan, Registrierung und Receipts; bestehende Values weiterverwenden | Mittel; strikte versionierte Serialisierung |
| `core/bindings.rs` | Native Bindings als Legacy-Pfad erhalten; neues Modell beschreibt Test-/Pflichtzuordnung | Mittel; keine Factory-Pflicht im neuen Backend |
| `dctx0-interpreter` | Zentraler Bewerter bleibt semantische Referenz | Mittel; vollständige beobachtete Kontexte und Continuations |
| `verification/generation.rs`, `generation/strings.rs`, `operation_arguments.rs` | Zentralen deterministischen Korpus weiterverwenden | Mittel; Seeds, ungültige Rohdaten und Budgetgrenzen identisch |
| `verification/projection`, `manifest.rs`, `bindings.rs` | Ausführungsneutrale Exportfläche über vorhandenen Plan | Mittel; keine verlorenen Closure-/Case-/Reviewpflichten |
| `verification/operation*.rs`, `resource_effects.rs`, `entity_identity.rs` | Bewertung als servicefähige reine API bereitstellen | Groß; Vorher/Nachher, vollständige Schlüssel und Ressourcenbelege |
| `verification/scenario.rs`, `lifecycle.rs`, `preconditions.rs` | Typisierte lokale Testfortsetzungen und Setup-Datenfluss anbinden | Groß; Szenarioreihenfolge, dynamische Calls und Reset |
| `verification/shrinking.rs`, `comparison.rs`, `diagnostics.rs` | Reale Test-Replays und differenzierte neue Evidenzquelle | Mittel; kein reines Label-Replay |
| `verification/attestations.rs`, `program_inspection.rs`, `suites.rs` | Automatik und quellgebundene Inspektion getrennt erhalten | Mittel; Review darf FAIL/fehlende Fälle nicht überstimmen |
| `dctx0-graph` | Bestehenden semantischen Graph behalten; Identitätsbezüge für Testanalyse exportieren | Klein; semantische Zyklen nicht als Test-Reihenfolge missverstehen |
| `dctx0-changes` | Source-/Semantik-Facets und explizite Renames weiterverwenden | Mittel; Änderungen an Tests/Fixtures zusätzlich abbilden |
| `dctx0-target-sdk` | Versioniertes Bewertungs-/Receipt-Protokoll ergänzen, Legacy-Transport erhalten | Groß; kein verdeckter SDK-2-Bruch |
| `targets/typescript` | Testbibliothek für Bun, Context/Async/Cleanup, Codecs und Registrierung | Groß; bestehendes Harness ist kein fertiger Ausgangszustand |
| `targets/go` | Testbibliothek über testing.T/Subtests/Cleanup, Collector-Anbindung | Groß; Parallelität, Panic und Testprozess-Abbruch |
| `targets/rust` | Test-/Dev-Bibliothek und libtest-Integration | Groß; Thread-/Prozesskontext, Features, Panics |
| `targets/java` | Profile bleiben verfügbar; Library-Ausführung separat planen | Groß bei Umsetzung; kein vorgetäuschter gleichzeitiger Support |
| Target-`inspection.rs`, SDK-`inspection`/`dependencies` | Strukturelle Inventare wiederverwenden; semantische Genauigkeit explizit begrenzen | Groß für neue Compileranalyse; Tree-sitter allein ist kein Typbeweis |
| `engine/verification_setup.rs`, `verification_discovery.rs`, `codecs.rs` | Explizite Backendwahl, Testregistrierung und deterministische Artefaktpublikation | Mittel; authored Tests erhalten, keine stillen Überschreibungen |
| `engine/native/*`, `verification_run/execution.rs` | Alten Worker behalten; neuen Runner-Collector/Bewerter daneben aufbauen | Groß; Streams, Timeout, Abbruch, Kindprozesse, Cleanup |
| `verification_run/build.rs`, `rust_build.rs`, `capture.rs`, `dependencies.rs` | Reale Test-Builds und sämtliche relevanten Eingaben binden | Groß; externer Host, generierter Code, Dirty Trees, native Binaries |
| `verification_run/replay.rs`, `shrink.rs` | Exakten Test/Unterfall mit gespeicherten Inputs erneut starten | Groß; Fixture-Reset und unterstützte Gruppen |
| `verification_run/pins.rs`, `adoption.rs`, `adoption_preview.rs`, `verification_complete.rs` | Neue Evidenzart bewacht prüfen; bestehende Guard-Semantik erhalten | Groß; vollständige Pflichtzuordnung und Konkurrenzkonflikte |
| `verification_run/status.rs`, `preflight.rs` | Registrierte/fehlende Prüfungen und Backendfähigkeit früh anzeigen | Mittel; statische Registrierung beweist keine Live-Fixture |
| `verification_run/task_reviews.rs`, `review_changes.rs`, `program_inspection.rs` | Neue Test-/Helper-/Bewerterquellen in Reviewbindung aufnehmen | Mittel; keine automatische Review-Erneuerung |
| `dctx0-tasks` | Task-DAG, Claims, Packages, Deltas, Done/PASS-Trennung behalten | Mittel; Testbibliothek verwaltet keine Tasks |
| `store/verification/*`, `store/tasks/native_proof.rs` | Receipts und Beobachtungen sicher speichern und erneut prüfen | Groß; atomare Veröffentlichung, Retention, alte Leser |
| `store/query.rs`, `store/site.rs`, Task-Readmodelle | Neue Evidenzbasis und aktuelle/alte/unbekannte Bindung anzeigen | Mittel; bestehende Abfrageverträge versionieren |
| `dctx0-test-analysis` | Fixture-/Build-/Spec-Kanten, Definitionen und Erklärungspfade ergänzen | Groß; vollständige Vergleichsreferenzen |
| `engine/experimental_coverage.rs`, `coverage_affected.rs`, `coverage_collect.py`, `analysis_adapter.py` | Bibliothekskataloge integrieren; sprachspezifische Coverage-Collector ergänzen | Sehr groß für alle Sprachen; keine Zuschreibung fremder Kindprozess-Coverage |
| `store/test_analysis.rs`, `experimental_coverage.rs` | Additive Versionierung neuer Kanten-/Identitätsprovenienz | Mittel; alte Daten bleiben unvollständig statt aufgewertet |
| `dctx0-cli` | Backendwahl und Collector-/Assessment-Ausgaben in bestehende Befehle integrieren | Mittel; finale Syntax erst nach API-Prototyp |
| `dctx0-lsp`, `editors/` | Verknüpfung von Pflicht, Test und Diagnose; Sprachfunktionen bleiben | Klein bis mittel; keine Voraussetzung des Pilots |
| `ui/`, Site-Engine | Ergebnisse und Grenzen neuer Evidenz darstellen | Mittel; kein alleiniger grüner Testzähler als Abnahme |
| `skills/`, Profile, AGENTS, README und Site-Dokumentation | Neuen Ablauf erklären, frühere normative Regeln gezielt migrieren | Mittel; historische Nachweise bewahren |
| `scripts/`, CI, Release und Installation | Bibliotheken paketieren/pinnen, reale native Matrix und Messungen ergänzen | Groß; CLI-Release allein liefert noch keine verwendbare Library |
| `dctx0/` Selbstspezifikation und Self-Verification | Neue öffentlich zugesagte Fähigkeiten passend spezifizieren und prüfen | Groß; keine sich selbst bestätigende PASS-Schleife |
| Cinema, Frameworkbeispiele, Tutorials | Vollständige Referenzmigration mit negativen Gegenproben | Groß; keine Wiederherstellung entfernter runatron-next-Fixtures |

## 4. Konkrete Schnittstellenarbeit vor der ersten Migration

### 4.0 Backendzuordnung und Mischbetrieb

Bestehende Konfigurationen wählen weiterhin den bisherigen Adapterpfad. Neue Anbindungen verwenden das neue Modell nach Abnahme der benötigten Targetfähigkeiten. Die Auswahl wird pro Prüfbereich gespeichert, nicht nur global pro Projekt. Das Datenmodell ordnet jede Planpflicht eindeutig einem Backend und einer Ausführungsgruppe zu; zusammenhängende Szenarien und Lifecycle-Pfade dürfen dabei nicht ihre gemeinsame Ausführung verlieren. Die konkrete Konfigurationssyntax wird im Prototyp festgelegt.

Der Pilot prüft auch einen gemischten Projektumfang: ein bestehender Adapterbereich bleibt unverändert angebunden, ein neuer Bereich nutzt direkte Anwendungstests. Beide werden gegen denselben aktuellen Pflichtplan bewertet. Fehlende, widersprüchliche oder überlappende Zuordnungen, inkompatible Quellen und Versionen sowie ungesicherte Ressourcenisolation blockieren die Gesamtabnahme. Vergleichsläufe behalten eine explizit gewählte maßgebliche Evidenz; ein doppeltes grünes Ergebnis ersetzt diese Zuordnung nicht. P1–P2 zeigen die Ausführung zunächst diagnostisch, P3 implementiert und prüft die bewachte Zusammenführung.

Die bisherige Schnittstelle wird eingefroren und erhält Kompatibilitäts-, Sicherheits- und Fehlerkorrekturen. Neue Integrationsfähigkeiten werden im neuen Modell umgesetzt. Es gibt keine automatische Projektmigration, keinen festgelegten Abschalttermin und keine vollständige Umschreibpflicht. Geänderte Anforderungen und Quellen erfordern auch im alten Backend neue gültige Nachweise und gegebenenfalls angepasste Adapter.

### 4.1 Ausführungsneutraler Plan

Aus `ProgramPlan` und `case_manifest` wird ein kleiner dokumentierter Export abgeleitet. Er trägt alle benötigten Eingaben, Typen, Erwartungen, Pflichten und Versionen. Der Export darf keine zweite unabhängige Projektion mit abweichenden Fallzahlen werden. Unbekannte zukünftige Versionen werden abgelehnt.

### 4.2 Registrierungsmodell und Collector

Ein Katalog beschreibt Tests, Varianten, Quellen, Pflichtzuordnungen und Fixture-Gruppen. Der Collector verifiziert die geplante gegenüber der tatsächlichen Ausführung. Sprachspezifische Unterschiede bleiben sichtbar: Bun-Registrierung, Go-Subtests und Rust-libtest sind keine identischen APIs. Der Prototype muss das tatsächliche Enumerieren und Ergebnisabgleichen zeigen; eine Bibliotheksliste alleine erkennt fehlende/nicht geladene Testdateien nicht zuverlässig.

### 4.3 Bewertungsprotokoll

Das neue Protokoll beschreibt `begin`, Beobachtung/Assessment und Abschluss als versionierte Nachrichten mit Größen-/Zeitlimits und Korrelations-IDs. Es darf keine Task-Mutationen oder beliebigen Produktionsaufrufe enthalten. Das ist eine neue schmale technische Schnittstelle; sie darf nicht als völlig adapterfreie Lösung vermarktet werden.

Alle Werte müssen exakt übertragbar bleiben, insbesondere große Integer, Decimal/Float, Unicode, fehlend/null, Varianten und deklarierte Fehler. Eine neue Rohdatenform muss invalides JSON samt Bytes erhalten können, statt es schon bei der Übertragung zu validieren und zu verlieren. Die Verantwortung für Semantik bleibt beim zentralen Rust-Bewerter.

### 4.4 Vollständigkeit und Autorität

Der native Runner liefert Testausgänge; der Bewerter liefert Vertragsurteile; der Collector stellt Katalog-/Laufvollständigkeit und Prozessbindung fest. Der Store prüft die Zusammengehörigkeit und Pflichtabdeckung erneut. Keine dieser Rollen darf durch einen ungeprüften `status: PASS`-Import ersetzt werden. Konfligierende Receipts, unerwartete IDs und abweichende Inhalte unter gleicher ID werden Fehler.

Task-Pins bleiben zunächst vor dem Lauf gebunden. Eine spätere Entkopplung von Task-Administration und wiederverwendbarer Ausführungsevidenz ist sinnvoll, aber eine separate neue Übernahmeregel mit aktuellem Anforderungs-/Quellvergleich. Sie darf nicht im selben Schritt die bestehenden Guards aushebeln.

### 4.5 Verbindliche Testformen und KI-Autorenschaft

Die nachträgliche Konzeptpräzisierung in Abschnitten 5.1/5.2 ist Teil des Pilots: zuerst `decoderTest`, `operationTest` und `stateChangeTest`, ergänzt um erforderliche Call-Beobachtung. Der Plan bestimmt die Pflichten; generierte Registrierung und die Library erzwingen deren Ablauf. Die KI implementiert nur die typisierten Anwendungsanschlüsse. Freie Assertion- oder Receipt-Wrapper allein erfüllen dieses Ziel nicht.

P0 inventarisiert vorhandene Fixtures und bestimmt, welche Anschlüsse tatsächlich handgeschrieben bleiben müssen. P1 prüft Signaturen, deterministische Generierung, fehlende Registrierung und Änderungen erzeugter Dateien. P2 enthält gezielte Produktions- und Bindungsgegenproben, insbesondere ausgelassenen Write, falschen State-Scope, nicht injizierten Trace-Decorator und konstantes Ersatzresultat. Erwartet wird die Erkennung der jeweils prüfbaren Gegenprobe; eine generelle Erkennung beliebiger täuschender Tests wird nicht zugesagt.

Die Diagnoseoberfläche liefert strukturierte minimale Reparaturkontexte statt zusätzlicher Agenten-Orchestrierung. Im Kostenvergleich werden handgeschriebene Anschlusszeilen und die Zahl bei einer typischen Änderung anzupassender Stellen gesondert gemessen. Gemeinsame Fixtures senken Duplikation, erhöhen aber ihren Änderungsradius; dieser gehört ausdrücklich in P6.

## 5. Migration in lieferbaren Paketen

| Paket | Konkretes Ergebnis | Abnahme / Stop-Kriterium |
| --- | --- | --- |
| P0: gemeinsamer Plan und Pilotbaseline | Identischer Pflichtplan für eine Datenstruktur, eine Policy und eine zustandsändernde Operation | Alle Fälle/Budgets/Erwartungen inventarisiert; Messmethode festgelegt |
| P1: erste echte Go-Testintegration | Ein vorhandener Anwendungstest ruft Produktion direkt auf und erhält zentrale Urteile | Kein universeller Worker-Dispatcher im neuen Pfad; gültige/ungültige Rohdaten, Fachfehler und technische Fehler unterschieden |
| P2: Zustand und Fortsetzungen | Echte Persistenz, Calls-null, Schlüssel und ein verbundener Lifecycle | Gleichwertige Pflichtbeobachtung; fehlende Beobachtung bleibt UNKNOWN; schwierige Call-Fortsetzung demonstriert |
| P3: Collector, Store und Abschluss | Kontrollierter Testlauf mit Quellbindung, kompletter Bewertung, Replay und bewachter Übernahme einschließlich gemischter Backends | Fehlender/übersprungener Test, Crash, Duplicate, veraltete Quelle und Cleanup-Fehler sowie Lücken, Konflikte und inkompatible Evidenz im Mischbetrieb werden korrekt blockiert |
| P4: Bun/Rust und vollständige Referenz | Gemeinsame Konformität und vollständige betroffene Cinema-Abnahme | Gleiche Semantik und Pflichtinventare; reale native Plattformen, Mutationen und Reviews |
| P5: reale Verbrauchermigration | Ein vollständiges runatron-business-Paket nutzt vorhandene Fixtures und ersetzt seine Adapteranbindung | Alter/neuer Weg vergleichbar; alle Paketnachweise und nötigen Projektgates auf gleichem finalem Stand |
| P6: bessere Testanalyse | Test-/Fixture-/Build-/Spec-Kanten mit Vorhersage und vollständiger Referenz | Übersehene Fehler, Fallbacks und End-to-End-Kosten sichtbar; zunächst weiterhin keine automatische Auslassung |
| P7: freiwilliger bereichsweiser Rückbau | Nur im migrierten Projektbereich nicht mehr benötigte Dispatcher/Factories/Observer-Router entfernt | Neue Evidenz deckt alten Pflichtumfang; bisheriges Backend, Legacy-Leser und nicht migrierte Bereiche desselben Projekts funktionieren weiterhin |

P1 startet mit Go, weil dort der reale Consumer-Schmerz liegt. Rusts bestehender Experiment-Collector wird für P6 wiederverwendet; er ist keine Voraussetzung, um die erste Testbibliothek nutzbar zu machen. P1–P3 dürfen zunächst nur diagnostische Evidenz liefern, bis die vollständige Übernahmeprüfung implementiert ist.

P4 und P5 können nach stabiler Schnittstelle unterschiedliche Arbeitspakete bilden. Eine Consumer-Migration vor vollständiger Plattformunterstützung bleibt auf den ausdrücklich belegten Scope beschränkt. Keine Fremdplattform wird aus einem Linux-PASS abgeleitet.

Ein Paket endet mit einem ausführbaren Ergebnis und gemessener Abnahme. Nicht nach jedem Helper die gesamte native Closure laufen lassen. Früh eine reale kleine vollständige Strecke etablieren; breite Checks nach integriertem Paket. Unveränderte erfolgreiche Checks werden nur aus einem benannten Grund wiederholt.

## 6. Verifikation des Umbaus

Die Vergleichsmatrix muss mindestens folgende Fälle enthalten:

- identische gültige und ungültige Werte einschließlich Unicode, Integergrenzen, fehlend/null und Variantentags;
- Parser akzeptiert ungültige Rohdaten, Serializer erzeugt falsche Feldnamen, Konvertierung scheitert vor Produktionsaufruf;
- echte Operation liefert falschen Wert oder falschen deklarierten Fehler;
- Zustand fehlt, Vorher-/Nachherphase vertauscht, Entity-Schlüssel unvollständig, zusätzlicher unerlaubter Schreibeffekt;
- leerer tatsächlicher Call-Trace versus fehlender Trace; Reihenfolge und Calls in Setup versus Operation;
- Lifecycle mit abhängigen Rückgabewerten, nested pure calls, unpassender Fixture und nicht unterstützter Klausel;
- Test wird ausgelassen, vorzeitig beendet, zweimal registriert, mit falscher Fall-ID ausgeführt oder fängt einen Fehler ab;
- erforderliche Testform fehlt, tatsächliche Inputs werden ausgetauscht, erzeugte Falliteration wird editiert oder ein nicht ausgeführter Decorator meldet scheinbar null Aufrufe;
- bestehender Adapter und neuer Anwendungstest laufen im selben Projekt; Pflichtzuordnungen fehlen oder überlappen, Quellen/Versionen weichen ab oder ein Lifecycle wird unzulässig zwischen Backends aufgeteilt;
- ein Update erhält alte Konfigurationen und Adapter; eine fachliche Änderung im alten Prüfbereich verlangt neue Evidenz, während ein neuer Bereich das neue Backend nutzt;
- Fixture-Aufbau scheitert teilweise, Cleanup scheitert, Kindprozess bleibt aktiv, Worker/Bewerter stürzt ab;
- Race, gemeinsame Resource und Hintergrundarbeit verursachen falsche Testzuordnung;
- gefälschter oder veralteter Ergebnisimport, geänderter Helper, geänderte Bibliothek oder ungepinnter externer Host;
- Replay reproduziert den Fehler, Reparatur wird durch neue Ausführung belegt, Shrinking erhält gültige Setup-Voraussetzungen;
- Taskplan ändert sich nach dem Lauf; bestehende Claims oder Reviewpflichten verhindern ungerechtfertigte Übernahme;
- Store-Migration, Export/Import, Retention und Pruning erhalten Historie und melden fehlende Details ehrlich;
- Testauswahl bei neuer Verzweigung, gelöschtem Symbol, Shared Fixture, Migration, Makro/Build-Tag und Kindprozesscode.

Gegenproben verändern sowohl Produktion als auch Testanbindung. Ein mutierter Test, der nur eine Konstante meldet, kann technisch korrekt geformte Beobachtungen fälschen: Hier muss die Grenze des Vertrauensmodells sichtbar bleiben. Es gibt keinen kryptografischen Nachweis, dass beliebiger Testcode ehrlich misst.

Die bestehenden 97%-Rust-Gates, native Plattformpflichten, Cinema-Szenarien und quellgebundenen Reviews werden durch diesen Entwurf nicht reduziert. Bibliothekscode in Go/TypeScript wird separat gemessen; Rust-Coverage deckt ihn nicht ab. Tests/Validierungen im künftigen Umbau sind mit den vorgeschriebenen separaten Release-Speed-/RAM-Messungen zu koppeln.

## 7. Kostenvergleich und Abbruchkriterien

Vor P1 tatsächliche Baseline erfassen: Buildzeit kalt/warm, native Laufzeit, Peak-RAM nach benannter Methode, Receipt-/Artefaktvolumen, handgeschriebene Bindungszeilen, Fixture-Code und eine typische Änderung mit ihrem Reparaturumfang. Gleicher Scope, gleiche Ressourcen, unveränderte Seeds/Budgets. Eine breitere instrumentierte Sammlung wird nicht mit einem engeren uninstrumentierten Lauf verglichen.

Die neue Library ist dann aussichtsreich, wenn P2 die semantisch schwierigen Fälle mit nachvollziehbar weniger projektspezifischer Vermittlung abdeckt. Wenn sie wieder Subject-Dispatcher, eigene Factoryhierarchien und umfassende State-Router verlangt, muss der Schnitt geändert werden. Wenn zentrale Bewertungskosten dominieren, zuerst Batchgröße/Transport untersuchen, bevor drei Interpreter neu implementiert werden.

Testselektionskosten umfassen Inventarisierung, Build, Instrumentierung, Capture, Graphpflege und Fallback-Ausführungen. Die Summe ausgesparter Testzeiten allein ist kein Geschwindigkeitsgewinn. Shadow-Auswertung braucht echte Fehlerfälle; zusätzliche Mutationen sind nützlich, ersetzen aber keine unabhängigen realen Änderungen.

Numerische Erfolgsgrenzen werden vor dem Pilot mit gemessener Baseline vereinbart. Keine erfundene Prozentersparnis oder feste Fertigstellungsdauer für den Gesamtumbau.

## 8. Kompatibilität, Dokumentation und Rückbau

- Bestehende Run-IDs, Snapshot-Hashes und historische Urteile bleiben unverändert. Neue Evidenz erhält explizite Basis und Formatversion.
- Sprache, SDK, Transport, Konfiguration, Store und Query-Views haben getrennte Versionen. Vor Umsetzung eine genaue Leser-/Schreibermatrix festlegen.
- Bestehende Adapter bleiben als unterstütztes Backend ohne festgelegten Abschalttermin verfügbar. Ihre Schnittstelle ist eingefroren; Kompatibilitäts-, Sicherheits- und Fehlerkorrekturen werden weitergeführt. Neue Fähigkeiten entstehen im neuen Modell.
- Bestehende Projekte behalten ihre Backendzuordnung bei Updates. Neue Anbindungen folgen nach Capability-Abnahme dem neuen Standard; beide Wege dürfen innerhalb desselben Projekts nebeneinander bestehen. Bei fehlender neuer Unterstützung kein stiller Fallback mit irreführendem PASS.
- Unterschiedliche Backends und Ausführungsgruppen dürfen nur unter einer implementierten und geprüften Kompositionsregel zu einer Gesamtabnahme zusammengeführt werden. Jede Pflicht hat eine eindeutige Evidenzzuordnung zum aktuellen Plan; Quellbindung, Versionen, Isolation, Ressourcen und Reviews müssen zusammenpassen. Kein Vermischen zufällig grüner Läufe verschiedener Quellen.
- Retention muss aktive Referenzläufe, Vergleichspaare und noch benötigte Übernahme-/Replay-Evidenz schützen. Bereits geprunte Details werden nicht rekonstruiert oder erfunden.
- Generated Tests werden deterministisch und atomar publiziert. Handgeschriebene Tests/Fixtures bleiben erhalten. Lockfiles und Bibliotheksversionen sind Bestandteil der Bindung.
- CLI-Help, Skills, AGENTS, Site, Release-Installer und Beispielanwendungen werden am Capability-Wechsel konsistent angepasst. Die noch vorhandenen historischen Zukunftsaussagen brauchen einen klaren aktuellen Einstieg.
- P7 ist eine freiwillige Bereinigung bereits migrierter Projektbereiche. Die Abkündigung des bisherigen Backends ist nicht Teil dieses Plans und würde eine gesonderte Entscheidung erfordern. Eine Rückkehr zum bisherigen Pfad darf neue Historie nicht überschreiben.

## 9. Aus der Diskussion bewusst separat gehalten

Runner-Persistenz vereinheitlichen, Local-Garantien ändern, WSS durch Queue/Wakeup ergänzen, Runtime-Typen begrenzen, Produktcode generieren und allgemeine Coverage-Pflichten ändern sind eigenständige Produkt-/Architekturentscheidungen. Sie werden nicht in dieses Testbibliotheksprojekt hineingezogen. Sonst wäre die Kostenwirkung der neuen Integration nicht mehr überprüfbar.

Ebenso bleiben ein vollständiges neues Testframework, universelle Compilerinstrumentierung, automatisches Test-Skipping und globale PASS-Caches außerhalb des ersten Liefergegenstands. Die Abhängigkeitsdaten werden von Beginn an passend modelliert; ihre vollständige automatische Nutzung ist ein späteres, separat nachgewiesenes Ergebnis.

## 10. Ergebnis der Prüfung

Der Umbau ist architektonisch plausibel, weil Plan, Interpreter und Bewertungsfunktionen bereits vom nativen Aufruf getrennt sind. Der leichte Teil ist die Testregistrierung. Die großen Arbeiten sind eine saubere Beobachtungs-/Fortsetzungsschnittstelle, kontrollierte Collector-Ausführung, native Bibliothekskonformität und die unverändert strenge Evidenzübernahme.

Das bestehende Experiment kann weiterverwendet werden; die Testbibliothek liefert fehlenden Kontext, ersetzt aber keine Codeanalyse. Der sinnvolle nächste Implementierungsauftrag ist P0–P2 als zusammenhängender Pilot mit einer echten zustandsändernden Operation. Dessen Ergebnis entscheidet über P3–P7. Dieses Dokument selbst behauptet weder implementierte Gleichwertigkeit noch aktuelle Gesamt-PASS.

## 11. Direkte Prüfreferenzen

Die folgenden Einstiegspunkte tragen die entscheidenden Befunde; die Modulmatrix nennt die übrigen betroffenen Bereiche:

- [Bindungsmodelle mit Session, Invoke und Projektionen](../crates/dctx0-core/src/bindings.rs)
- [Exakte Werte und bestehende Evidenzmodelle](../crates/dctx0-core/src/verification.rs)
- [Zentrales Pflichtmanifest](../crates/dctx0-verification/src/manifest.rs)
- [Bewertung von Operationen und ausstehenden Aufrufen](../crates/dctx0-verification/src/operation.rs)
- [Bewertung tatsächlicher Zustandseffekte](../crates/dctx0-verification/src/operation_state.rs)
- [Endliche Entity-Identitätsprüfung](../crates/dctx0-verification/src/entity_identity.rs)
- [Aktuelle native Ausführung und Abbruch](../crates/dctx0-engine/src/verification_run/execution.rs)
- [Quellcapture](../crates/dctx0-engine/src/verification_run/capture.rs)
- [Task-Pins](../crates/dctx0-engine/src/verification_run/pins.rs)
- [Erneute Evidenzprüfung bei Übernahme](../crates/dctx0-store/src/tasks/native_proof.rs)
- [Aktuelle Go-Testscaffolds](../targets/go/src/scaffolds.rs)
- [Aktueller TypeScript-Testwrapper](../targets/typescript/harness/testing.ts)
- [Pure Testanalyse und konservative Auswahl](../crates/dctx0-test-analysis/src/lib.rs)
- [Aktueller Rust-Coverage-Collector](../crates/dctx0-engine/src/coverage_collect.py)
- [Grenzen und Methoden des Testanalyseexperiments](site/src/reference/test-analysis.md)
- [Aktuelle Generatorfähigkeiten](site/src/reference/native-test-generators.md)
- [Retention und historische Evidenz](site/src/reference/verification-storage.md)
