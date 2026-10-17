# Übermäßiger Speicherverbrauch der lokalen Prüfhistorie

**Stand 17.10.2026:** Speichermodell und Bereinigung sind umgesetzt. Die ursprünglichen Messungen stehen unter „Befund“, das Ergebnis unter „Umsetzung“.

## Befund

Untersucht am 17.10.2026 anhand von
`../company-manager/spec/.dctx0ctl/state.sqlite3` und dem Speichercode dieses
Repositories. Die Datenbank belegt 19.684.085.760 Bytes, also rund 19,68 GB
(18,33 GiB). Die folgenden Messungen stammen aus lesenden Abfragen; Datenbank und
Implementierung wurden nicht verändert.

| Bereich                              | Umfang                  | Belegte SQLite-Seiten inklusive Overhead |
| ------------------------------------ | ----------------------- | ---------------------------------------: |
| `verification_runs`                  | 400 Läufe               |                     13.653.172.224 Bytes |
| `verification_artifacts`             | 94.458 Artefakte        |                      3.568.537.600 Bytes |
| `snapshots`                          | 104 Snapshots           |                      1.041.354.752 Bytes |
| `verification_run_artifacts`         | 2.357.478 Verknüpfungen |                        335.142.912 Bytes |
| Index der Run-Artefakt-Verknüpfungen |                         |                        390.909.952 Bytes |

Die übrige Belegung entfällt auf Taskhistorie, Graphdaten und weitere Indizes.
`PRAGMA freelist_count` ergab 0: Die Größe entsteht durch gespeicherte Inhalte,
nicht durch bereits gelöschte Datensätze. Ein `VACUUM` allein löst das Problem
nicht.

Ein besonders großer Lauf enthält 10.554 Testfälle und 12.502 Artefaktverweise.
Sein gespeichertes JSON umfasst 241.892.140 Zeichen; bei erneuter kompakter
Serialisierung entfallen rund 239,1 MB auf `cases`, 1,76 MB auf
Artefaktverweise und 0,71 MB auf Provenienz. Die Zahlen für JSON-Felder sind
Serialisierungsmessungen und nicht mit belegten SQLite-Seiten gleichzusetzen.

Eine zlib-Kompression des UTF-8-Inhalts dieses einzelnen Laufberichts ergibt
3.744.412 Bytes: rund 98,5 Prozent weniger. Das ist ein konkreter Einzelbefund,
keine gemessene Kompressionsrate für die gesamte Datenbank.

Zusätzlich liegen im Company-Manager-Projekt rund 3,4 GiB exportierte
Prüfberichte unter `.artifacts/verification/`. Diese sind nicht Teil der
Datenbankbelegung, verstärken aber den gesamten Speicherverbrauch.

## Ursachen im Code

- `crates/dctx0-store/src/verification.rs`, `record_verification_run`:
  `serde_json::to_string(run)` serialisiert den vollständigen Run;
  `verification_runs.data` speichert ihn unkomprimiert als TEXT.
- Dieselbe Funktion speichert Artefaktinhalte unkomprimiert als BLOB in
  `verification_artifacts`. Artefakte werden anhand ihres Inhalts-Hashes
  dedupliziert; unterschiedliche vollständige Run-Berichte werden separat
  gespeichert.
- `crates/dctx0-engine/src/verification_run/execution.rs`, `record`:
  Beobachtungen, Events, Ressourcen und Zustände aus der Sitzung werden in
  `VerificationCaseRecord` übernommen und damit Teil des vollständigen Runs.
- In derselben Datei schreibt `preserve_prefix` Sitzungsevidenz zusätzlich als
  `application/vnd.dctx0.session+json`-Artefakt. Weitere Assessment-Artefakte
  enthalten ebenfalls Ausführungsergebnisse. Es gibt damit inhaltliche
  Überschneidungen zwischen Run-Bericht und Artefakten; diese sind nicht in jedem
  Fall identische oder ohne Prüfung austauschbare Darstellungen.

## Gewünschte Änderung

### Standardmäßig nur die letzten drei Runs behalten

Produktwunsch des Maintainers: Im Standard sollen nur die letzten drei
Prüfläufe gespeichert bleiben. Eine unbegrenzt wachsende lokale Historie ist
nicht erwünscht.

Die Aufbewahrung sollte pro Projekt gelten. Vor der Umsetzung ist festzulegen,
ob die drei letzten Läufe insgesamt genügen oder ob unterschiedliche Prüfscopes
getrennte Aufbewahrung benötigen. Ebenso müssen relevante Tasknachweise,
Adoptionen und Replay-Verweise berücksichtigt werden: Eine Bereinigung darf
keine weiter gültigen Nachweise stillschweigend unlesbar machen oder einen
historischen PASS ohne seine Evidenz als weiterhin belegbar darstellen.

