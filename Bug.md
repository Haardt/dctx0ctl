# Bug: Vollständige Quellreviews scheitern an der 1-MiB-Eingabegrenze

Stand: 17.10.2026. Reproduziert mit `/home/massenhaft/.cargo/bin/dctx0`, Version **0.2.4**, im Verbraucherprojekt `../company-manager`. Dieser Bericht ersetzt den vorherigen, inzwischen als umgesetzt dokumentierten Speicherbericht. Dessen Ergebnis bleibt in `docs/verification-storage-company-manager.json` und der Storage-Dokumentation erhalten.

## Problem und Auswirkung

`dctx0 verify inspect` lehnt Eingabedateien über 1.048.576 Bytes ab. Ein vollständiger `ProgramInspection` bindet seine Beurteilung an den Implementierungs-Hashvektor des nativen Laufs. Beim ersten hier untersuchten Company-Manager-Lauf enthielt dieser 543 Implementierungsartefakte; ein einzelner vollständiger Review benötigte bereits rund 65 KiB. 19 Runner-Pflichten beziehungsweise 47 Knowledge-Pflichten passen zusammen nicht in die erlaubte Eingabe.

Die nativen Fälle können alle bestehen und die Provenienz vollständig sein, während die nötigen Quellreviews technisch nicht gemeinsam eingereicht werden können. Das ist eine andere Begrenzung als die bereits aufgehobene Zahl generierter Testfälle. Nicht jeder offene Company-Manager-Punkt entsteht dadurch: Dort gibt es auch echte Implementierungslücken und veraltete Reviews, die unabhängig behoben werden müssen.

## Minimaler reproduzierbarer Nachweis

Aus `../company-manager` ausführen. Der Datensatz ist gültiges JSON (führender Leerraum und ein leeres Array), enthält keine behaupteten PASS-Ergebnisse und überschreitet die Grenze um zwei Bytes:

```sh
python3 - <<'PYCODE'
from pathlib import Path
Path('/tmp/dctx0-inspection-size-probe.json').write_bytes(b' ' * 1_048_576 + b'[]')
PYCODE

dctx0 verify inspect spec <native-run-id> --project . \
  --file /tmp/dctx0-inspection-size-probe.json --json
```

Tatsächlich: Exit **1**, `Inspection input exceeds 1 MiB`, vor JSON-Parsing und Laufprüfung. Für diesen reinen Größen-Nachweis kann `<native-run-id>` daher durch einen beliebigen Platzhalter ersetzt werden. Für die fachliche Akzeptanz unten ist ein echter vollständiger nativer Lauf erforderlich.

Die bereits erfassten lokalen Nachweise liegen unter:

- `../company-manager/.artifacts/verification/dctx0-audit/inspection-size-probe.json`
- `../company-manager/.artifacts/verification/dctx0-audit/inspection-size-probe.log`
- `../company-manager/docs/dctx0-inspection-limit.md`

## Bestätigte Ursachen

1. `crates/dctx0-cli/src/verification.rs`, `VerifyAction::Inspect`: `take(1_048_577)` und anschließende Prüfung `bytes.len() > 1_048_576` mit obiger Fehlermeldung.
2. `VerifyAction::Attest` im selben Modul enthält ebenfalls eine 1-MiB-Grenze. Deren Größenproblem ist im Code bestätigt; ein großer echter Attestation-Datensatz wurde hier nicht separat ausgeführt.
3. Einfaches Aufteilen in mehrere Inspect-Aufrufe führt derzeit nicht zu einem kumulierten Gesamtergebnis: `crates/dctx0-engine/src/verification_run/program_inspection.rs` akzeptiert als Basis ausschließlich einen vollständigen, passenden Lauf mit `VerificationRunOrigin::Native`. Der abgeleitete Inspektionslauf erhält `VerificationRunOrigin::Imported`. Ein Folgeaufruf auf diesem abgeleiteten Lauf wird abgewiesen; mehrere unabhängige Aufrufe auf denselben nativen Ursprung vereinigen ihre Inspektionen nicht.

Die Herkunftsprüfung schützt korrekt die Evidenz. Sie sollte nicht einfach entfernt werden, um diese Grenze zu umgehen.

## Gewünschte Korrektur

Vollständige, quellgebundene Review-Gruppen müssen verarbeitet werden können. Eine ausreichend große kontrollierte oder konfigurierbare Eingabegrenze wäre eine direkte Korrektur. Alternativ kann das Format den unveränderten Implementierungsvektor einmal pro Gruppe binden und Wiederholungen vermeiden. CLI, Engine, öffentliche Verträge und gegebenenfalls `verify complete`/Attest gemeinsam auf vergleichbare Grenzen prüfen.

