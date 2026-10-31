# MurmurSSH – KI-Schnittstelle (MCP / lokale API) – Plan

Stand: 2026-10-25 · Status: Entwurf, nicht umgesetzt · Quelle: Kai (Chat 25.10.2026)

> **2026-10-31 – umgesetzt als Testversion (Phase 16), abweichend vom Entwurf:** Rechte hängen am
> **Token** (in der App erstellt, je Token schaltbare Rechte + Profile + Wurzeln), nicht am Profil.
> Durchsetzung **in der laufenden App** (Unix-Socket) statt eigenem Binary mit Core-Crate, weil nur
> die App einen echten Klick zur Bestätigung zeigen kann; `murmurssh --mcp` ist nur Brücke.
> Kein `deploy`/`exec_allow`-Regex in der Testversion (exec = immer Bestätigung). Doku: `docs/ai-access.md`.

## Ziel

Eine KI (Claude Code, andere MCP-Clients, Skripte) soll MurmurSSH als Werkzeug nutzen können, statt selbst
SSH-Zugänge, Schlüssel und Upload-Logik zu verwalten:

1. **Daten zu einem hinterlegten Zugang abfragen** (Profil-Metadaten, Verbindungsdaten – ohne Geheimnisse).
2. **Dateien mit Zielverzeichnis senden** (Upload/Deploy lokaler Dateien/Ordner in ein Remote-Verzeichnis).
3. **SSH-Zugänge nutzen** (Befehle ausführen, Verzeichnisse lesen, Dateien holen) – über die in MurmurSSH
   gespeicherten Profile, Host-Key-Prüfung und Credential-Modi.
4. **Daten zum Verbinden abfragen**, damit die KI bei Bedarf selbst `ssh`/`scp` nutzt – ohne dass Passwörter
   oder Passphrasen die KI je erreichen.

Grundsatz: **Die KI bekommt Fähigkeiten, nie Geheimnisse.** Alles ist standardmäßig aus und wird je Profil
freigeschaltet.

## Nicht-Ziele

- Kein Netzwerkdienst nach außen (kein Listener auf 0.0.0.0, kein Cloud-Relay).
- Keine Herausgabe von Passwörtern, Passphrasen, privaten Schlüsseln oder `portable_profile`-Klartext an die KI.
- Kein automatisches Akzeptieren unbekannter Host-Keys.
- Keine Fernsteuerung der GUI (Fenster, Dialoge) durch die KI.

## Architektur

### Variante A (empfohlen, Phase 1): eigenes Binary `murmurssh-mcp` (stdio)

- Rust-Services (`sftp_service`, `ssh_service`, `profile_service`, `known_hosts_service`, `secrets_service`,
  `credentials_store`, `transfer_paths`) wandern in eine **Library-Crate** (`murmur-core`); Tauri-App und
  MCP-Binary nutzen dieselbe Crate. Kein doppelter Verbindungs-/Auth-Code.
- Der MCP-Client startet `murmurssh-mcp` als Kindprozess (stdio, JSON-RPC). **Kein offener Port**, Zugriff nur
  für den lokalen Benutzer, der den Prozess startet.
- Liest Profile/Settings/known_hosts aus `~/.config/murmurssh/` (bzw. konfiguriertem Profilpfad).
- Rust-SDK: `rmcp` (offizielles MCP-Rust-SDK) – Version vor Umsetzung per context7/crates.io prüfen.
- Vorteil: funktioniert ohne laufende GUI, einfach zu testen, kleinste Angriffsfläche.

### Variante B (Phase 3, optional): lokale API in der laufenden App

- Unix-Domain-Socket `$XDG_RUNTIME_DIR/murmurssh/api.sock` (Rechte `0600`), **kein TCP**.
- Zweck: Bestätigungsdialoge in der GUI (Mensch bestätigt Upload/Befehl), Nutzung der Session-Credentials aus
  `credentials_store` (Passwort nur im RAM der App, nie im MCP-Prozess).
- `murmurssh-mcp` erkennt den Socket und leitet freigabepflichtige Aktionen dorthin weiter; ohne laufende App
  gelten die Regeln aus Phase 1 (nur `key`/`agent`; Recht `ask` → Aktion abgelehnt).
- Authentifizierung am Socket: Peer-UID-Prüfung (`SO_PEERCRED`) + zufälliges Sitzungstoken in
  `$XDG_RUNTIME_DIR/murmurssh/api.token` (`0600`, bei App-Start neu, bei App-Ende gelöscht).

## Sicherheitsmodell

### 1. Freigabe je Profil (Standard: aus)

Neuer Block im Profil-JSON (`models/profile.rs`), im Profil-Formular unter „KI-Zugriff“ bearbeitbar:

