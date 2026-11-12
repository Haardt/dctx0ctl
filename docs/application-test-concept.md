# Anwendungstests als Ausführungsgrenze für dctx0

Stand: 2026-11-08. Status: **beschlossene Architekturrichtung für neue Anbindungen mit kompatiblem Mischbetrieb**. Die neue Ausführung ist noch nicht implementiert oder als Ersatz der geltenden Abnahme freigegeben. Ausgangspunkt ist die Diskussion und die Untersuchung von runatron-business und dctx0ctl. Der konkrete Umbau und die geprüften Quellstellen stehen in [Umbauprüfung](application-test-migration.md).

## 1. Entscheidung und Ziel

dctx0 behält die Autorität über Spezifikation, deterministische Pflichtfälle und deren fachliche Bewertung. Normale Anwendungstests übernehmen Produktionsaufrufe, Fixture-Aufbau und Ressourcenverwaltung. Kleine Bibliotheken integrieren diese Tests mit dem Vertragsplan. Bun, Go test und Rusts Testinfrastruktur bleiben die eigentlichen Test-Runner.

Das Ziel ist weniger separat gepflegter Integrationscode bei explizit vergleichbarer Nachweisstärke. Weder eine Zeilenersparnis noch ein Geschwindigkeitsgewinn ist bereits gemessen. Vollständigkeit bedeutet weiterhin vollständige Erfüllung des deklarierten endlichen Plans; sie ist kein universeller Korrektheitsbeweis.

Die wesentliche Änderung ist die Aufrufrichtung:

```text
Heute: dctx0 → Worker → universeller Adapter → Anwendung
Ziel:  Anwendungstest → Anwendung
       Anwendungstest → Testbibliothek → zentrale Vertragsbewertung
```

Die aktuelle Scaffold-Funktion realisiert das Ziel noch nicht. Sie ruft aus einem Test `verify bindings test` auf und startet damit wieder die zentrale Adapterausführung. Diese Verschachtelung muss tatsächlich ersetzt werden.

### 1.1 Neuer Standard und kompatibler Mischbetrieb

Anwendungstests mit zentraler Vertragsbewertung sind der Standard für neue Anbindungen, sobald das jeweilige Target und die benötigten Fähigkeiten implementiert und abgenommen sind. Bis dahin bleibt die bisherige Ausführung verfügbar; die Architekturentscheidung allein aktiviert kein neues Backend.

Bestehende Adapteranbindungen werden in ihrer Schnittstelle eingefroren und als unterstütztes Backend weitergeführt. Sie erhalten Kompatibilitäts-, Sicherheits- und Fehlerkorrekturen; neue Integrationsfähigkeiten entstehen im neuen Modell. Ein Update erhält bestehende Konfigurationen, Adapter und Tests ohne automatische Migration. Es gibt keinen festgelegten Abschalttermin und keine Pflicht zur vollständigen Neufassung eines Projekts.

Beide Backends dürfen im selben Projekt eingesetzt werden. Bestehende Prüfbereiche bleiben am bisherigen Adapter, neue Bereiche werden nach dem neuen Standard angebunden. Eine freiwillige Migration kann bereichsweise erfolgen. Ein Prüfbereich ist eine explizit abgegrenzte Menge von Planpflichten; die Zuordnung muss auch zusammenhängende Szenarien und Lifecycle-Pfade berücksichtigen.

Für jede Pflicht ist eindeutig gespeichert, welches Backend und welche Ausführungsgruppe ihren Nachweis liefern. Vergleichsläufe dürfen dieselbe Pflicht auf beiden Wegen prüfen, ersetzen aber keine explizite Auswahl der maßgeblichen Evidenz. Überlappende oder fehlende Zuordnungen dürfen weder stillschweigend dedupliziert noch durch einen automatischen Backendwechsel verdeckt werden. Ein zusammenhängender Szenario- oder Lifecycle-Nachweis bleibt in einer dafür geeigneten Ausführungsgruppe; ein beliebiges Aufteilen seiner Schritte zwischen Backends ist kein vollständiger Nachweis.

Beide Wege verwenden denselben aktuellen zentralen Pflichtplan und Bewerter. Der Collector und die Übernahmeprüfung führen ihre Evidenz nur bei kompatibler Plan-, Anforderungs-, Quell- und Versionsbindung zusammen. Dazu müssen auch backendübergreifende Pflichtabdeckung, Isolation, Ressourcen und erforderliche Reviews geprüft werden. Die neue Kompositionsregel muss implementiert und abgenommen sein, bevor gemischte Ergebnisse eine Gesamtabnahme autorisieren.

