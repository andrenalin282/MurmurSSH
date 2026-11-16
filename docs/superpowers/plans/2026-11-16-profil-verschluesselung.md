# Profil-Verschlüsselung Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Gespeicherte SSH-/FTP-Passwörter in Profilen werden mit einem lokal gespeicherten, portablen symmetrischen Schlüssel verschlüsselt, sodass geleakte Profildateien ohne den Schlüssel wertlos sind.

**Architecture:** Neues `crypto_service` (AES-256-GCM) ver-/entschlüsselt gespeicherte Geheimwerte transparent an den bestehenden Schreib-/Lesestellen (`save_credential`, `connect_sftp`). Gespeicherte Werte tragen das Präfix `enc:v1:`; fehlendes Präfix = Legacy-Klartext (abwärtskompatibel). Schlüssel = 32 Byte in eigener 0600-Datei, Pfad in `Settings`, verwaltet über die Settings-UI.

**Tech Stack:** Rust (Tauri 2), Crate `aes-gcm` 0.10; Vanilla TypeScript Frontend.

**Spec:** `docs/superpowers/specs/2026-11-16-profil-verschluesselung-design.md`

## Global Constraints
- Rückwärtskompatibel: Profile ohne Schlüssel und Klartext-Secrets funktionieren unverändert.
- SSH-Key-Passphrasen werden NIE gespeichert/verschlüsselt (bestehende Regel in `save_credential`/`connect_sftp` bleibt).
- Private Dateien 0600, atomar — über bestehendes `fs_secure` (`write_private`, `private_dir_all`).
- Gespeichertes Ciphertext-Format exakt: `enc:v1:` + base64(nonce ‖ ciphertext+tag), nonce = 96 Bit aus OsRng, pro Wert neu.
- Schlüssel-Rohwert NIE in `settings.json` (nur Pfad/Status dort).
- i18n-Keys in allen 6 Locales (`src/i18n/{en,de,fr,nl,pl,ru}.ts`).
- Build grün: `cargo build`, `cargo clippy -D warnings`, `cargo test --lib`, `npx tsc`, `npx vite build`. (Dev-Libs fehlen auf dem Z790 → Build/Test im Docker-Image `murmur-builder`; `touch` der Quellen vor cargo wegen lc-mtime.)

## Review Focus
- **Ciphertext ohne Schlüssel** (frisch importiertes Profil, Schlüssel fehlt): `connect_sftp` muss klar `NEED_ENCRYPTION_KEY` melden statt zu crashen oder stumm nach Passwort zu fragen → Task 3.
- **Manipulierte/defekte Ciphertext-Daten** (GCM-Tag-Fehler): `decrypt` gibt Fehler zurück, überschreibt nichts → Task 1.
- **Doppelte Verschlüsselung** vermeiden: bereits `enc:v1:`-Werte werden bei „alle verschlüsseln" übersprungen (Idempotenz) → Task 4.
- **Importierter Schlüsselwert falscher Länge/Base64:** `import_key` lehnt ab, keine kaputte Keydatei → Task 1.
- **Nonce-Wiederverwendung:** zwei Verschlüsselungen desselben Klartexts ergeben unterschiedliche Ciphertexte → Task 1.

---

### Task 1: crypto_service — Schlüssel & AEAD

**Files:**
- Create: `src-tauri/src/services/crypto_service.rs`
- Modify: `src-tauri/src/services/mod.rs` (pub mod crypto_service), `src-tauri/src/models/settings.rs` (Feld `encryption_key_path`), `src-tauri/Cargo.toml` (`aes-gcm = "0.10"`)
- Test: inline `#[cfg(test)] mod tests` in `crypto_service.rs`

**Interfaces:**
- Produces:
  - `pub fn key_path() -> PathBuf` (Settings `encryption_key_path` oder Default `~/.config/murmurssh/master.key`)
  - `pub fn is_enabled() -> bool` (Keydatei existiert & lesbar)
  - `pub fn create_key() -> Result<String, String>` (Fehler wenn schon vorhanden; schreibt 0600; liefert base64-Wert)
  - `pub fn export_key() -> Result<String, String>`
  - `pub fn import_key(value: &str) -> Result<(), String>` (base64 → genau 32 Byte, sonst Err; schreibt 0600)
  - `pub fn remove_key_file() -> Result<(), String>`
  - `pub fn is_ciphertext(s: &str) -> bool` (`s.starts_with("enc:v1:")`)
  - `pub fn encrypt(plaintext: &str) -> Result<String, String>` (Err wenn kein Schlüssel)
  - `pub fn encrypt_if_enabled(plaintext: &str) -> String` (Klartext zurück wenn kein Schlüssel)
  - `pub fn decrypt(stored: &str) -> Result<String, String>` (kein Präfix → Wert unverändert; Präfix ohne Schlüssel → Err; Präfix mit Schlüssel → entschlüsselt)

