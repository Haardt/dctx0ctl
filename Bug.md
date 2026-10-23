# Bug: Native Beispielprüfung verliert Call-Observer bei „exactly 0“

## Auftrag

Den verbleibenden UNKNOWN-Fall aus Company Manager in dctx0 beheben und mit
nativen Regressionstests absichern. Die Assertion und die Unterscheidung zwischen
fehlender Beobachtung und tatsächlich null Aufrufen müssen erhalten bleiben.

Beobachtete Umgebung: dctx0 0.2.4, Bun 1.3.2, Linux arm64.
Consumer: `/home/massenhaft/workspaces/conpinion/company-manager`.

## Fachlicher Zweck

Wenn das Lesen eines kanonischen Wissensdokuments ausfällt, darf Company Manager
nicht ersatzweise Dokumentbytes über den Kanal-Storage lesen. Die Prüfung soll
belegen, dass `Storage.content("artifact-1")` dabei exakt nullmal aufgerufen wird.
Ein Fehler beim Wissenszugriff darf keinen alternativen Inhaltszugriff eröffnen.

## Tatsächliches Ergebnis

Fall-ID:

```text
CompanyManager.ChannelArtifacts.Ledger.Memory outage never falls back to channel document bytes
```

Aktueller gespeicherter Befund:

```json
{
  "status": "UNKNOWN",
  "issue": {
    "code": "TRACE_OBSERVER_REQUIRED",
    "message": "Actual collaborator calls were not observed"
  }
}
```

Position: `spec/channel-artifacts-services.dctx0:215`, Spalten 14–59.
Der native Herkunftsnachweis ist COMPLETE; es gibt keine Provenance-Lücken.
Der Adapter deklariert `call-observation` und zeichnet tatsächliche Storage-Aufrufe
auf. Der TypeScript-Worker kann eine verfügbare, leere Aufrufliste liefern.

UNKNOWN ist hier weder ein nachgewiesener Produktfehler noch ein bestandener
Sicherheitsnachweis: Die Auswertung erhält die benötigte Beobachtung nicht.

## Reproduktion im vorhandenen Consumer

```sh
cd /home/massenhaft/workspaces/conpinion/company-manager
bun run spec:verify --scope=channel-artifacts
```

Den vorhandenen Package-Befehl verwenden: Er begrenzt den aggregierten
Prozessbaum auf ein gemessenes RSS-Budget von 3072 MiB und verarbeitet die großen
nativen Berichte inkrementell. Keine parallelen schweren Prüfläufe starten.

Das unveränderte Beispiel lautet:

```dctx0
example "Memory outage never falls back to channel document bytes" {
  when result = Evidence.read("artifact-1")
  expect result fails ArtifactUnavailable
  expect calls Storage.content("artifact-1") exactly 0
}
```

Die tatsächliche Implementierung wird über
`.dctx0/testing/typescript/artifacts-adapter.ts` aufgerufen. Die Fixture
`.dctx0/testing/typescript/artifacts/artifact-native-fixture.ts` verwendet Core/Hono
mit einem expliziten beobachtbaren Storage-Port-Testdouble. Sie legt kanonische
Wissensdokumente an und macht für das benannte Beispiel nur den Memory-Zugriff
unverfügbar. Storage-Aufrufe werden tatsächlich aufgezeichnet; das erwartete
Domain-Ergebnis wird nicht als Konstante im Adapter nachgebaut.

## Vorhandene Belege

Alle folgenden Pfade liegen im Company-Manager-Checkout:

- `.artifacts/verification/native-execution-channel-artifacts.json`: vollständige
  native Eingaben und Beobachtungen, ungefähr 219 MiB.
- `.artifacts/verification/native-assessment-channel-artifacts.json`: kompakte
  Fallurteile und unveränderter UNKNOWN-Befund.
- `.artifacts/verification/native-scopes.json`: letzter vollständiger Lauf über
  15 konfigurierte Bereiche; 49.045 erfasste Pflichtfälle, davon 49.044 PASS,
  ein UNKNOWN und null FAIL, mit COMPLETE-Provenance.
