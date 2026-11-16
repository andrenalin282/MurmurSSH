# Löschen-Speed + Härtung Implementation Plan

**Goal:** Schnelles Ordner-Löschen per `rm -rf` über Exec (mit Fallback + Guards) plus Härtung/Optimierung aus dem Fable-Review (Minors).

**Spec:** Folge-Entscheidung zu docs/superpowers/specs/2026-11-16-profil-verschluesselung-design.md (Kai, 2026-11-16: Härtung + Speed + rm -rf, ohne enc:v2).

## Global Constraints
- Build/Test nur im Docker (murmur-dev). `touch` vor cargo (lc-mtime).
- clippy -D warnings, cargo test --lib grün.
- Löschen ist destruktiv: Pfad shell-quoten, nach `--`, root-/leer-Pfade ablehnen, Bestätigungsdialog bleibt.

## Tasks
### Task A: Migration lädt Schlüssel einmal (M10, Speed)
`migrate_value` nimmt `&[u8;32]` statt pro Wert load_key(); `run_migration` lädt den Schlüssel einmal. Tests pur ohne File-I/O.

### Task B: zeroize (Härtung)
`zeroize` crate; Schlüsselmaterial als `Zeroizing<[u8;32]>` (decode_key/load_key/create_key). Nullt Schlüssel beim Drop.

### Task C: rm -rf-Schnellpfad fürs Ordner-Löschen
`remote_copy::rm_rf_command(path)` = `rm -rf -- '<quoted>'`; `is_safe_delete_path` lehnt leer/"/"/"~"/"."/".." ab. `sftp_service::delete_directory`: bei SSH + exec_available + sicherem Pfad → ein `rm -rf`, sonst/bei Exec-Channel-Fehler → bestehende SFTP-Rekursion. rm-nonzero = echter Fehler. Tests: command-builder + guard (pur).

### Task D: Doku-Kommentare (M9)
secrets_service.rs Header + connection.rs save_credential: „Klartext/NOT encrypted" → Hinweis auf optionale Verschlüsselung.

## Review Focus
- Guard lässt keinen root-/leer-Pfad zu `rm -rf` durch (Task C).
- exec-Channel-Fehler → Fallback SFTP, nicht stiller Erfolg (Task C).
- migrate_value mit falschem Schlüssel → Fehler propagiert (Task A).