Eingefroren wird die alte Anbindungsschnittstelle, nicht der fachliche Anforderungsstand. Ändert sich eine Anforderung oder relevante Implementierung, muss auch ein bestehender Adapter angepasst werden, soweit nötig, und neue gültige Evidenz liefern. Ein historisches PASS bleibt an seine damaligen Anforderungen und Quellen gebunden. Benötigt ein Bereich eine nur im neuen Modell unterstützte Fähigkeit, wird diese Grenze ausdrücklich angezeigt und der betroffene Bereich gezielt migriert; die übrigen Adapter können bestehen bleiben.

## 2. Korrekturen und Präzisierungen aus der Diskussion

- Ein Assert-Wrapper alleine genügt nicht. Er kennt weder den vollständigen Pflichtumfang noch die tatsächlichen Produktionsabhängigkeiten.
- Eine Factory verschwindet nur, wenn sie ausschließlich für den universellen Adapter existiert. Die Anwendung braucht weiterhin Testaufbau und reale Ressourcen.
- Zustandsbeobachtung bleibt notwendig. Sie wird lokal im Test ausgedrückt; fachliche Projektionen lassen sich nicht immer generieren.
- Ein grüner Test ist keine automatische Erfüllung einer Anforderung. Registrieren, ausführen, beobachten, zentral bewerten und vollständig abschließen sind getrennte Schritte.
- Eine Bibliothek erkennt nicht automatisch beliebige Datei-, Netzwerk-, Prozess- oder globale Zustandszugriffe. Erfassung hat einen ausgewiesenen Geltungsbereich.
- Die Produktionsanwendung braucht keine Testbibliothek. TypeScript importiert sie ausschließlich in Tests, Rust über Test-/Dev-Abhängigkeiten, Go ausschließlich aus Testdateien. Go-Modulmetadaten können die Abhängigkeit trotzdem enthalten; das produktive Binary muss sie nicht enthalten.
- Produktionsvalidatoren oder erzeugte Produktionstypen wären eine eigenständige optionale Entscheidung. Ihre Einführung ist keine Voraussetzung dieser Migration.
- Die frühere Idee, beliebige native Assertions als vollständigen Ersatz der DSL-Auswertung zu nutzen, würde Semantik duplizieren und Nachweise schwächen. Dieser Vorschlag behält deshalb den zentralen Interpreter als maßgeblichen Bewerter.

## 3. Zuständigkeiten

| Bestandteil | Verantwortung |
| --- | --- |
| Compiler/IR | Bestehende Sprache, Typen, Ausdrücke, Quellpositionen und semantische Beziehungen |
| Planer/Generator | Pflichtmanifest, Eingabekorpora, Seeds, Budgets, Szenario- und Lifecycle-Pfade |
| Testbibliothek je Sprache | Registrierung, Falliteration, Beobachtungsaufnahme, Prüfungskommunikation, Finalisierung |
| Anwendungstest | Direkter Produktionsaufruf, vorhandene Fixtures, tatsächliche Zustände und Effekte |
| Zentraler Bewerter | Auswertung gegen den gepinnten Plan; PASS/FAIL/UNKNOWN und offene Pflichten |
| Engine/Collector | Start und Überwachung nativer Test-Runner, Quell-/Buildbindung, Ergebnisaufnahme |
| Testanalyse | Änderungsauswirkungen und begründete Testauswahl mit expliziten Unbekannten |
| Store/Taskverwaltung | Unveränderliche Evidenz, Historie, bewachte Übernahme und getrennte Reviews |

Die Bibliothek wird kein DI-Container, ORM, Container-Orchestrator, eigener Assertion-Interpreter oder universeller Service-Dispatcher. Vorhandene Projekt-Fixtures werden eingebunden. Optionale Ressourcenhelfer liefern Lebensdauer und Ownership, keine zweite Produktionsarchitektur.

## 4. Plan und Testregistrierung

Vor der Ausführung wird aus dem geprüften IR ein unveränderlicher Plan erzeugt. Er enthält mindestens Snapshot und Check-Occurrence, Vertrags-/Anforderungsidentitäten, Manifest-Hash, Generator-/Evaluatorversion, Seeds, Budgets, konkrete Fälle, Pflichtbeobachtungen und offene manuelle Pflichten.