- `.artifacts/verification/feature-completion/verify.log`: zugehöriger Gesamtlauf,
  begonnen am 2026-10-18 um 02:45:04 UTC.

Der betroffene Einzelbereich enthält 7.314 Pflichtfälle: 7.313 PASS und diesen
UNKNOWN-Fall. Die 49.045 Fälle des Gesamtlaufs sind überwiegend generierte
Vertrags-/Randwertfälle, keine ebenso vielen handgeschriebenen Produkttests.

## Ansatzpunkt aus der bisherigen Quellprüfung

Diese Stellen vor einer Änderung am aktuellen dctx0-Stand erneut prüfen:

- `crates/dctx0-verification/src/scenario.rs`: `Scenario::value` verwendet nach
  bisheriger Analyse gewöhnliches `evaluate(...)` ohne `ObservationContext`.
  `Expectation::Assertion` wertet darüber die Call-Count-Expression aus.
  `Scenario::deliver` reicht den tatsächlichen optionalen Call-Trace nicht bis
  zu dieser Auswertung durch.
- `crates/dctx0-interpreter/src/observed.rs`: verlangt für Call-Count-Ausdrücke
  zu Recht einen tatsächlichen Beobachtungskontext.
- `targets/typescript/harness/worker.ts`: unterscheidet über die deklarierte
  Capability bereits zwischen fehlendem Observer und vorhandener leerer Liste
  (`calls: []`). Diese Unterscheidung muss bis zur Assertion erhalten bleiben.

Tatsächliche Aufrufbeobachtungen durch den nativen Beispielpfad weiterreichen und
beobachtungsabhängige Assertions mit diesem Kontext auswerten. Operationsidentität,
Argumente und die Reihenfolge mehrerer `when`-Schritte müssen erhalten bleiben.

## Zugehöriger Fehler in der Capability-Projektion

Im selben Bericht verlangt die Call-Count-Assertion zusätzlich `mock-injection`,
obwohl dieses Beispiel kein `given` enthält und tatsächliche Aufrufe beobachtet:

```json
{
  "code": "ADAPTER_CAPABILITY_UNAVAILABLE",
  "message": "The selected plan requires adapter capability mock-injection"
}
```

Call-Beobachtung und vorbereitete Mock-Antworten müssen getrennt bleiben.
Der Consumer darf keine nicht implementierte Mock-Capability vortäuschen müssen,
um eine tatsächliche Null-Aufruf-Beobachtung auszuwerten.

## Akzeptanz und Regressionstests

1. Verfügbarer Observer und null passende tatsächliche Aufrufe: `exactly 0` PASS.
2. Verfügbarer Observer und ein passender tatsächlicher Aufruf: dieselbe Assertion FAIL.
3. Fehlender Observer: weiterhin UNKNOWN; keine erfundene leere Liste.
4. Abweichende Argumente und andere Operationsziele zählen nicht als Treffer.
5. `exactly 1`, mehrere geordnete `when`-Schritte und deklarierte Anwendungsfehler
   behalten ihre tatsächlichen Beobachtungen. Mock-Vorgaben werden nicht als
   zusätzliche tatsächliche Aufrufe gezählt.
6. Ein Adapter mit `call-observation`, ohne `mock-injection` und ohne `given`
   kann tatsächliche Aufrufanzahlen prüfen.
7. Den obigen Consumer-Bereich vollständig wiederholen: alle Pflichtfälle
   erfassen, COMPLETE-Provenance beibehalten und das konkrete Beispiel mit PASS
   nachweisen. Eventuell verbleibende manuelle Pflichten separat ausweisen.

Keine Assertion entfernen oder abschwächen, UNKNOWN nicht durch ein manuelles
PASS ersetzen und historische Belege nicht umschreiben. Einen neuen nativen
Nachweis mit tatsächlicher Ausführung erzeugen.

Diese Übergabe beschreibt ausschließlich diesen Observer-/Projektionsfall.
Es wurden dafür keine dctx0-Implementierungsdateien verändert.
