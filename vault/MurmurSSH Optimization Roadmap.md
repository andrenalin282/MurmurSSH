# MurmurSSH Optimization Roadmap

Quelle: `docs/superpowers/specs/2026-06-10-optimization-roadmap-design.md`
Pläne: `docs/superpowers/plans/` (pro Phase, just-in-time erstellt)

Ausführung: subagent-driven (frischer Subagent pro Task, Spec- + Quality-Review je Task, Modellzuweisung je Task zur Token-Ersparnis). Direkt auf `main`, Commit pro Schritt, Version-Bump + Tag + Push am Phasenende.

## Phasen (Reihenfolge nach Abhängigkeit/Risiko)

| Phase | Inhalt | Status |
|-------|--------|--------|
| 0 | Bugfixes: Edit-Flow (Hash-Baseline statt mtime) + Cleanup beim Fenster-Schließen | ✅ done — v1.4.7 |
| 1 | Dateiliste: Änderungsdatum-Spalte + Rechte-Spalte + chmod-Dialog (Häkchen-Grid ↔ Oktal) | ✅ done — v1.4.8 |
| 2 | Transfer-Background-Queue mit konfigurierbarer Parallelität (Multiverbindung), Queue-UI | ✅ done — v1.5.0 |
| 3 | Profil-Gruppen (`group` + `created_at`), Baum-Ansicht, Sortierung alphabetisch/Erstellungsdatum | ✅ done — v1.6.0 |
| 4 | FileZilla-Import (sitemanager.xml → Profile + Gruppen, ohne Passwörter) | pending |
| 5 | Editor-Konfiguration: globaler Default + Pro-Dateityp-Map | pending |

## Locked Decisions

- Transfer: In-App-Queue (kein Sidecar-Prozess).
- Rechte-UI: rwx-Häkchen-Grid synchron mit Oktal-Textfeld.
- Gruppen: `group`-Feld + Baum-Ansicht, Sortierung pro Gruppe.
- Editor: globaler Default + Endung→Editor-Map; Profil-`editor_command` überschreibt.
- „Erstellungsdatum" in Dateiliste = **Änderungsdatum** (echte Creation-Time über SFTP v3 nicht verfügbar).
- FileZilla-Import: gespeicherte Passwörter werden NICHT importiert (Sicherheit).

## Phase 0 — abgeschlossen (v1.4.7, 2026-06-10)

Root-Cause B1 (korrigiert ggü. erster Annahme): `open_for_edit` lädt bei jedem Edit-Klick neu herunter → überschreibt lokale Datei → mtime springt → laufender Watcher feuert `upload-ready` ohne echte Änderung. Multi-Write-Saves (temp+rename) → doppelte Bestätigung. Fix: Content-Hash-Baseline-Registry (`OnceLock<Mutex<HashMap<PathBuf,String>>>`), geteilt zwischen Download und Watcher; Watcher reagiert nur bei Hash-Abweichung.

B2: `ExitRequested`-Hook in `lib.rs` (`.build().run(...)`) ruft `cleanup_on_exit()` → `stop_all_sessions()` + `cleanup_all_runtime_keys()` + `clear_all()`.

Commits: 3987f65, 61adf22, ef8f416, 1e918de, a7e8b2d (B1), ef6e082 (B2), + docs/release; Tag `v1.4.7` (dac910b). Tests 4/4, clippy clean, Final-Review: SHIP.

### Bekannte Einschränkung
- Confirm-Modus: App vor Bestätigung eines ausstehenden Uploads schließen verwirft diese Änderung (Designcharakteristik, kein Regression). In CHANGELOG vermerkt.

### Offene manuelle Verifikation (braucht echten Server, vom Nutzer durchzuführen)
1. Edit → ändern → speichern → genau **1** Upload/Prompt. Editor schließen → erneut Edit → frischer Remote-Inhalt, **kein** Prompt ohne Änderung.
2. Editor mit temp+rename (gedit/VS Code): ein Speichern → genau **1** Bestätigung.
3. Fenster per OS-X schließen → keine Runtime-Keys in `~/.config/murmurssh/secrets/`, keine ControlMaster-Sockets übrig.