Die Testregistrierung wird gegen diesen Plan abgeglichen. Ein Test kann mehrere Fälle und Pflichten abdecken; eine Pflicht kann mehrere Tests benötigen. Die Zuordnung ist viele-zu-viele. Die tatsächlich registrierten Tests und Unterfälle werden mit dem erwarteten Katalog verglichen. Ein Filter darf eine fokussierte Ausführung erzeugen, aber keine scheinbar vollständige Abnahme.

Eine Test-ID besteht aus Projekt, Target und explizitem lokalen Namen; parametrisierte Unterfälle erhalten eine planbezogene Fall-ID. Anzeigename und Quellzeile sind keine ausreichende Identität. Umbenennung ist zunächst Entfernen/Hinzufügen, außer es existiert eine explizite Zuordnung. Gleiche Namen nach geänderter Definition erhalten eine neue Versionsbindung.

Setup-Fehler, vorzeitige Returns, übersprungene Unterfälle, abgefangene Assertion-Fehler und fehlendes Cleanup müssen in der Finalisierung sichtbar sein. Unbekannte und doppelt eingereichte Fallidentitäten werden abgelehnt. Mehrere legitime Prüfungen desselben Falls erhalten eigene Check-IDs; sie dürfen nicht durch Mengenbildung stillschweigend verschwinden.

Fehlgeschlagene Vertragsprüfungen bleiben im Laufzustand dauerhaft fehlgeschlagen, auch wenn der Test ihre Exception abfängt. Die tatsächlichen Inputs werden gegen die geplanten Inputs verglichen; eine richtige Fall-ID mit ausgetauschten Werten erfüllt den Fall nicht. Normale Assertions außerhalb der Bibliothek kann diese hingegen nicht vollständig beobachten; deren Ergebnis muss über den nativen Runner kommen. Absichtlich abgefangene fremde Assertions bleiben Teil des Testcode-Vertrauensproblems.

## 5. Bibliotheksoberfläche

Die folgenden Namen sind Entwurf, keine vorhandenen APIs oder installierbaren Pakete:

| Oberfläche | Bedeutung |
| --- | --- |
| `contractTest` / targettypische Entsprechung | Identität, Planpflichten, Unterfälle, Fixture-Bezüge und Abschluss |
| `cases` | Zentrale Eingaben durchlaufen, ohne Fälle selbst zu erfinden oder zu filtern |
| `assertContract` | Tatsächlichen Wert strukturell und bezüglich ausführbarer Refinements prüfen |
| `recordOutcome` | Tatsächlicher Erfolg, deklarierter Fachfehler oder technische Ausnahme |
| `observeState` | Vorher-/Nachher-Zustand mit Schlüsseln, Umfang und Vollständigkeitsangabe |
| `observeCalls` / `observeEvents` | Tatsächliche Aufrufe/Ereignisse samt Reihenfolge und Beobachterverfügbarkeit |
| `fixture` | Bestehendes Setup/Cleanup mit Scope, Ownership und Ressourcenbezügen registrieren |

Normale `expect`-/`assert`-Aufrufe bleiben für zusätzliche Anwendungserwartungen zulässig. Ein grünes Zusatzassertion-Ergebnis ersetzt keine zentrale Vertragsprüfung. Ein fehlgeschlagener nativer Test verhindert den erfolgreichen Abschluss seiner Ausführungsgruppe, auch wenn zuvor alle Vertragsassertions erfolgreich waren.

In Go erfolgt die Zuordnung über expliziten Testkontext und Subtests; in TypeScript über weitergereichten asynchronen Kontext; in Rust über Test-/Session-Handles. Globale veränderliche Variablen wie „aktueller Test“ sind bei Parallelität ungeeignet. Kindprozesse benötigen eine explizite Zuordnung; Threads und asynchrone Aufgaben müssen vor Abschluss beendet oder als offene Ressourcen gemeldet werden.

### 5.1 Verbindliche Testformen statt frei zusammengesetzter Pflichtprüfungen

Für automatische Vertragsnachweise ist die bevorzugte öffentliche Oberfläche eine kleine Menge zusammensetzbarer Testformen. Der Plan legt anhand der modellierten Pflichten fest, welche Form und Beobachtungen erforderlich sind. Die KI verbindet deren typisierte Anschlüsse mit der Anwendung; die Bibliothek besitzt Falliteration, Reihenfolge, Bewertung und Abschluss. Die oben genannten Aufnahmefunktionen sind dafür überwiegend interne Bausteine. Ein frei formulierter Test mit beliebigen `recordOutcome`-Aufrufen erlangt dadurch keine gleichwertige Nachweisautorität.

