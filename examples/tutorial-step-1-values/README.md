# Schritt 1: Werte und eine echte Operation

Ein kleines Board zählt null bis drei Einträge. `Count` beschreibt genau diese
vier Werte. `increment` ist eine reale TypeScript-Funktion: ihre Vorbedingung
schließt ein volles Board aus; die Nachbedingung beschreibt das Ergebnis.

Die Spec führt `workspace`, `domain`, `intent`, `value`, `range`, `surface`,
`operation`, `pure`, `requires`, `ensures`, `example`, `when`, `expect`,
`invariant`, `forall` und `assert` ein. `implies` verhindert in der Invariante
einen Aufruf außerhalb der Vorbedingung. Produktion verwendet `bigint`, damit
Ganzzahlen nicht durch JavaScript-Rundung verändert werden.

```sh
bun install --frozen-lockfile
bun run typecheck
bun test --coverage src
../../target/release/dctx0 check dctx0
../../target/release/dctx0 verify run dctx0 --project . --no-adopt
```

Der Adapter übersetzt Werte und ruft die tatsächliche Funktion auf. Erwartungen
bleiben in der Spec und im Rust-Prüfkern. Tests überprüfen auch ungültige Werte
und die Ablehnung eines vollen Boards. Die Lösung wird zusätzlich mutiert, um
sicherzustellen, dass eine falsche Implementierung auffällt.

Eine frische Agent-Arbeitskopie erzeugst du vom Repository aus mit
`./examples/tutorial/agent-step.sh 1`.