## Phase 1 — abgeschlossen (v1.4.8, 2026-06-10)

`FileEntry` um `perm: Option<u32>` erweitert (aus SFTP `stat.perm`; FTP/local = None). Neuer `set_permissions`-Command: perm-erhaltendes `setstat` (statet zuerst, ODERt Typ-Bits `& 0o170000` mit `mode & 0o7777` → Ordner bleibt Ordner); FTP → „nicht unterstützt". Dateibrowser: Spalten „Geändert" (Locale-Datum) + „Rechte" (symbolisch + Oktal-Tooltip), Tabelle jetzt 4 Spalten. `showPermissionsDialog`: rwx-Häkchen-Grid ↔ Oktal-Feld, bidirektional; Apply = Grid-Wert. Kontextmenü (Datei + Ordner) → „Rechte ändern…". i18n in allen 6 Sprachen. Final-Review (Opus): SHIP. cargo/clippy/tsc/vite grün.

**Wichtig (Build-Konvention entdeckt):** Frontend hat getrackte `.js`-Geschwister (tsc emittiert in-place, Vite lädt `.ts`). Regel: `.ts` editieren → `npx tsc` → beide committen. Gilt für alle weiteren Frontend-Phasen.

### Offene manuelle Verifikation (echter Server)
1. Liste zeigt Geändert-Datum + Rechte-Spalte (z.B. `-rw-r--r--`, Oktal im Tooltip).
2. Rechtsklick Datei → „Rechte ändern…": Häkchen ⇄ Oktal synchron; Apply ändert Remote-Rechte; Spalte aktualisiert nach Refresh.
3. FTP-Profil: „Rechte ändern…" zeigt sauber den „nicht unterstützt"-Fehler.

## Phase 2 — abgeschlossen (v1.5.0, 2026-06-11)

Neuer `transfer_queue`-Service: ein Dispatcher-Thread (`Mutex<QueueState>` + `Condvar`) befördert Queued→Active-Jobs bis `max_concurrent_transfers` (Settings, Default 2, Clamp 1–8) und startet je Job einen Worker-Thread mit eigener SFTP/FTP-Session → echte Parallelität. Per-Job-Abbruch via `Arc<AtomicBool>` ersetzt das gelöschte profil-gekoppelte `transfer_cancel`; Transfer-Funktionen nehmen jetzt `cancel: &dyn Fn() -> bool` (Edit-Flow übergibt `&|| false`). Neues Modell `models/transfer.rs`, neue Commands `commands/transfer.rs` (`enqueue_transfer`/`cancel_transfer(job_id)`/`cancel_all_transfers`/`list_transfers`/`clear_finished_transfers` + `local_path_is_dir`); alte Channel-basierte Transfer-Commands entfernt. Queue emittiert `transfer-update`-Events; `init(&app)` in `lib.rs` `.setup`, `cancel_all()` in `cleanup_on_exit`. Frontend: neues `TransferQueuePanel` (In-Place-Row-Updates, voller Re-Render nur bei Strukturänderung), File-Browser reiht Transfers ein statt Einzelbalken, Settings-Eingabe für Parallelität, i18n in 6 Sprachen. Dispatcher-Korrektheit (Opus-Review): Settings-Read + Emit außerhalb des Locks, prädikat-geschütztes `cv.wait` (kein Lost-Wakeup), Worker-Panic-Guard. Final-Review (Opus) fand + behob eine Lost-Wakeup-Regression → SHIP. cargo/clippy/tsc/vite grün, 9 Lib-Tests.