| Testform (Entwurfsname) | Anwendung liefert | Bibliothek erzwingt |
| --- | --- | --- |
| `decoderTest` | Tatsächlichen Decoder und optional tatsächlichen Encoder | Unveränderte gültige/ungültige Rohdaten, tatsächliches Outcome, strukturelle Prüfung; zusätzliche Wire-Erwartungen nur aus definiertem Wire-Vertrag |
| `operationTest` | Direkten Produktionsaufruf und vorhandene Fixture | Vollständigen Korpus, tatsächliche Inputs/Outcomes und zentrale Klauselprüfung |
| `stateChangeTest` | Operation, Zustandsleser und expliziten betroffenen Scope | Vorzustand → Operation → Nachzustand, auch bei Fachfehlern; geforderte Effekte und Unverändertheit |
| `callTraceTest` | Tatsächlich injizierten beobachtbaren Port | Erfassung realer Weiterleitungen, Zuordnung zu Setup/Operation, vorhandenen leeren Trace und geforderte Reihenfolge/Anzahl |
| `resourceLifecycleTest` | Reale Provisionierungs-, Nutzungs-, Beobachtungs- und Cleanup-Funktionen | Ownership, geforderte Phasen, tatsächliche Endzustandsprüfung und Cleanup-Ergebnis |
| `scenarioTest` | Lokale typisierte Aufrufe und gemeinsame Fixture | Geplante Reihenfolge, Ergebnisbindungen, Fortsetzungen und vollständigen Lifecycle-Pfad |

Formen können kombiniert werden: Eine Operation kann Zustand, Calls und Ressourcen gleichzeitig betreffen. Fehlende Pflichtanschlüsse verhindern Registrierung oder Kompilierung; fehlende Ausführung verhindert Abschluss. Ein nicht modellierter Freitext wird nicht automatisch in eine neue Testpflicht übersetzt. Beispielsweise verlangt ein Entity-Key nicht ohne weitere Anforderung eine bestimmte SQL-Implementierung; Identitätsprüfung und physischer Persistenznachweis bleiben verschieden.

Bei technischem Abbruch kann der Nachzustand tatsächlich nicht mehr lesbar sein. Die Testform meldet dann fehlende Evidenz und versucht kontrolliertes Cleanup; sie darf keinen Zustand rekonstruieren und als beobachtet ausgeben. Ein Call-Decorator muss dem Service wirklich injiziert sein. Auch die beste Testform kann diese fachliche Anbindung nicht allein aus ihrer Registrierung beweisen.

### 5.2 Optimierung für KI-geschriebene Tests

**Deterministisch erzeugen, was keine fachliche Entscheidung braucht.** Pflichtzuordnungen, Fall-IDs, Falliteration und Finalisierung werden vom Compiler beziehungsweise der Bibliothek geliefert. Die KI schreibt im Normalfall nur direkte Aufrufe, Fixture-Verknüpfungen und notwendige Datenprojektionen. Keine handgepflegten Listen erwarteter Pflichtfälle, frei kopierten Sollwerte oder eigenen Bewertungsalgorithmen.

Erzeugte Artefakte und handgeschriebene Anschlüsse liegen getrennt. Vor Abnahme wird die deterministische Ausgabe mit dem gepinnten Plan verglichen. Eine lokale Änderung erzeugten Codes kann die zentrale Pflichtmenge nicht verändern. Native Signaturen werden soweit möglich beim Build geprüft; eine syntaktisch gefundene Funktion oder ein unsicherer Type-Cast ist noch kein Beweis korrekter Bindung.

**Soll und Ist haben unterschiedliche Herkunft.** Sollwerte kommen aus der geprüften Spezifikation und zentralen Auswertung. Istwerte kommen aus echten Produktionsaufrufen und Zustandslesern. Die KI darf weder den Sollwert aus dem Produktionsresultat ableiten noch einen kopierten Produktionsalgorithmus als unabhängiges Orakel einsetzen. Wenn dieselbe KI Spezifikation, Produktion und Test schreibt, bleiben gemeinsame fachliche Missverständnisse möglich; Spec-Review und unabhängig begründete Beispiele bleiben wesentlich.

