# Schritt 3: Records, Varianten und Optionswerte

Die komplette Spec aus Schritt 2 bleibt erhalten. `Status` zeigt eine Variante
 ohne Payload, `Choice` eine mit Payload. `Details` kombiniert beide mit Listen,
Optionswerten und den übrigen Builtins. `Attempt` verwendet `Dctx0Result`.
`keepDetails` und `keepAttempt` rufen die tatsächlichen Produktionsvalidatoren
auf; die Spec verlangt, dass die Werte unverändert zurückgegeben werden.

Die Anwendung verwendet eigene TypeScript-Typen. Ganzzahlen sind `bigint`,
Dezimalwerte exakte Strings, Bytes `Uint8Array`. Der externe Adapter übersetzt
diese Repräsentationen in das gemeinsame Werteformat. Die Byte-Factory prüft
Elemente vor der Umwandlung, damit beispielsweise 256 nicht still zu 0 wird.
Optionswerte haben ausdrückliche `none`/`some`-Alternativen.

```sh
bun install --frozen-lockfile
bun run typecheck
bun test --coverage src
../../target/release/dctx0 verify complete dctx0 --project .
```

Der Checkpoint besteht mit 997 nativen Pflichtfällen und 35 Tasks Done/PASS.
Die unabhängigen Produktionstests bestehen mit vollständiger gemessener
Zeilen- und Funktionsabdeckung. Die Gesamtserie und ihre Mutationstests
werden abschließend gemeinsam validiert. Das ist kein Beweis über alle möglichen Eingaben.

Eine frische Agent-Übung erstellt `examples/tutorial/agent-step.sh 3` vom
Repository aus: aktuelle Spec und Tests, Produktionscode aus Schritt 2 und
keine übernommenen Review-Freigaben.
