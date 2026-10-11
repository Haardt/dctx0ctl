# Schritt 4: Ausdrücke und Konstruktionen

Alle bisherigen Deklarationen bleiben erhalten. Vier kleine reale Operationen
zeigen die nächste Sprachschicht:

- `score` verwendet exakte Ganzzahlarithmetik. Vor- und Nachbedingungen zeigen
  Addition, Subtraktion, Multiplikation, Division, Rest und unäres Minus.
- `eligible` verbindet Vergleiche mit `not`, `and`, `or` und `implies`.
  Dieselbe Erwartung verwendet sowohl Listen- als auch Bereichszugehörigkeit.
- `describe` zeigt Stringverkettung.
- `named` erzeugt eine echte Variante mit Payload. Die Spec konstruiert sie als
  `Named { value = title }`; `is` prüft ihre Alternative.

Das neue Beispiel ruft mehrere Operationen in Reihenfolge auf. Die zwölf
zulässigen Eingabepaare von `score` werden zusätzlich unabhängig vollständig
getestet. Der externe Adapter ruft Produktionsfunktionen auf und übersetzt
Repräsentationen; seine Implementierung enthält keine erwarteten Antworten.

```sh
bun install --frozen-lockfile
bun run typecheck
bun test --coverage src
../../target/release/dctx0 verify complete dctx0 --project .
```

Der Checkpoint umfasst 1.030 native Pflichtfälle. Die Produktionstests erreichen
100 % gemessene Zeilen- und Funktionsabdeckung. Die Gesamtserie und gezielte
Implementierungsmutationen werden abschließend gemeinsam geprüft.

`examples/tutorial/agent-step.sh 4` erstellt vom Repository aus eine Übung mit
der aktuellen Spec und Tests sowie dem Produktionscode aus Schritt 3.