**Wenige gemeinsame Anbindungen statt Wiederholung je Test.** Bestehende Anwendungs-Fixtures und konkrete Port-Decorators werden innerhalb einer fachlichen Testgruppe wiederverwendet. Ihre Definitionen und transitiven Abhängigkeiten gehören in den Änderungsgraph. Projektionen erhalten kleine gezielte Prüfungen gegen bekannte tatsächliche Records, sofern ihre Transformation den Nachweis beeinflusst. Keine allgemeine Adapterhierarchie oder manuell gepflegte Symbolregistry daraus bauen.

**Fehler melden den kleinsten reparierbaren Zusammenhang.** Die KI erhält Vertragsposition, Testform, fehlenden Anschluss oder konkrete Input-/Outcome-Differenz, zugehörige Quellstellen und einen reproduzierbaren Unterfall. Nach einer Änderung werden relevante Deltas und dieser Fall geladen. Das vollständige gepinnte Paket bleibt zugänglich; kompakte Diagnostik darf keine Pflicht verstecken. Die Library führt selbst keine LLM-Aufrufe aus.

**Gegenproben prüfen die Aussagekraft der Anbindung.** Beim erstmaligen Anschluss eines Bereichs und nach relevanten Bindungsänderungen werden gezielte bekannte Fehler eingebaut: Produktionsaufruf entfernen, Persistenzschreiben auslassen, falschen Schlüssel lesen oder verbotenen Aufruf hinzufügen. Der betroffene Nachweis muss scheitern. Mutationen werden isoliert ausgeführt, nicht durch Änderungen am aktiven Arbeitsbaum. Ein nicht erkannter Fehler löst eine Untersuchung aus; ein technisch nicht ausführbarer Mutant ist kein erkannter Fehler. Dies ist keine Forderung nach vollständiger Mutation jedes Helpers bei jedem Turn.

Ressourcen, Szenarien und Bindungen werden dort geprüft, wo ein konkretes Risiko besteht. Nicht jedes Detail bekommt eine neue allgemeine Regel oder zusätzliche Frameworkschicht. Erfolg des Designs ist ein kleinerer handgeschriebener Anschluss bei unverändertem Pflichtumfang, nicht eine größere Menge generierter Testdateien.

## 6. Zentrale Bewertung ohne universelle Produktionssteuerung

Empfohlener erster Backend-Entwurf: ein verwalteter Rust-Bewerter pro Testausführungsgruppe. Die Bibliotheken senden gebündelte Eingaben und Beobachtungen; der Bewerter nutzt `dctx0-interpreter` und `dctx0-verification`. Das Protokoll dient der Bewertung und erhält keine Fähigkeit, beliebige Produktionsmethoden zu starten.

Damit bleibt ein versionierter, begrenzter Datenaustausch erforderlich. Exakte Zahlen, Varianten und Fehlertypen brauchen weiterhin eine eindeutige Darstellung. Wir entfernen die handgeschriebene Produktionsvermittlung, nicht jede Serialisierung. Erzeugte Test-Codecs dürfen mechanische Konvertierung übernehmen. Stille Normalisierung, Werteverlust und ungültige Konvertierung dürfen nicht als erfolgreiche Produktionsablehnung gelten.

Ein Interpreter-Port pro Sprache wäre ein großer zusätzlicher Semantik- und Konformitätsaufwand. Native Einbettung oder WASM sind mögliche spätere Transportoptimierungen, keine Voraussetzungen. Der erste Versuch misst Aufruf-/Batchkosten, Startup und RAM; es gibt keinen Prozessstart pro Assertion.

Asynchrone Bewertung muss vor Testabschluss abgeschlossen sein. Synchrone Tests können Beobachtungen zunächst sammeln und beim Gruppenabschluss bewerten; der Collector muss dann trotz grünem nativen Runner einen fehlgeschlagenen Gesamtstatus liefern. Diese beiden Modi werden in der API sichtbar unterschieden.

Verschachtelte reine Aufrufe und dynamische Szenariofortsetzungen sind die schwierigste Schnittstelle. Der Bewerter darf fehlende Beobachtungen mit konkreter Call-ID und Argumenten anfordern. Ein lokaler, typisierter Test-Callback liefert den echten Aufruf und dessen Ergebnis. Die DSL-Reihenfolge und bereits festgelegten Inputs bleiben zentral. Keine allgemeine String-zu-Methoden-Registry wieder einführen. Wo dafür zu viel manueller Code entsteht, bleibt die Fähigkeit zunächst ausdrücklich unsupported; keine angebliche Vollständigkeit.

