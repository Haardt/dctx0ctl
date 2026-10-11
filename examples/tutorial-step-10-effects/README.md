# Schritt 10: Tatsächlich beobachtete Ressourcen und Effekte

Die bisherige Spec und Produktion bleiben erhalten. Die bestehenden
Zustandsleser deklarieren jetzt ihre read-Berechtigungen. Der Prüfadapter
beobachtet tatsächliche Zugriffe auf die gespeicherten Board-Arrays mit
Proxies. Ein AsyncLocalStorage-Scope ordnet Zugriffe auch nach await der
wirklich aufgerufenen Operation zu. Snapshots für den Harness laufen außerhalb
dieses Scopes und erfinden keine Anwendungseffekte.

Die Zuordnung tasks → Task und entries → Note ist ausdrücklich überprüfter
Adaptercode. Fehlt ein tatsächliches Array, wird die Umgebung nicht scheinbar
prüffähig. Cleanup stellt die ursprünglichen Property-Deskriptoren wieder her.
Unmodellierte weitere I/O bleibt außerhalb dieses Beobachters.

Eine explizite lokale External-Grenze Signals.Clock zeigt den observe-Effekt.
SystemClock liest die echte native Uhr; Diagnostics erhält diese über einen
normalen Callback. Der Prüfadapter protokolliert dessen tatsächliche Ausführung.
Die Spec schreibt weder eine URL noch einen Remote-Dienst vor. Die umfassendere
externe Anbindung folgt später.

**read/write/observe sind Berechtigungen, keine Mindestaufrufzahlen.** Der
Rust-Prüfkern beurteilt tatsächlich gelieferte Ressourcenereignisse gegen die
modellierten Grenzen. Für genaue Zahlen braucht es ausführbare Trace-Anforderungen.
Die bereits eingeführten Calls und Emits bleiben separat beobachtet.

```sh
bun install --frozen-lockfile
bun run typecheck
bun test --coverage src
bun test ./.dctx0/testing/typescript/resource-observer.test.ts
../../target/release/dctx0 verify complete dctx0 --project .
```

3.246 native Pflichtfälle bestehen. Die Produktionstests erreichen 100 %
gemessene Zeilen- und Funktionsabdeckung; vier weitere Tests prüfen den
Prüfbeobachter. Zwei manuelle Architekturpunkte behalten ihre separate Evidenz.
Tatsächliche Schreiboperationen kommen im folgenden Schritt. Gesamtvalidierung
und Mutationsabnahme der Serie stehen noch aus.

`examples/tutorial/agent-step.sh 10` erstellt vom Repository aus die aktuelle
Spec und Tests mit dem Produktionscode aus Schritt 9; Freigaben werden nicht kopiert.