Falls kumulierte Einreichung vorgesehen wird, braucht sie eine explizite gemeinsame Basis und verständliche Konfliktregeln. Fremde, veraltete oder widersprüchliche Reviews dürfen nicht stillschweigend zusammengeführt werden. Keine Lockerung der Bindung an Snapshot, Plan, Implementierung, erforderliche Fälle oder native Beobachtungen; kein automatisches PASS für nicht beurteilte Pflichten. Keine Abwärtskompatibilität erforderlich.

## Akzeptanz und Tests

- Eine gültige Gruppe über 1 MiB gegen einen echten nativen Lauf vollständig verarbeiten; die Resultate sämtlicher eingereichter Pflichten erscheinen im effektiven Ergebnis.
- Repräsentative Gruppen von mindestens 19 und 47 vollständigen Inspektionen mit großem Quellvektor prüfen. Unbeurteilte Pflichten bleiben UNKNOWN.
- Ein nativer FAIL, fehlende/duplizierte Fälle und unvollständige Provenienz werden durch Reviews niemals zu PASS.
- Abweichende Quellhashes, Snapshot-/Planbindung, unbekannte Pflichten und widersprüchliche doppelte Beurteilungen weiterhin ablehnen.
- Ungültiges JSON und eine etwaige neue Ressourcenobergrenze verständlich und ohne teilweise Veröffentlichung zurückweisen; Speicherverbrauch mit einer realistischen großen Gruppe messen.
- Falls Kumulation implementiert wird: getrennte Einreichungen ergeben nachweislich denselben effektiven Stand wie eine gemeinsame Gruppe; falsche Eltern/Scopes und veraltete Quellen werden abgewiesen.
- Eine reine Erhöhung der Grenze nicht als fachlichen Review der Company-Manager-Lücken ausgeben.

## Abgrenzung: Bun-Pipe-Fehler im Verbraucher bereits behoben

Company Manager exportiert für Knowledge rund 240 MB natives JSON. Unter Bun 1.3.2 brach das Einlesen über `ReadableStream`/`Response.text()` mit `#getInternalBuffer`/`#pull` und `RangeError: Out of memory` ab. Der Verbraucher schreibt stdout jetzt zuerst vollständig in eine private Datei und liest erst nach Prozessende; die Übersicht behält alle Fall-IDs, Urteile und Provenienzfelder, während vollständige Eingaben/Beobachtungen in den Rohberichten verbleiben. Ein Prozesstest prüft unveränderte Bytes und den tatsächlichen Exitcode. Der Knowledge-Lauf bestand danach 10.657/10.657 Fälle ohne diesen Abbruch.

Dies ist **kein nachgewiesener dctx0-Engine-Fehler** und kein Anlass, native Evidenz zu kürzen. Optional kann ein kompakter, explizit auf vollständige Rohdaten verweisender CLI-Ausgabemodus die Verbraucher entlasten. Der frühere SQLite-Speicherbefund wird hier nicht erneut als ungelöster Fehler behauptet.

## Umsetzung im Release-Kandidaten 0.2.5

Die 1-MiB-Grenzen in Inspect, Attest und Complete wurden durch einen gemeinsamen
streamenden Reader mit 64 MiB Standard und `--max-input-bytes` ersetzt. Vollständige
Gruppen werden gegen dieselbe native Basis verarbeitet; es gibt keine implizite
Kumulation und keine Lockerung der Herkunftsprüfung. Ungültige Bindungen und
doppelte Pflichten werden vor Veröffentlichung abgewiesen.

Die lokale Release-Abnahme verarbeitet 19 beziehungsweise 47 quellgebundene
Reviews mit mindestens 543 Implementierungsartefakten. Ihre JSON-Dateien messen
1.497.179 und 3.703.635 Bytes; Laufzeiten/RSS liegen bei 93,13 ms/21.987.328 Bytes
und 122,58 ms/30.572.544 Bytes. Eine unbeurteilte Pflicht bleibt UNKNOWN. Tests
für falsche Quellen/Snapshots/Pläne, unbekannte und widersprüchliche doppelte
Pflichten, native FAILs, fehlende/doppelte Fälle, unvollständige Provenienz,
ungültiges JSON und Ressourcenobergrenzen bestehen. Die separaten Complete-Tests
verwenden ebenfalls Notizen über 1 MiB.

Details und reproduzierbare Befehle stehen in
[Large source-bound review inputs](docs/site/src/reference/inspection-inputs.md).
Dies behebt die Transportgrenze, bewertet aber keine Company-Manager-Lücke.
Die erneute vollständige CI und das Release sind noch ausstehend.