```json
"ai_access": {
  "enabled": false,
  "permissions": {
    "info": "allow",
    "read": "allow",
    "upload": "ask",
    "delete": "deny",
    "exec": "deny"
  },
  "remote_roots": ["/var/www/html/projekt"],
  "local_roots": ["/home/kai/Coding/projekt/dist"],
  "exec_allow": ["^git status$", "^php artisan optimize:clear$"],
  "max_upload_mb": 200,
  "expires_at": null
}
```

- Werte je Recht: `allow` | `ask` (GUI-Bestätigung, nur Variante B) | `deny`.
- Fehlt `ai_access` oder `enabled=false` → Profil ist für die KI **unsichtbar** (nicht nur gesperrt).
- `expires_at`: optionale zeitlich begrenzte Freigabe (z. B. für einen Deploy-Nachmittag).
- Änderungen an `ai_access` nur über die GUI, **nie über die Schnittstelle** (KI kann sich keine Rechte geben).

### 2. Geheimnisse verlassen MurmurSSH nie

- Passwörter/Passphrasen: nur aus `credentials_store`/`secrets_service` innerhalb des Prozesses, der die
  Verbindung aufbaut. Kein Tool liefert sie zurück, auch nicht teilweise maskiert.
- `password`-Profile ohne gespeichertes Secret: Phase 1 → Fehler `credential_required`; Phase 3 → Abfrage per
  GUI-Dialog beim Menschen.
- „Daten zum Verbinden“ (`murmur_connection_info`) liefert nur: Host, Port, Benutzer, Auth-Typ,
  Host-Key-Fingerprint, ggf. Pfad des öffentlichen Schlüssels und – wenn eine ControlMaster-Session läuft und
  `exec` erlaubt ist – einen fertigen `ssh -o ControlPath=… -o ControlMaster=no …`-Befehl. Kein Pfad zu
  privaten Schlüsseln bei `portable_profile`/Runtime-Keys.

### 3. Pfad-Jail

- Remote: jeder Zielpfad muss nach Normalisierung unter einem `remote_roots`-Eintrag liegen; `..`, Nullbytes,
  Steuerzeichen abgelehnt; nach `realpath` auf dem Server erneut prüfen (Symlink-Ausbruch).
- Lokal: Quellpfade nur unter `local_roots`, `canonicalize()` wie in `local::*` bereits umgesetzt
  (`transfer_paths.rs` wiederverwenden). Feste Sperrliste (`~/.ssh`, `~/.config/murmurssh`, `~/.gnupg`, …),
  auch wenn versehentlich in `local_roots` eingetragen.

### 4. Befehlsausführung