## 7. Nachweismatrix für Sprache und Laufzeit

| Vertragsmerkmal | Erforderlicher Nachweis | Grenze |
| --- | --- | --- |
| Alias, Record, Message, Event, Variant, Option, Liste | Tatsächlicher Wert gegen zentrale Typprüfung und Korpus | Native Typkompatibilität ist eine zusätzliche Compilerfrage |
| Range, Format, ausführbares `where` | Zentraler Interpreter mit identischen numerischen/String-Semantiken | Kein Ersatz durch nur JSON Schema |
| Ungültige Eingaben | Unveränderte rohe Eingabe an echten Produktionsdecoder; tatsächliche Ablehnung | Ablehnung im Test-Codec zählt nicht |
| Wire-Format | Tatsächliche Serialisierungsbytes und Parsergrenze | Roundtrip allein kann gemeinsame Encoder-/Decoderfehler übersehen |
| Entity-Form | Tatsächlich geladene Records, explizite Projektion falls nötig | Ein gültiger DTO beweist keine Tabellenstruktur |
| Entity-Identität/Key | Bestehende zentrale Identitätsprüfung über tatsächliche Beobachtungen | Kein universeller Eindeutigkeitsbeweis; physische Unique-Constraints separat testen |
| Operation, requires/ensures, deklarierte Fehler | Eingaben, echte Outcomes, relevante Vorher-/Nachher-Zustände | Technischer Fehler ist kein deklarierter Fachfehler |
| State-Effekte und unveränderte Records | Begrenzter, vollständiger relevanter Beobachtungsumfang | Nur den Rückgabewert zu prüfen übersieht andere Schreibeffekte |
| Calls, exakt null, Reihenfolge | Beobachteter Trace; vorhanden-leer ausdrücklich anders als fehlend | Mock-Erwartung allein beweist keinen tatsächlichen Aufruf |
| Events und Ressourcen | Tatsächliche Ereignis-/Ressourcenbeobachtung am wirksamen Port | Logtext oder Provisionierungserfolg genügt nicht |
| Pure Properties/Invarianten | Identische generierte Fälle und zentrale Ausdrucksauswertung | Endliche Abdeckung bleibt endlich |
| Beispiele | Geordnete Schritte, Inputs, Bindungen, Outcomes und Assertions | Einzelne isolierte Operationstests ersetzen die Folge nicht |
| Lifecycle | Zusammenhängender Pfad mit tatsächlichem Zustand und Setup-Datenfluss | Sequenzielle Pfade beweisen keine Nebenläufigkeit |
| Suites, Review, Constraints, Policies | Modellierte Teile automatisch; verbleibende Pflichten quellgebunden prüfen | Wrapper machen Freitext nicht automatisch ausführbar |
| Crash, Race, Isolation, Prozessende | Spezifische native Integrations-/Crash-/Ressourcentests | Ein Entity-Snapshot kann diese Garantien nicht ersetzen |

Das bestehende Manifest darf beim Backendwechsel keine Pflicht verlieren. Nicht unterstützte Ausdrucksformen und fehlende Beobachtungen bleiben offen. Geschäftsanforderungen werden nicht angepasst, damit die neue Bibliothek schneller fertig wird.

## 8. Vertrauensmodell und Evidenz

Auch ein kontrollierter Testlauf beweist nicht, dass vom Test gelieferte Beobachtungen wahr sind. Ein Test kann einen Fake oder eine Konstante liefern; ein Adapter kann das heute ebenfalls. Quellbindung, direkte überprüfbare Anbindung, Integrationsnachweise und gezielte Mutationen bleiben notwendig. Ein Receipt-Hash beweist Unverändertheit, keine Wahrheit.

Ein geplanter Receipt enthält Run-/Group-/Test-/Case-/Check-ID, Plan-/Anforderungsbindung, tatsächliche Beobachtungen oder verifizierbare Artefaktreferenzen, Bewertungsresultat, Fehlerkategorie, Quell-/Build-/Toolchain-/Bibliotheksidentitäten und Abschlussstatus. Daten werden begrenzt und atomar geschrieben. Test-stdout ist kein vertrauenswürdiger Ergebniskanal. Ein verwalteter separater Kanal verbessert Zuordnung und Vollständigkeit, schützt aber nicht gegen absichtlich täuschenden Testcode im selben Prozess.