- [ ] **Step 1: Failing tests** in `crypto_service.rs`:
  - `encrypt_then_decrypt_round_trips` (Schlüssel via Test-Helper setzen, `decrypt(&encrypt("pw")?)? == "pw"`)
  - `decrypt_passes_through_plaintext` (`decrypt("plainpw")? == "plainpw"`)
  - `decrypt_ciphertext_without_key_errs`
  - `two_encryptions_differ` (`encrypt("pw") != encrypt("pw")`, beide entschlüsseln zu "pw")
  - `import_rejects_wrong_length`
  - `tampered_ciphertext_errs` (ein Byte im base64-Teil kippen → `decrypt` Err)
  - Test-Helper setzt `encryption_key_path` auf eine Temp-Datei (z. B. per `std::env` Override oder eine `#[cfg(test)]`-settbare statische Pfadvariable).

- [ ] **Step 2: Run** `cargo test --lib crypto_service` → FAIL (nicht definiert).
- [ ] **Step 3: Implementieren.** `Aes256Gcm` (KeyInit aus 32-Byte-Key), Nonce `Aes256Gcm::generate_nonce(&mut OsRng)`, `encrypt` → base64(nonce‖ct) mit Präfix; `decrypt` zerlegt Präfix, splittet 12-Byte-Nonce, `aead decrypt`. Keydatei lesen/schreiben über `fs_secure`.
- [ ] **Step 4: Run** `cargo test --lib crypto_service` → PASS.
- [ ] **Step 5: Commit** `feat(crypto): AES-256-GCM crypto_service + Schlüsselverwaltung`.

---

### Task 2: Transparente Ver-/Entschlüsselung an den Secret-Stellen

**Files:**
- Modify: `src-tauri/src/commands/connection.rs` (`save_credential`, `connect_sftp`), ggf. `src-tauri/src/services/secrets_service.rs`
- Test: inline tests in `connection.rs` für die reine Auflösungs-Logik

**Interfaces:**
- Consumes: `crypto_service::{encrypt_if_enabled, decrypt, is_ciphertext, is_enabled}` (Task 1).
- Produces: `fn resolve_stored_secret(mode, portable: Option<String>, pid: &str) -> Result<Option<String>, String>` (liefert entschlüsselten Klartext; Err `"NEED_ENCRYPTION_KEY"` wenn geladener Wert Ciphertext ist und kein Schlüssel gesetzt).

- [ ] **Step 1: Failing tests** für `resolve_stored_secret`:
  - `returns_plaintext_passthrough` (Legacy-Klartext in portable → gleicher Wert)
  - `decrypts_when_key_set`
  - `errs_need_key_when_ciphertext_and_no_key` (`Err("NEED_ENCRYPTION_KEY")`)
  - `returns_none_for_never`
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implementieren.** `save_credential`: Secret vor dem Speichern durch `encrypt_if_enabled` (für `local_machine` UND `portable_profile`). `connect_sftp` (Password- und FTP-Zweig): Loader über `resolve_stored_secret`; bei dessen Err wird dieser aus `connect_sftp` zurückgegeben (vor `test_connection`).
- [ ] **Step 4: Run** `cargo test --lib connection` → PASS.
- [ ] **Step 5: Commit** `feat(crypto): Secrets beim Speichern verschlüsseln, beim Verbinden entschlüsseln`.

---

### Task 3: Nachträglich verschlüsseln / zurück-entschlüsseln (Migration)

**Files:**
- Modify: `src-tauri/src/services/crypto_service.rs` (Migrationsfunktionen) — oder neue `src-tauri/src/services/crypto_migrate.rs`
- Test: inline tests

