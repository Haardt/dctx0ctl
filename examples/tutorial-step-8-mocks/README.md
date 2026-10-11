# Schritt 8: Port-Mocks und tatsächliche Aufrufe

Die vollständige Spec aus Schritt 7 bleibt erhalten. Zwei neue Beispiele
bereiten mit `given TaskReader.lookup(...) returns ...` das Verhalten des
Readers vor: einmal eine vorhandene Summary, einmal None.

Der Consumer ist weiterhin die echte Catalog-Klasse. Ihr normaler Reader kann
jetzt auch asynchron antworten. Der externe Adapter injiziert ausschließlich
für diese beiden Beispielsitzungen einen Reader, der den zentralen Mockplan
über `context.collaborator` abfragt. Er übersetzt Antworten und protokolliert
tatsächliche Aufrufe. Er erzeugt keine erwarteten Ergebnisse selbst.

Die Szenarioeingaben sind ausdrücklich in examples.json hinterlegt. Die
vorbereitete Antwort passt zum tatsächlichen Initialzustand der Sitzung,
damit die bestehende Zustandsnachbedingung erhalten bleibt. Die sonstigen
generierten Fälle sowie direkte Port-Conformance prüfen weiter BoardReader.
Ein bestandener Consumer-Mocktest bestätigt daher nicht allein den echten Port.

Unabhängige Tests prüfen echte Verdrahtung, asynchrone Antworten und die
unveränderte Weitergabe einer abgelehnten Reader-Promise. Deklarierte fachliche
Fehler und Event-Beobachtung folgen in Schritt 9.

```sh
bun install --frozen-lockfile
bun run typecheck
bun test --coverage src
../../target/release/dctx0 verify complete dctx0 --project .
```

Der Checkpoint besteht mit 3.160 ausgeführten nativen Fällen, 121 Tasks
Done/PASS und 100 % gemessener Produktionsabdeckung. Die zwei manuellen
Architekturpunkte aus Schritt 7 werden weiterhin separat durch quellgebundene
Inspektionen bestätigt. Gesamtvalidierung und Mutationsabnahme stehen aus.

`examples/tutorial/agent-step.sh 8` erstellt vom Repository aus die aktuelle
Spec samt Tests mit dem Produktionscode aus Schritt 7. Die Referenzlösung und
historische Freigaben werden nicht als fertige Lösung in die Übung kopiert.

Der Consumer muss außerdem `calls TaskReader.lookup(id) exactly 1` erfüllen.
Rust zählt tatsächlich aufgezeichnete Aufrufe und vergleicht ihre Argumente;
statische Imports oder Aufrufmuster liefern diesen Nachweis nicht.