Unterscheiden: freiwillig importiertes Testergebnis, vom Collector beobachtete native Ausführung, zentral reproduzierbare Bewertung und explizite menschliche/Agenteninspektion. Die ersten beiden Kategorien berechtigen nicht alleine zur Task-Übernahme. Ein direkter Entwicklerlauf kann Diagnostik liefern; vollständige Abnahme verlangt den kontrollierten Capture-/Build-/Collector-Pfad oder eine gleichwertig spezifizierte CI-Bindung.

Snapshots enthalten Tests, Helper, Fixtures, Bewertungsbibliothek, Generatoren, Lockfiles, Build-Skripte und externe Quellwurzeln, soweit relevant. Unbekannte dynamische Inputs verhindern die Behauptung vollständiger Bindung. Secrets werden nicht in Reports übernommen; private Konfiguration benötigt geeignete Identitätsnachweise statt Klartext oder leicht erratbarer Secret-Hashes.

Ein roter nativer Test, eine zentrale Vertragsverletzung, UNKNOWN, fehlende Fälle und Cleanup-Fehler bleiben unterscheidbar. Keine automatische Wiederholung bis Grün. Fehlende/technisch abgebrochene Fälle werden nie in normale PASS/FAIL-Beobachtungen umgedeutet.

## 9. Fehlerisolation, Replay und Wiederaufnahme

Ausführungseinheit ist eine begründete Isolationsgruppe, nicht zwangsläufig ein einzelner Test. Lifecycle-Schritte bleiben zusammen. Bei Crash wird die Gruppe beendet, ihr Ressourcenbesitz geprüft und ihr Ergebnis unvollständig. Unabhängige Gruppen dürfen weiterlaufen, wenn ihre Unabhängigkeit tatsächlich gesichert ist.

Replay führt gespeicherte konkrete Inputs im Anwendungstest erneut aus. Shrinking darf nur Inputs reduzieren, für die die reale Fixture erneut gültig aufgebaut werden kann. Minimierte Gegenbeispiele werden erneut bewertet; gespeicherte Labels sind kein Ersatz dafür.

Wiederaufnahme verwendet vollständige Receipts nur bei identischen relevanten Inputs, Versionen und nachgewiesener Gruppenisolation. Eine neue Ausführung erhält eine neue Identität und verweist auf die alte. Wiederaufnahme, Testauswahl und Wiederverwendung von PASS sind drei unterschiedliche Fähigkeiten. Anfangs bleibt Wiederverwendung außerhalb eines kontrollierten unterbrochenen Laufs deaktiviert.

## 10. Abhängigkeiten und Testauswahl

Vier Graphen bleiben begrifflich getrennt: semantische Spec-Abhängigkeiten, Implementierungs-/Build-Abhängigkeiten, Test-/Fixture-Abhängigkeiten und Ressourcenkonflikte. Der Task-DAG bleibt zusätzlich ein Planungsinstrument. Keiner dieser Graphen ist allein ein vollständiger Ersatz für die anderen.

Kanten erhalten Quelle, Version und Vertrauensklasse: deklariert, statisch ermittelt, dynamisch beobachtet oder unbekannt. Eine beobachtete fehlende Kante beweist keine Unabhängigkeit. Der kombinierte Graph muss Erklärungspfade ausgeben können.

| Änderung | Mindestens betroffener Bereich |
| --- | --- |
| Testkörper oder Assertion | Test selbst und explizit mitbenutzte Hilfsfunktionen/Generierungen |
| Shared Fixture, Hook oder Test-Helper | Alle transitiven Nutzer; unbekannte Nutzung erweitert den Scope |
| Produktionsfunktion | Statische mögliche Nutzer plus dynamisch beobachtete Testnutzer |
| Neue Funktion, Dispatch, Pluginregistrierung | Neuer Graph und konservative Modul-/Targetgrenze; alte Coverage reicht nicht |
| Gelöschtes/umbenanntes Symbol | Alte und neue Beziehungen sowie Verbraucher |
| Vertragsänderung | Zugeordnete Prüfungen und semantisch betroffene Pflichten |
| Migration, Datenfixture, Buildscript, Lockfile, Toolchain | Abhängige Targets/Integrationsgruppen; bei unbekannten Grenzen gesamter Scope |
| Ressourcenkonflikt | Planung/Isolation ändern; keine automatische fachliche Testabhängigkeit |
| Testbibliothek oder Bewerter | Betroffene Prüfungsversionen neu ausführen; alte Receipts bleiben historisch |

