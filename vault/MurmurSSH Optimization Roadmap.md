# MurmurSSH Optimization Roadmap

Quelle: `docs/superpowers/specs/2026-06-10-optimization-roadmap-design.md`
Pläne: `docs/superpowers/plans/` (pro Phase, just-in-time erstellt)

Ausführung: subagent-driven (frischer Subagent pro Task, Spec- + Quality-Review je Task, Modellzuweisung je Task zur Token-Ersparnis). Direkt auf `main`, Commit pro Schritt, Version-Bump + Tag + Push am Phasenende.

## Phasen (Reihenfolge nach Abhängigkeit/Risiko)

| Phase | Inhalt | Status |
|-------|--------|--------|
| 0 | Bugfixes: Edit-Flow (Hash-Baseline statt mtime) + Cleanup beim Fenster-Schließen | ✅ done — v1.4.7 |
| 1 | Dateiliste: Änderungsdatum-Spalte + Rechte-Spalte + chmod-Dialog (Häkchen-Grid ↔ Oktal) | pending |
| 2 | Transfer-Background-Queue mit konfigurierbarer Parallelität (Multiverbindung), Queue-UI | pending |
| 3 | Profil-Gruppen (`group` + `created_at`), Baum-Ansicht, Sortierung alphabetisch/Erstellungsdatum | pending |
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

## Notizen

- Pläne für Phase 1–5 werden je vor Ausführung erstellt (Phase 4 hängt am Datenmodell aus Phase 3).
- Modellzuweisung: Haiku = mechanisch (i18n, Docs, einfache Spalten), Sonnet = Implementierung, Opus = Architektur/Root-Cause/Final-Review.
