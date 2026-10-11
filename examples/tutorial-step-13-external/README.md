# Schritt 13: Explizite externe HTTP-Anbindung

Alle bisherigen Anforderungen bleiben erhalten. Directory.HealthApi ist eine
explizite externe Grenze mit der öffentlichen Message Health. Connectivity
verwendet den tatsächlichen HealthClient und gibt die erhaltene Bereitschaft
zurück. Die Spec weist keinen internen Surface automatisch eine URL zu.

HealthClient erhält seine Basis-URL ausdrücklich und fragt GET /health ab.
Der Client lehnt HTTP-Fehler ab und prüft echte JSON-Antworten mit der normalen
Produktionsfactory. healthHandler ist der tatsächlich ausgeführte lokale Dienst.
Die Testkonfiguration benennt Loopback, einen freien Port und den Anfangswert.
Der Prüfadapter startet den Dienst ausschließlich für erforderliche HTTP-Fälle,
ruft echte Produktionsklassen auf und stoppt den Server nach jeder Sitzung.

Die Prüfung beobachtet den tatsächlichen HTTP-Aufruf als observe-Effekt.
Sie ersetzt keinen Netzwerkaufruf durch eine vorbereitete Beispielantwort.
Der Nachweis betrifft den lokalen Dienst und Client; eine fremde Bereitstellung
oder TLS-Konfiguration wird damit nicht behauptet.

```sh
bun install --frozen-lockfile
bun run typecheck
bun test --coverage src
../../target/release/dctx0 verify complete dctx0 --project .
```

3.678 native Fälle bestehen. Unabhängige HTTP-Tests prüfen beide ready-Werte,
HTTP 503, ungültige Payloads, falsche Pfade/Methoden und fehlerhafte URLs.
Die Produktion erreicht 100 % gemessene Zeilen- und Funktionsabdeckung.
Die vollständige Serienvalidierung und Mutationsabnahme stehen noch aus.

`examples/tutorial/agent-step.sh 13` erstellt vom Repository aus eine frische
Übung mit vollständiger aktueller Spec und dem Produktionscode aus Schritt 12.