### Offene manuelle Verifikation (echter Server)
1. 10+ Dateien einreihen: UI bleibt responsiv; max. `max_concurrent_transfers` laufen gleichzeitig, Rest „Warteschlange" und startet, sobald Slots frei werden.
2. Per-Job-✕ bricht nur diesen Job ab (andere laufen weiter); „Alle abbrechen" stoppt alles; abgebrochene Jobs zeigen „Abgebrochen", nicht „Fehlgeschlagen".
3. Fehlschlagender Transfer (z.B. Permission denied) → „Fehlgeschlagen" mit Fehler, ohne Geschwister-Jobs abzubrechen.
4. Ordner-Upload/-Download = ein einzelner Job, Dateiname aktualisiert sich.
5. Overwrite-Dialog erscheint weiterhin VOR dem Einreihen.
6. Nach Abschluss: Remote-Browser aktualisiert nach Upload, lokaler nach Download.
7. App per OS-X schließen während laufender Transfers → sauberer Teardown (`cancel_all()` beim Exit), keine Geister-Sockets.
8. „Gleichzeitige Übertragungen" in Settings ändern → wirkt auf die nächste Charge (Anhebung sofort via `notify()`).

## Phase 3 — abgeschlossen (v1.6.0, 2026-06-11)

`Profile` um `group: Option<String>` + `created_at: Option<u64>` erweitert (rückwärtskompatibel, serde `skip_serializing_if`). `save_profile` stempelt `created_at` einmalig (bei Edit erhalten, da zuerst der On-Disk-Wert gelesen wird); `list_profiles` UND `get_profile` füllen `created_at` für Alt-Profile aus der Datei-mtime nach (nur im Speicher, kein Rückschreiben) — `get_profile` ist Single Source of Truth, sodass das Bearbeiten eines Alt-Profils sein ursprüngliches Datum behält statt es auf „jetzt" zu setzen. Neues Settings-Feld `profile_sort` ("name"|"created"). Frontend: Profil-Selector vom `<select>` zur einklappbaren Gruppen-Baumansicht (Gruppen-Header mit Caret/Name/Anzahl, Klick klappt ein/aus, Session-State; Zeilen: Einfachklick wählt, Doppelklick verbindet; „Ohne Gruppe" zuletzt). Persistierter Sortier-Umschalter (A–Z | Neueste) via read-merge-write. Öffentliche API + Button-IDs unverändert → main.ts ohne Änderung. Profil-Formular: `group`-Textfeld mit `<datalist>` bestehender Gruppen; `show()` jetzt async zum Vorladen; `created_at` beim Speichern durchgereicht. i18n in 6 Sprachen. Final-Review (Opus) fand + behob das Zurücksetzen von `created_at` bei Alt-Profilen (get_profile-Backfill) → SHIP. cargo/clippy/tsc/vite grün, 12 Lib-Tests.

### Offene manuelle Verifikation (echter Server / mehrere Profile)
1. Profile erscheinen gruppiert; ohne Gruppe unter „Ohne Gruppe" (zuletzt).
2. Gruppen-Header klappt ein/aus (Caret wechselt); Zustand bleibt während der App-Sitzung.
3. Zeile anklicken wählt (Hervorhebung); Edit/Delete/Connect aktiv; Doppelklick verbindet.
4. Sortier-Umschalter A–Z ↔ Neueste ordnet je Gruppe um und überlebt App-Neustart.
5. Gruppe im Formular setzen → Profil wandert nach Speichern in diese Gruppe; leeren → zurück zu „Ohne Gruppe".
6. Neues Profil bekommt Erstellungsdatum (Neueste oben); Alt-Profile sortieren via mtime; Bearbeiten eines Alt-Profils setzt das Datum NICHT zurück.
7. Connect/Edit/Delete bleiben bei aktiver Verbindung gesperrt.

## Notizen

- Pläne für Phase 1–5 werden je vor Ausführung erstellt (Phase 4 hängt am Datenmodell aus Phase 3).
- Modellzuweisung: Haiku = mechanisch (i18n, Docs, einfache Spalten), Sonnet = Implementierung, Opus = Architektur/Root-Cause/Final-Review.
