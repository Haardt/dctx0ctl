# Schritt 15: Reviews, Prüfsuiten und Release-Policy

Die bisherigen Specs und Produktionsmodule bleiben erhalten. Die dritte
Spec-Datei verification.dctx0 ergänzt alle benannten proof/test-Suiten und
einen konkreten BoundaryReview mit inspect, criteria und question.
Catalog übernimmt außerdem ausdrücklich die Ownership für Note.

Der Review beurteilt die tatsächlichen Implementierungsgrenzen: gewöhnliche
Interface-Injektion, öffentliche Messages, echte Consumer unter Mocks,
beobachtete Zustände/Effekte, explizite URLs und Cleanup. Sein source-bound
Bericht in inspection-notes.json enthält eine Freigabe und einen niedrigen
Befund zu den Grenzen endlicher Prüfungen. Er erzeugt keine Testergebnisse.

Release verlangt PASS für sämtliche benannten Suiten, keinen REQUEST_CHANGES
im Review und null hohe Findings. Rust bewertet diese Verknüpfungen zentral.
Ein nativer FAIL wäre durch eine Review-Freigabe nicht überstimmbar. Fehlende
oder veraltete Inspektionen verhindern den vollständigen Abschluss.

```sh
bun install --frozen-lockfile
bun run typecheck
bun test --coverage src
../../target/release/dctx0 verify complete dctx0 --project .
```

Der Checkpoint besteht mit 3.710 Fällen und 147 Tasks Done/PASS. Die gesamte
Produktion erreicht 100 % gemessene Zeilen- und Funktionsabdeckung. Im nativen
Bericht bleiben manuelle Anforderungen offen; der separate effektive Bericht
bindet die tatsächlich verfassten Inspektionen und weist die erfüllte Policy aus.
Das ist keine automatische Überprüfung beliebiger Architekturprosa.

Bei diesem Review wurde die Dezimal-Factory vereinheitlicht: führendes Plus
wird wie im gemeinsamen Werteformat abgelehnt und unabhängig negativ getestet.
Die gesamte Serie wird nach dieser Korrektur abschließend neu geprüft.
Gesamtvalidierung, Mutation und Replay stehen noch aus.

`examples/tutorial/agent-step.sh 15` erstellt die aktuelle Spec und Tests mit
dem Produktionscode aus Schritt 14. Der Agent erhält keine alten Freigaben;
er muss die genannten Review-Kriterien erneut inhaltlich beurteilen.
