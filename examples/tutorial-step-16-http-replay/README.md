# Schritt 16: HTTP und ein sichtbares MiniBoard

Dieser Schritt verbindet das echte Board mit einem lokalen Bun-Server.
`GET /tasks` liefert öffentliche TaskSummary-Nachrichten, `GET /tasks/:id`
liefert eine vorhandene Aufgabe oder 404. Die zusätzliche administrative Route
`GET /board/tasks` liefert den gespeicherten Status. `POST /tasks` legt eine
Aufgabe an; `PATCH /tasks/:id` ersetzt sie über die bestehenden Board-Methoden.
Die vierte Spec-Datei modelliert diese Operationen einschließlich ihrer
tatsächlichen Zustandsbezüge. Ein ausdrückliches
Beispiel prüft einen vorhandenen Datensatz zusätzlich zu generierten Eingaben.

Die Webseite auf `/` zeigt die Aufgaben, lädt Details bei Auswahl und erlaubt
erneutes Laden. Sie verwendet dieselben echten HTTP-Routen. Fehler und leere
Listen werden sichtbar; eine verspätete Detailantwort überschreibt keine neuere
Auswahl. Das Formular legt echte Aufgaben an, die Checkbox setzt sie auf
Erledigt oder wieder Offen. Fehlgeschlagene Änderungen erhalten den gespeicherten
Zustand und stellen die Checkbox wieder her. Aufgabentitel werden mit
`textContent` dargestellt. Titel haben entsprechend der Spec höchstens 16 Zeichen.

## Lokal ansehen

```sh
cd examples/tutorial-step-16-http-replay
bun install --frozen-lockfile
bun run start
```

Öffne danach <http://127.0.0.1:4173/>. Mit `PORT=4180 bun run start` lässt sich
ein anderer Port wählen. Der Startcode erstellt drei tatsächliche Aufgaben im
Arbeitsspeicher; ein Neustart stellt diese Beispieldaten wieder her.
Die kleine Oberfläche zeigt, erstellt und erledigt Aufgaben. Sie benötigt keine
Datenbank; Änderungen bleiben bis zum nächsten Server-Neustart erhalten.

## Prüfen

```sh
bun run typecheck
bun run test --coverage
../../target/release/dctx0 verify complete dctx0 --project .
```

Der externe Prüfadapter startet für HTTP-Fälle eine frische lokale Testsitzung.
Er ruft die tatsächlichen Client- und Handler-Methoden auf. Eine ausschließlich
im Prüfadapter verwendete Korrelation ordnet echte Serverzugriffe dem geprüften
Aufruf zu. Produktion importiert keine Prüfbibliotheken oder Spec-Dateien.

Die 38 unabhängigen TypeScript-Tests bestehen mit 100 % gemessener Zeilen- und
Funktionsabdeckung der Produktionsmodule unter `src` und des tatsächlichen
`server.ts`-Starts. Das JavaScript im HTML-String wird dadurch nicht als
ausgeführter Browsercode gemessen. Eine
zusätzliche echte Chromium-Prüfung kontrolliert Laden, Auswahl, leere Listen,
Fehler, Retry, konkurrierende Detailantworten, tatsächliches Anlegen und
Erledigen, fehlgeschlagene Änderungen und das schmale Layout:

```sh
chromium --headless --no-sandbox --disable-gpu --remote-debugging-port=9228 \
  --user-data-dir=/tmp/dctx0-tutorial-browser about:blank
# In einem zweiten Terminal, vom Repository-Verzeichnis aus:
bun examples/tutorial/ui-browser-check.ts
```

Der Browser-Debugger ist ein lokales Entwicklungswerkzeug. Beende ihn nach der
Prüfung. `BROWSER_DEBUG_URL` wählt einen anderen Debugger; `UI_SCREENSHOT` wählt
den Screenshot-Pfad.

Die Browserprüfung misst zusätzlich alle sieben tatsächlichen JavaScript-
Funktionen und 58 ausführbaren Zeilenanfänge mit V8 Precise Coverage. Beide
erreichen 100 %; die Rohbereiche bleiben im Bericht erhalten. Dies ist keine
vollständige Branch- oder All-Input-Abdeckung.

Gesamtvalidierung der Serie, Mutationen und Replay bleiben separate
Abschlussprüfungen. Ein erfolgreicher Browsercheck behauptet keine vollständige
Abnahme aller Tutorial-Schritte oder Deployment-Fähigkeiten.
