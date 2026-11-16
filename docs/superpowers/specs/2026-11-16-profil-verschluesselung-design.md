# Design: Profil-Verschlüsselung mit portablem Schlüssel

Datum: 2026-11-16 · Status: Freigegeben (Kai, 2026-11-16)

## Intent / Warum
Profile sollen zwischen Geräten mitgenommen werden, ohne dass ein geleaktes Profil
(geteiltes portables Profil, Backup, Cloud-Sync des Config-Ordners) direkt nutzbar ist.
Heute liegen gespeicherte Passwörter im Klartext — im Profil-JSON (`portable_profile`) oder
in `~/.config/murmurssh/secrets/<id>`. Ziel: gespeicherte Passwörter werden mit einem
symmetrischen Schlüssel verschlüsselt; ohne diesen Schlüssel sind die Profildateien wertlos.
Der Schlüssel wird als Textwert exportiert und auf dem zweiten PC importiert.
**Primär** geht es um Profile im **benutzerdefinierten Speicherpfad** (Phase 5.4, z. B. ein
synchronisierter Ordner) — dort liegen die Profil-JSONs außerhalb von `~/.config/murmurssh/` und
leaken am ehesten. Die lokalen `secrets/`-Dateien werden ebenfalls verschlüsselt.

## Bedrohungsmodell
- **Schützt gegen:** Angreifer erhält Profildateien / Config-Dateien **ohne** den Schlüssel.
- **Schützt NICHT gegen:** Angreifer, der zugleich den Schlüssel hat (z. B. kompletter
  Vollzugriff auf den laufenden Rechner). Der Schlüssel liegt lokal gespeichert (Kais Wunsch:
  Alltag nahtlos). Der Schutz steht und fällt damit, dass der Schlüssel NICHT im selben
  Leak/Backup wie die Profile landet → siehe „Schlüsselort".

## Goals
- Symmetrischer Schlüssel, einmal erzeugt, als Textwert sicher-/exportierbar und importierbar.
- Gespeicherte Passwörter (`portable_profile`-Feld UND `secrets/`-Dateien) at-rest verschlüsselt.
- „Schlüssel anlegen" verschlüsselt **bestehende** Klartext-Passwörter nachträglich.
- Transparente Entschlüsselung beim Verbinden, kein Mehraufwand im Alltag.
- Alles verwaltet in **Settings → Verschlüsselung** (anlegen / Wert zeigen / importieren / entfernen / Status).

## Non-Goals
- Keine Verschlüsselung der SSH-Key-Passphrasen (werden weiterhin nie gespeichert).
- Keine Verschlüsselung der privaten Key-**Dateien** (liegen per Pfad außerhalb, nicht im Profil).
- Kein neuer Sync-/Transportmechanismus für die Profildateien selbst (Profile bleiben einzelne
  JSON-Dateien; Transport wie bisher durch den Nutzer, z. B. Datei kopieren).
- Kein Passwort-/Passphrase-abgeleiteter Schlüssel (KDF) — der Schlüssel wird gespeichert, nicht
  gemerkt; volle 256-Bit-Entropie statt merkbarer Passphrase.

## Anti-Goals (Fehlschlag trotz technischer Funktion)
- Schlüssel landet im selben Leak wie die Profile (z. B. roh in `settings.json` im selben Ordner)
  → Schutz wäre nur Scheinsicherheit.
- Ein importiertes, aber (noch) nicht entschlüsselbares Profil führt zu Crash/Datenverlust statt
  klarer Meldung.

## Constraints
- Rust/Tauri-Backend, Vanilla-TS-Frontend; bestehende Credential-Architektur
  (`secrets_service`, `connection.rs`, `profile_service`, `credentials_store`).
- Rückwärtskompatibel: Profile ohne Verschlüsselung funktionieren unverändert weiter.
- Private Dateien 0600, atomar geschrieben (bestehendes `fs_secure`).
- Neue Krypto-Abhängigkeit nötig (heute keine im Projekt).

## Approaches (betrachtet)
1. **Verschlüsseltes Export-Bündel** (Profile bleiben lokal Klartext). — Verworfen: ein lokal
   geleaktes Profil bliebe nutzbar, widerspricht dem Ziel.