Nach erfolgreicher Veröffentlichung eines neuen Runs sollen ältere, gemäß der
festgelegten Regel entbehrliche Runs samt Verknüpfungen entfernt werden.
Artefakte dürfen erst entfernt werden, wenn sie von keinem verbleibenden
Nachweis mehr benötigt werden. Die Bereinigung muss konsistent und bei
Unterbrechungen wiederholbar sein. Nach dem Löschen muss auch geklärt sein, wie
bereits belegter Dateispeicher tatsächlich zurückgegeben wird.

### Zusätzliche vollständige Speicherung des Runs hinterfragen

Produktfrage des Maintainers: Muss der vollständige Run noch einmal separat
persistiert werden, wenn die zugrunde liegende Evidenz bereits in Artefakten
liegt?

Prüfen, ob ein kompakter Run-Datensatz mit Identität, Provenienz, Scope,
Gesamtergebnis und Artefaktverweisen genügt und Detailberichte bei Bedarf aus
kanonischer Evidenz aufgebaut werden können. Dabei konkret feststellen, welche
Informationen heute ausschließlich im Run stehen und welche tatsächlich doppelt
vorliegen. Erforderliche Informationen für History, Show, Replay und
Tasknachweise müssen erhalten bleiben; keine pauschale Entfernung des
Run-Inhalts ohne diesen Abgleich.

Falls eine vollständige zusätzliche Run-Darstellung erforderlich bleibt, ihre
Notwendigkeit dokumentieren und sie komprimiert speichern. Auch für große
Artefakte Kompression prüfen. Inhalts-Hashes und bestehende Integritätsprüfungen
müssen sich weiterhin eindeutig auf die ursprünglichen Inhalte beziehen.

## Erwartete Validierung

- Mehr als drei Läufe ausführen und die vereinbarte Aufbewahrung pro Projekt
  sowie das Verhalten unterschiedlicher Scopes nachweisen.
- Gemeinsam verwendete Artefakte erhalten; tatsächlich unreferenzierte Inhalte
  bereinigen. Andere Projekte dürfen nicht betroffen sein.
- History, Show, Replay und Tasknachweise für verbleibende Runs sowie das
  verständliche Verhalten bei entfernten Runs prüfen.
- Konsistenz bei Fehlern oder Unterbrechungen der Bereinigung prüfen.
- Mit repräsentativen großen Runs den belegten Speicher vor und nach der
  Änderung messen; logische Datenmenge und tatsächliche Dateigröße getrennt
  ausweisen.

Der ursprüngliche Speicherbefund und die damaligen Änderungswünsche bleiben oben
zur Einordnung erhalten. Die Umsetzung ist unten dokumentiert.


## Umsetzung am 17.10.2026

Pro Projekt bleiben drei vollständige Runs über alle Scopes hinweg erhalten.
Ältere Runs behalten kompakte, ausdrücklich als bereinigt markierte Nachweise.
Cases werden einmal komprimiert und nach Inhalt dedupliziert gespeichert;
zusätzliche Session- und Einzel-Assessment-Diagnosen entstehen nur bei FAIL,
UNKNOWN und unvollständiger Ausführung. History-Abfragen laden Zusammenfassungen,
Taskdetails zeigen die aktuelle Verfügbarkeit ihrer historischen Run-Evidenz.

Die Company-Manager-Datenbank wurde auf ausdrücklichen Wunsch **ohne Backup**
bereinigt. Bis zur Bereinigung war sie auf 409 Runs angewachsen. Ihre Dateigröße
sank von 19.740.737.536 auf 1.764.896.768 Bytes (−91,06 %); der Exportordner sank
von 3.533.094.515 auf 262.330.742 Bytes (−92,58 %). Alle 20 übrigen Tabellen mit
1.139.345 Datensätzen haben unveränderte Inhaltsprüfsummen. Die letzten drei
Run-IDs blieben erhalten; Integritäts- und Fremdschlüsselprüfung bestehen.

Details zu Schema 5, Kompatibilität, Wartungsbefehlen und Messumfang stehen in
[Verification history storage](docs/site/src/reference/verification-storage.md).
Die konkrete Bereinigung ist in
[verification-storage-company-manager.json](docs/verification-storage-company-manager.json)
festgehalten. Vollständige entfernte Details lassen sich ohne Sicherung nicht
wiederherstellen.