Definitions-Hashes umfassen den tatsächlichen Testkörper; transitive Helper-/Fixture-Hashes werden zusätzlich gespeichert. Ein Binary-Hash ersetzt keinen Testdefinitions-Hash. AST-Analyse liefert Struktur, keine vollständige semantische Callgraph-Garantie. Build-Tags, Features, Makros, dynamische Imports, Interface-Dispatch, Codegenerierung und native Kindprozesse brauchen explizite Behandlung oder einen konservativen Fallback.

Per-Test-Coverage erfordert korrekte Zuordnung. Prozessweite Zähler können bei parallelen Tests nicht einfach dem „aktuellen Test“ zugeschrieben werden. Initialisierung, Setup, Hintergrundarbeit und Kindprozesse müssen ebenfalls erfasst oder als Zuordnungslücke ausgewiesen werden. Zunächst bleibt isolierte Collection die Referenz; feinere Instrumentierung muss gegen diese geprüft werden.

Ressourcenhelfer bevorzugen eigene Namespaces/Verzeichnisse und kontrollierte Prozesse. Exklusive Ressourcen erzeugen Scheduling-Kanten. Ein Datenbank-Rollback isoliert nicht automatisch Sequenzen, externe Verbindungen oder Nebenprozesse. Einzeln bestandene Tests beweisen auch keine sichere gemeinsame Ausführung; diese braucht zusätzliche Interferenz-/Reihenfolgetests.

Das aktuelle Experiment bleibt zunächst Shadow: Vorhersage vor Referenzausführung einfrieren, vollständig laufen lassen, übersehene Fehler und Gesamtkosten messen. Keine Fehler in der Referenz bedeutet keine messbare Failure-Recall. Erst ein separat geprüftes Auswahlverfahren kann Testauslassung erlauben. Unveränderte Vertrags-PASS werden daraus nicht automatisch wiederverwendbar.

## 11. Erhalt und bewusste Begrenzung

Parser, IR, Sprachsemantik, zentrale Generation, Interpreter, semantische Änderungen, Rename-Historie und Task-Definitionen bleiben die Grundlage. Neue transportable Testartefakte bekommen eigene Versionen. SDK, Ergebnisformat, Konfiguration, Store und Sprache werden nicht gemeinsam hochgezählt.

Die plattformübergreifende Ausführung bleibt eine eigene Pflicht. Ein erfolgreicher Go-Test auf Linux beweist weder Windows-Verhalten noch Rust-/Bun-Bibliothekskonformität. Java erhält erst mit tatsächlicher Bibliothek und nativer Abnahme eine neue Capability.

Aufträge, Claims, Packages und Task-Abschlüsse werden nicht in die Testbibliothek verschoben. Ein bereits vollständiger Run kann später nach erneuter Prüfung der unveränderten Anforderungen und Quellen bewacht zugeordnet werden. Das verlangt eine explizite neue Übernahmeregel; derzeitige Task-Pins dürfen nicht einfach weggelassen werden.

## 12. Erfolgskriterien und verbleibende Entscheidungen

Der Pilot vergleicht dieselben Fälle, Budgets, Quellen und fachlichen Erwartungen auf altem und neuem Weg. Verglichen werden Urteile, fehlende Pflichten, erkannte Produktionsmutationen, Adapter-/Testcode, Reparaturaufwand, Build-/Ausführungszeit, Peak-RAM und Artefaktvolumen. Codezeilen werden nach generiert, handgeschrieben, Test und Produktion getrennt; Kalenderzeit wird nicht als aktive Arbeitszeit ausgegeben.

Offene Entscheidungen sind die konkrete Bibliothekspaketierung je Sprache, synchroner versus gebündelter Bewertungsmodus, Umgang mit dynamischen DSL-Callbacks, Collector-Integration bei bestehenden Tests, vollständige Build-/Quellbindung und der zugelassene Scope späterer Testauswahl. Der Pilot muss diese Entscheidungen anhand echter Anwendungspfade schließen.

Ein freiwilliger Rückbau projektspezifischer Adapter ist erst gerechtfertigt, wenn der migrierte Bereich einschließlich negativer Fälle, Zustand/Effekten, Replay und Pflichtreviews gleichwertig geprüft ist. Nicht migrierte Bereiche und das unterstützte bisherige Backend bleiben erhalten. Kostenersparnis bei entfallenen Pflichten zählt nicht als Erfolg.
