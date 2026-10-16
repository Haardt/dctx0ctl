# Native Abhängigkeitsauflösung: festes Limit von 1.000 Referenzen

## Auftrag

Behebe die Skalierungsgrenze der nativen Abhängigkeitsauflösung im Repository
`/home/massenhaft/workspaces/conpinion/dctx0ctl`.

## Problem und bestätigter Befund

In `crates/dctx0-engine/src/verification_run/dependencies.rs`, bei der Prüfung
`resolved.len() >= 1000` (zum Untersuchungszeitpunkt Zeile 277), beendet
`resolve_native` die Auflösung nach 1.000 Referenzen. Größere
TypeScript-Anwendungen erreichen dieses feste Limit und erhalten:

```text
DEPENDENCY_RESOLUTION_BUDGET
Native dependency resolution exceeded 1000 references
```

Dadurch bleibt ihre Provenienz unvollständig und das Verifikationsgate geschlossen.
Das wurde mit dem offiziellen Release v0.2.3 am Company Manager reproduziert.
Die Meldung der Unvollständigkeit ist korrekt; zu beheben ist die feste
Skalierungsgrenze, nicht diese Schutzwirkung. Gezählt werden Referenzen, nicht
notwendigerweise 1.000 unterschiedliche Abhängigkeiten.

Der bestehende Test
`native_dependency_budget_retains_its_executed_prefix_and_reports_incompleteness`
in derselben Datei verwendet 1.001 Referenzen auf dieselbe Datei und prüft den
erhaltenen Präfix. Der Integrationstest
`exhausted_native_dependency_budget_cannot_adopt_otherwise_passing_contracts`
in `crates/dctx0-engine/tests/verification_run.rs` sichert die verhinderte
Aufgabenübernahme ab.

## Erwartete Lösung

- Ersetze das feste Limit durch ein explizites, begrenztes und konfigurierbares
  Budget. Prüfe, ob wiederholte identische Auflösungen effizient
  zwischengespeichert werden können, ohne Referenzen aus der Evidenz zu verlieren.
- Binde die wirksame Konfiguration an die Verifikationsevidenz und berücksichtige
  Replay sowie Aufgabenübernahme.
- Bei Budgetüberschreitung müssen die bereits gewonnenen Beobachtungen erhalten
  bleiben, die Provenienz unvollständig bleiben und automatische Übernahme
  verhindert werden.
- Erhöhe nicht lediglich die Konstante und entferne keine Schutzprüfung.
- Dokumentiere Konfiguration, Standardwert, zulässige Grenzen und Diagnose.

## Erforderliche Nachweise

- Eine tatsächliche Abhängigkeitskette mit mehr als 1.000 Referenzen lässt sich mit
  ausreichendem Budget vollständig erfassen.
- Ein kleineres Budget erzeugt weiterhin den korrekten Unvollständigkeitsbefund.
- Wiederholte Referenzen, Zyklen und fehlschlagende Auflösungen verlieren keine
  Evidenz.
- Bestehende Tests zum Budgetabbruch und zur verhinderten Aufgabenübernahme bleiben
  erhalten beziehungsweise werden fachlich angepasst.
- Führe die vorgesehenen Repository-Gates aus und dokumentiere die tatsächlich
  ausgeführten Prüfungen.

## Reproduktion am Company Manager

Arbeitsverzeichnis:
`/home/massenhaft/workspaces/conpinion/company-manager`

```bash
bun run spec:check
bun run spec:verify --scope=platform-runners
```

Dabei sicherstellen, dass das zu prüfende dctx0-Binary im `PATH` liegt. Das
SHA-256-geprüfte Release v0.2.3 wurde projektlokal abgelegt. Damit lässt sich der
bestätigte Lauf ausdrücklich so wiederholen:

```bash
PATH="$PWD/.artifacts/toolchains/dctx0-v0.2.3:$PATH" \
  bun run spec:verify --scope=platform-runners
```

Mit v0.2.3 wurden 6.246/6.246 Fälle erfasst, aber die Provenienz blieb wegen
`DEPENDENCY_RESOLUTION_BUDGET` unvollständig. Das Ergebnis war `UNKNOWN`,
Exitcode 1. Der gezielte Bericht liegt unter
`.artifacts/verification/native-scope-platform-runners.json`.

Dieser gezielte Lauf ersetzt keine vollständige Company-Manager-Verifikation.

## Abgrenzung und Arbeitsstand

Veraltete Quellreviews, fehlende Adapter-Beobachtungen und offene manuelle Pflichten
im Company Manager sind eigenständige Befunde. Sie dürfen durch diesen Fix weder
automatisch freigegeben noch verborgen werden. Ein behobenes Abhängigkeitslimit
allein bedeutet deshalb keinen vollständigen Company-Manager-PASS.

Im dctx0ctl-Checkout bestand zum Untersuchungszeitpunkt eine fremde Änderung an
`artice-medium.md`. Diese erhalten und nicht bearbeiten. Vor Implementierung die
aktuellen Repository-Anweisungen und den tatsächlichen Arbeitsstand erneut lesen.
