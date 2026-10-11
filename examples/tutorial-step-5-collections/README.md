# Schritt 5: Collections, Strings und exakte Dezimalrechnung

Die bisherige Spec bleibt vollständig erhalten. Kleine Operationen zeigen jede
Collection-Funktion: Anzahl und Summe, all/any, contains/distinct, min/max,
filter/map, auf- und absteigendes Sortieren, first/single, mean/indexOf,
getOrElse, reverse und slice. `optional` kombiniert ifElse, Some und None.

`min` und `max` verlangen eine nicht leere Liste; `single` verlangt genau ein
Element. Die Spec verwendet dafür ausführbare Vorbedingungen. Die Produktion
validiert sie ebenfalls. Sortieren, Filtern und Slicing verändern die Eingabe
nicht. **slice nimmt Offset und Anzahl**, während JavaScript slice einen
Endindex nimmt: `slice(values, 1, 3)` wird daher als `values.slice(1, 4)` angebunden.

Die Stringoperationen zeigen lowercase, uppercase, trim, length, startsWith,
endsWith und includesText. Stringlängen zählen UTF-16-Codeeinheiten.

Vier Dezimaloperationen zeigen Down, Up, HalfUp und HalfEven. `third` zeigt
divide mit ausdrücklich gewählter Präzision und Rundung. Die Anwendung rechnet
mit bigint-Koeffizienten und transportiert Ergebnisse als exakte Strings.
Ein endlicher exakter Quotient bleibt erhalten; eine periodische Division wird
gerundet. Das Beispiel prüft positive und negative Rundungsties ausdrücklich.

```sh
bun install --frozen-lockfile
bun run typecheck
bun test --coverage src
../../target/release/dctx0 verify complete dctx0 --project .
```

Der Checkpoint besteht mit 2.793 nativen Pflichtfällen, 107 Tasks Done/PASS
und keinen offenen Obligations.
Die unabhängigen Produktionstests bestehen mit 100 % gemessener Zeilen- und
Funktionsabdeckung. Abschließende Mutationstests und Gesamtvalidierung stehen aus.

`examples/tutorial/agent-step.sh 5` erstellt vom Repository aus eine frische
Übung mit diesen Anforderungen und Tests sowie dem Produktionscode aus Schritt 4.