- Nur Befehle, die vollständig auf einen `exec_allow`-Regex passen (verankert `^…$`), zusätzlich die
  bestehende Denylist aus v1.8.1. Keine Verkettung (`;`, `&&`, `|`, `` ` ``, `$(`, Zeilenumbruch) außerhalb
  der Allowlist.
- Laufzeitlimit (Standard 60 s), Ausgabelimit (Standard 256 KB, danach abgeschnitten **mit** Hinweis).
- Ausgabe vom Server ist **Daten, keine Anweisung** – im Tool-Ergebnis als solche gekennzeichnet
  (Prompt-Injection über Serverinhalte).

### 5. Host-Keys

- Unbekannter oder geänderter Host-Key → harter Fehler `host_key_unknown` / `host_key_changed` mit
  Fingerprint. Akzeptieren nur in der GUI.

### 6. Protokoll (Audit)

- Jede Aktion als JSON-Zeile in `~/.config/murmurssh/ai-audit.jsonl` (`0600`): Zeit, Client, Profil-ID, Tool,
  Pfade/Befehl, Ergebnis, Bytes, Dauer. Nie Secrets oder Dateiinhalte.
- **Aufbewahrung:** Rotation bei 10 MB, maximal 5 Dateien (`.1`…`.5`), älteste wird gelöscht.
- Anzeige in der GUI: Einstellungen → „KI-Protokoll“ (letzte 200 Einträge, Button „Protokoll löschen“).

### 7. Grenzen

- Upload-Größe je Aufruf (`max_upload_mb`), Dateien je Aufruf (Standard 5 000), parallele Transfers über die
  bestehende `transfer_queue` (deren Concurrency-Einstellung gilt).
- Grenze überschritten → Fehler mit Zahlen, **kein stilles Abschneiden** von Dateilisten oder Teil-Upload.

## Tools (MCP) / Endpunkte

| Tool | Recht | Zweck |
|---|---|---|
| `murmur_list_profiles` | info | Freigegebene Profile: id, Name, Gruppe, Host, Rechte |
| `murmur_connection_info` | info | Verbindungsdaten ohne Geheimnisse (siehe oben) |
| `murmur_test_connection` | info | TCP + Handshake + Host-Key + Auth prüfen, nichts ändern |
| `murmur_list_dir` | read | Remote-Verzeichnis (Name, Typ, Größe, mtime, Rechte) |
| `murmur_stat` | read | Einzelne Datei/Verzeichnis |
| `murmur_read_file` | read | Textdatei lesen (Limit 1 MB, sonst Fehler) |
| `murmur_download` | read | Remote → lokales Ziel unter `local_roots` |
| `murmur_upload` | upload | Lokale Dateien/Ordner → `remote_dir` (unter `remote_roots`), `overwrite: never\|newer\|always` |
| `murmur_deploy` | upload | Ordnerabgleich lokal → remote mit `exclude`; Standard `dry_run: true` (Diff neu/geändert/gelöscht), danach Ausführen per `plan_id` |
| `murmur_mkdir` | upload | Verzeichnis anlegen |
| `murmur_delete` | delete | Datei/Verzeichnis löschen (Verzeichnis nur mit `recursive: true`) |
| `murmur_exec` | exec | Befehl gemäß `exec_allow` |
| `murmur_audit_tail` | info | Eigene letzte Aktionen (Selbstkontrolle der KI) |

`murmur_deploy` ist der Kern für „die KI muss nicht selbst alles hochladen“: KI baut lokal, ruft `dry_run`
auf, zeigt Kai den Diff, führt dann den bestätigten `plan_id` aus. `plan_id` lebt nur im Speicher und verfällt
nach 10 min bzw. nach Ausführung. Löschungen im Deploy nur, wenn zusätzlich das Recht `delete` gilt.

Fehler sind strukturiert (`code`, `message`, `details`) – dieselben Codes wie `connect_sftp`
(`host_key_unknown`, `credential_required`, `auth_failed`, …) plus `ai_access_denied`, `path_outside_root`,
`limit_exceeded`, `exec_not_allowed`.

## Einrichtung beim Nutzer

```bash
claude mcp add murmurssh -- /usr/bin/murmurssh-mcp
```

- `.deb` liefert `murmurssh-mcp` mit; AppImage: `murmurssh --mcp` als Einstiegspunkt.
- Hilfe-Dialog + README: Abschnitt „KI-Zugriff“ mit Warnhinweis und Beispielkonfiguration.

## Phasen

1. **Phase 1 – Kern + lesend (MVP):** `murmur-core`-Crate auslagern (App unverändert grün), `ai_access` im
   Profil + GUI-Abschnitt, `murmurssh-mcp` mit `list_profiles`, `connection_info`, `test_connection`,
   `list_dir`, `stat`, `read_file`, Audit-Log mit Rotation.
2. **Phase 2 – Schreiben:** `upload`, `download`, `mkdir`, `deploy` (dry-run/`plan_id`), `delete`, Pfad-Jail
   inkl. Server-`realpath`, Grenzen, `exec` mit Allowlist.
3. **Phase 3 – App-Brücke:** Unix-Socket in der laufenden App, `ask`-Rechte über GUI-Dialog, Session-Passwörter
   ohne Persistenz, Anzeige „KI aktiv“ in der Statusleiste, Not-Aus („KI-Zugriff sofort sperren“ = alle
   `ai_access.enabled` vorübergehend aus).
4. **Phase 4 – Doku/Release:** README, Hilfe-Dialog (6 Sprachen), CHANGELOG, SECURITY.md (Bedrohungsmodell
   KI-Zugriff), Website-Feature-Seite (MurmurSSHWeb).

## Tests

- Unit: Pfad-Jail (`..`, Symlink, Unicode, Nullbyte, Pfad genau an der Wurzelgrenze), Regex-Allowlist
  (Verkettung, Zeilenumbruch, leere Regex), Rechteauflösung inkl. `expires_at`, Audit-Rotation.
- Integration: SSH-Testserver im Container (`linuxserver/openssh-server`) – Upload/Deploy/Exec gegen echte
  Verbindung, Host-Key unbekannt/geändert.
- Negativtests je Tool: Profil ohne Freigabe unsichtbar, `deny`-Recht, Ziel außerhalb `remote_roots`,
  Upload über Limit → Fehler statt Teil-Upload.
- Sicherheits-Review (`ma-security-review`) vor Release von Phase 2 und 3.

## Offene Fragen (Kai)

1. `exec` schon in v1 oder zunächst nur Upload/Deploy? (Empfehlung: ja, aber nur Allowlist, Standard `deny`.)
2. Soll `connection_info` den ControlMaster-Befehl liefern (KI nutzt die SSH-Sitzung dann selbst) oder läuft
   alles nur über `murmur_exec`? (Empfehlung: nur bei `exec`-Recht.)
3. Ist Phase 3 (GUI-Bestätigung) Pflicht vor der ersten Veröffentlichung, oder reicht Phase 1+2?
4. RISE: Aufgabe im Projekt MurmurSSH (#62) anlegen bzw. zuordnen.