2. **`age`-Identität** statt symmetrischem Schlüssel. — Verworfen: schwerere Abhängigkeit, kein
   Mehrwert für diesen Fall.
3. **Per-Feld-AEAD direkt im Profil (gewählt).** Jeder gespeicherte Geheimwert wird zu
   `enc:v1:<base64(nonce||ciphertext)>`. Profile bleiben einzelne JSON-Dateien. Kleinster Eingriff,
   fügt sich in die bestehenden Speicherstufen ein.

## Entschiedene How-Details
- **Krypto:** AES-256-GCM (Crate `aes-gcm`, RustCrypto-AEAD), 96-Bit-Zufalls-Nonce pro Wert aus
  `OsRng`. Serialisierung als Präfix-String `enc:v1:` + base64(nonce||ct||tag). Präfix erlaubt
  Klartext/Ciphertext zu unterscheiden und spätere Formatversionen.
- **Schlüssel:** 32 Byte aus `OsRng`. Export/Import als base64-Textwert. Verlust = gespeicherte
  Passwörter nicht mehr entschlüsselbar (neu eingebbar, da es SSH-Passwörter des Nutzers sind) —
  deutliche Warnung beim Anlegen.
- **Schlüsselort (siehe offene Frage):** eigene Datei `master.key` (0600) an definierter App-Stelle,
  Pfad in `settings.json` hinterlegt und in der Settings-UI sichtbar/verwaltet. NICHT roh in
  `settings.json` selbst.
- **Verschlüsselt werden:** `profile.stored_secret_portable` und die `secrets/<id>`-Dateien.
  `connect_sftp`/`save_credential`/`clear_credential` ent-/verschlüsseln transparent über ein neues
  `crypto_service`.
- **Nachträgliche Verschlüsselung:** „Schlüssel anlegen" läuft über alle Profile + `secrets/`-Dateien,
  verschlüsselt jeden noch unverschlüsselten Geheimwert in place (mit Backup wie bei `save_profile`).
- **Kein Schlüssel vorhanden:** Verhalten wie heute (Klartext). Ciphertext ohne Schlüssel (frisch
  importiertes Profil, Schlüssel fehlt) → Verbinden scheitert mit klarer Meldung „Schlüssel
  importieren", kein Crash, keine Überschreibung.
- **Schlüsselwechsel/entfernen:** „Entfernen" entschlüsselt alle Werte zurück zu Klartext (bewusste
  Downgrade-Bestätigung). (Rotation = entfernen + neu anlegen; eigener Rotations-Button optional.)

## Was dem Builder überlassen bleibt
- Genaue Modul-/Command-Namen (`crypto_service`, `encryption`-Commands), i18n-Keys in allen 6 Locales.
- Settings-UI-Feinschliff (Warntexte, Copy-to-clipboard des Schlüsselwerts).
- Tests: Round-trip ver-/entschlüsseln, Legacy-Klartext-Lesbarkeit, nachträgliche Verschlüsselung,
  „Ciphertext ohne Schlüssel"-Pfad, Nonce-Einzigartigkeit.

## Entschieden (Kai, 2026-11-16)
Der Schlüssel liegt in einer **eigenen 0600-Datei** an definierter App-Stelle und wird vollständig
über die Settings-UI verwaltet (anlegen/zeigen/importieren/entfernen/Status). Der **Rohwert kommt
nicht in `settings.json`** — sonst läge er im selben Ordner/Backup wie die Profile und der Schutz
wäre dahin. In `settings.json` steht höchstens der Pfad/Status, nicht der Schlüssel.

## Folge-Deliverables (nach dem Bau, Kais Auftrag 2026-11-16)
1. **Fable-Sicherheitsreview** über den fertigen Krypto-/Credential-Pfad.
2. **Programmbeschreibung** im CrispyInfoSpace (`projekte/murmurssh/…-funktionen.md`) — Funktions-
   übersicht des Programms, inkl. Verschlüsselung; Stand/Quelle/Version mitziehen.
3. **README** + **CHANGELOG** um die Verschlüsselung erweitern.
