# Schritt 9: Fachliche Fehler und tatsächliche Events

Die bisherige Spec bleibt erhalten. `MissingTask` deklariert einen fachlichen
Fehler mit taskId; `TaskViewed` ein Event mit derselben Payloadstruktur.
Catalog besitzt das Event und bietet die neue Operation requireTask an.

Der reale Consumer wartet auf seinen Reader. Bei None wirft er das tatsächliche
MissingTask-Objekt. Bei einer vorhandenen Summary veröffentlicht er TaskViewed
über einen normalen Callback und gibt die Summary zurück. Produktionsfactorys
validieren beide Payloads. Der externe Adapter übersetzt das tatsächlich
gefangene Fehlerobjekt in den Fehler-Envelope und beobachtet den echten Callback.
Er erzeugt weder Fehler noch Events aus den erwarteten Spec-Ergebnissen.

Die Spec zeigt `! MissingTask`, success/failure-Nachbedingungen, `expect Success`,
`expect result fails MissingTask` sowie `expect emits` mit einer konkreten Payload.
Der Fehler-Payload wird als Record geprüft; der Failure-Envelope ist ausschließlich
die Darstellung des tatsächlichen Operationsausgangs.

```sh
bun install --frozen-lockfile
bun run typecheck
bun test --coverage src
../../target/release/dctx0 verify complete dctx0 --project .
```

3.244 native Pflichtfälle bestehen. Das erfolgreiche Beispielszenario enthält
exakt ein beobachtetes Event, das fehlgeschlagene keines. Unabhängige Tests prüfen
zusätzlich Fehlerklasse, Payload und die ausbleibende Veröffentlichung nach einem
Fehlschlag. Die Produktion erreicht 100 % gemessene Zeilen- und Funktionsabdeckung.
Die zwei Architekturinspektionen bleiben gesonderte Evidenz; abschließende
Gesamtvalidierung und Mutationsabnahme der Serie stehen aus.

`examples/tutorial/agent-step.sh 9` erstellt vom Repository aus eine frische
Übung mit aktueller Spec und Tests sowie dem Produktionscode aus Schritt 8.

Die Spec fordert jetzt zusätzlich genaue Laufzeitspuren: ein tatsächlicher
Lookup, genau ein TaskViewed mit passendem Payload nach Erfolg und null Events
nach dem deklarierten MissingTask-Fehler. Die unabhängige Event-Mutation muss
an dieser ausführbaren Assertion scheitern.