**Interfaces:**
- Consumes: `profile_service::{list_profiles, save_profile}`, `secrets_service` (Dateiliste), `crypto_service::{encrypt, decrypt, is_ciphertext}`.
- Produces:
  - `pub fn encrypt_all_existing() -> Result<usize, String>` (jedes `stored_secret_portable` + jede `secrets/<id>`-Datei, nur Klartext, in Ciphertext; `enc:v1:` übersprungen; Backup via `save_profile`/`write_private`)
  - `pub fn decrypt_all_existing() -> Result<usize, String>` (Umkehrung; für „Schlüssel entfernen")

- [ ] **Step 1: Failing tests:** `encrypt_all_idempotent` (zweiter Lauf ändert nichts / verschlüsselt nicht doppelt), `encrypt_then_decrypt_all_restores_plaintext`. (Temp-Profilordner + Temp-Secrets über settbare Pfade.)
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implementieren** (Idempotenz: `is_ciphertext` prüfen, überspringen).
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `feat(crypto): bestehende Secrets nachträglich ver-/entschlüsseln`.

---

### Task 4: Tauri-Commands + Registrierung

**Files:**
- Create: `src-tauri/src/commands/encryption.rs`
- Modify: `src-tauri/src/commands/mod.rs`, `src-tauri/src/lib.rs` (generate_handler!)
- Test: minimaler lib-Test oder manuell via Frontend (Commands sind dünne Wrapper)

**Interfaces (alle `#[tauri::command]`):**
- `encryption_status() -> EncryptionStatus { enabled: bool, key_path: String }`
- `create_encryption_key() -> Result<String, String>` (= `crypto_service::create_key()` + `crypto_migrate::encrypt_all_existing()`; liefert base64-Wert zum Anzeigen)
- `export_encryption_key() -> Result<String, String>`
- `import_encryption_key(value: String) -> Result<(), String>`
- `remove_encryption_key() -> Result<(), String>` (= `decrypt_all_existing()` dann `remove_key_file()`)

- [ ] **Step 1:** Commands implementieren, in `mod.rs` + `lib.rs` registrieren.
- [ ] **Step 2: Run** `cargo build && cargo clippy -D warnings && cargo test --lib` → grün.
- [ ] **Step 3: Commit** `feat(crypto): Encryption-Commands + Registrierung`.

---

### Task 5: Frontend — API, Settings-UI, i18n, Connect-Fehlerpfad

**Files:**
- Modify: `src/api/index.ts`, `src/components/settings-dialog.ts`, `src/main.ts` (NEED_ENCRYPTION_KEY), `src/i18n/{en,de,fr,nl,pl,ru}.ts`, `src/types.ts`

**Interfaces:**
- Consumes: Task-4-Commands.
- `api.encryptionStatus()`, `api.createEncryptionKey()`, `api.exportEncryptionKey()`, `api.importEncryptionKey(value)`, `api.removeEncryptionKey()`.

- [ ] **Step 1:** API-Wrapper in `index.ts` + `EncryptionStatus`-Typ.
- [ ] **Step 2:** Settings-Dialog: Abschnitt „Verschlüsselung" — Status; **Schlüssel anlegen** (zeigt Wert in kopierbarem Feld + Warnung „sicher aufbewahren, Verlust = Passwörter neu eingeben"); **Wert anzeigen**; **Importieren** (Einfügefeld); **Entfernen** (Bestätigung = Downgrade zu Klartext).
- [ ] **Step 3:** `main.ts` `verifyConnection()`: `NEED_ENCRYPTION_KEY` → klare Meldung „Verschlüsselungs­schlüssel importieren (Settings → Verschlüsselung)".
- [ ] **Step 4:** i18n-Keys (`settings.encryption*`, `encryption.*`) in allen 6 Locales.
- [ ] **Step 5: Run** `npx tsc && npx vite build` → grün; `.js`-Geschwister committen.
- [ ] **Step 6: Commit** `feat(crypto): Settings-UI + Connect-Fehlerpfad für Verschlüsselung`.

---

### Task 6: README + CHANGELOG

**Files:** Modify `README.md`, `CHANGELOG.md`

- [ ] **Step 1:** CHANGELOG `[Unreleased] → Added`: Profil-Verschlüsselung (AES-256-GCM, portabler Schlüssel, Settings → Verschlüsselung, nachträgliche Verschlüsselung).
- [ ] **Step 2:** README: kurzer Abschnitt bei den Credential-Storage-Modi — verschlüsselt, Schlüssel-Export/-Import, Bedrohungsmodell (geleaktes Profil ohne Schlüssel wertlos).
- [ ] **Step 3: Commit** `docs: Verschlüsselung in README + CHANGELOG`.

---

## Nach dem Plan (Kais Aufträge, kein Code)
- **Fable-Sicherheitsreview** über den fertigen Krypto-/Credential-Pfad (`ma-security-review`/Fable-Modell).
- **Programmbeschreibung** im CrispyInfoSpace `projekte/murmurssh/…-funktionen.md` (Funktionsübersicht inkl. Verschlüsselung; Stand/Quelle/Version).
