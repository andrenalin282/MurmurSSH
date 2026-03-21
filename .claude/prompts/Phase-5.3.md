# Phase 5.3 — SSH Trust Flow, File Browser Navigation & Disconnect

## Ziel

Phase 5.3 verbessert den SSH-Verbindungsfluss und den SFTP-Dateibrowser deutlich in UX, Sicherheit und Stabilität.

Die aktuelle Situation:
- SSH-Login funktioniert grundsätzlich
- unbekannte Host-Keys/Fingerprints werden noch nicht sauber im UI behandelt
- Passwort soll nur bei erfolgreicher Authentifizierung gespeichert werden
- der Dateibrowser kann sich in nicht lesbaren Verzeichnissen festfahren
- es fehlen Navigationshilfen (Home / Up / Breadcrumbs)
- es fehlt ein expliziter Disconnect-Button

Diese Phase führt deshalb einen vollständigen, robusten Connect-/Browse-/Disconnect-Flow ein.

---

## Ziele im Detail

### 1. Host-Key / Fingerprint Trust Flow
Wenn ein Server-Host-Key unbekannt ist, darf die App nicht still scheitern und der Benutzer soll nicht ins externe Terminal ausweichen müssen.

Stattdessen:
- Host-Key prüfen
- wenn unbekannt:
  - Dialog im UI anzeigen
  - Host, Port, Key-Typ und Fingerprint darstellen
  - Optionen anbieten:
    - Accept once
    - Accept and save
    - Cancel
- nach Zustimmung automatisch mit der Authentifizierung fortfahren
- innerhalb desselben Connect-Versuchs das bereits eingegebene Passwort weiterverwenden

### 2. Passwortspeicherung nur nach erfolgreicher Anmeldung
Gespeicherte Credentials dürfen nur geschrieben werden, wenn die SSH-Authentifizierung erfolgreich abgeschlossen ist.

Wichtig:
- Passwort nie vor erfolgreichem Login persistieren
- bei Fehler, Cancel oder Disconnect während des Verbindungsaufbaus nichts speichern
- innerhalb einer laufenden Connect-Session darf das Passwort temporär im RAM gehalten werden

### 3. File Browser Navigation verbessern
Der Dateibrowser soll oberhalb der Dateiliste eine kleine Navigationsleiste bekommen:

- Disconnect
- Home
- Up
- Refresh
- Breadcrumbs des aktuellen Pfads

Zusätzlich:
- Home springt zum im Profil definierten Startpfad
- falls kein Startpfad gesetzt ist, verwende sinnvollen Fallback
- Up geht genau ein Verzeichnis nach oben
- Breadcrumb-Segmente sind klickbar

### 4. Kein Festfahren bei nicht lesbaren Ordnern
Wenn ein Benutzer einen Ordner öffnet, für den keine Leserechte bestehen:
- zeige Fehlermeldung
- bleibe im letzten gültigen Verzeichnis
- überschreibe currentPath nicht mit einem unlesbaren Pfad
- die Navigation bleibt weiterhin funktionsfähig

### 5. Disconnect
Es soll einen klar sichtbaren Disconnect-Button geben.

Disconnect muss:
- SSH/SFTP-Session sauber schließen
- Browserzustand zurücksetzen
- Statusanzeige auf getrennt setzen
- File-Liste leeren oder neutralen Zustand anzeigen
- Buttons und Aktionen korrekt aktualisieren

---

## Nicht-Ziele

Diese Phase macht noch nicht:
- Multi-tab Sessions
- parallele Verbindungen zu mehreren Hosts
- Drag & Drop Upload/Download
- Terminal-Emulation
- vollständigen known_hosts-Import aus System-SSH, falls dies architektonisch aufwändig ist
- vollständige GTK-native Host-Key-Integration außerhalb der App

---

## Funktionale Anforderungen

## A. SSH Host Trust / Known Hosts

### A1. Host-Key-Prüfung
Beim Connect:
1. TCP-Verbindung aufbauen
2. SSH-Handshake starten
3. Host-Key/Fingerprint auslesen
4. Gegen bekannte Hosts prüfen

### A2. Unbekannter Host
Wenn der Host unbekannt ist:
- stoppe vor der Authentifizierung
- gib dem Frontend strukturierte Informationen zurück:
  - host
  - port
  - algorithm / key type
  - fingerprint
  - optional raw host key / known-hosts entry, falls nötig

Frontend zeigt Dialog:
- Titel: Unknown Host Key
- Text: Der Server ist noch nicht als vertrauenswürdig gespeichert
- Buttons:
  - Accept once
  - Accept and save
  - Cancel

### A3. Nach Bestätigung fortfahren
Wenn der Benutzer bestätigt:
- Connect-Vorgang fortsetzen, ohne neues Passwort anzufordern
- das zuvor eingegebene Passwort derselben Connect-Session wiederverwenden
- bei "Accept and save" Host dauerhaft speichern
- bei "Accept once" nur für diese Session zulassen

### A4. Host-Key-Mismatch
Falls später ein bekannter Host einen anderen Fingerprint liefert:
- nicht automatisch verbinden
- klaren Sicherheitsfehler anzeigen
- keine Authentifizierung fortsetzen
- nichts automatisch überschreiben

---

## B. Passwortspeicherung

### B1. Erfolgsbedingung
Passwort nur speichern, wenn:
- Auth-Call erfolgreich war
- Session tatsächlich authenticated ist
- Verbindung nicht abgebrochen wurde

### B2. Fehlerfall
Bei:
- falschem Passwort
- unbekanntem Host und Cancel
- Host-Key-Mismatch
- Netzwerkfehler
- SFTP-Init-Fehler vor erfolgreichem Abschluss des Connect-Flows

dürfen keine Credentials persistiert werden.

### B3. Reihenfolge
Speichern erst nach:
1. Host-Key-Vertrauen bestätigt
2. Auth erfolgreich
3. Session bestätigt
4. optional SFTP initialisiert, falls eure Architektur den Connect erst dann als vollständig betrachtet

---

## C. File Browser Navigation

### C1. Toolbar oberhalb der Dateiliste
Füge eine Toolbar hinzu mit:
- Disconnect
- Home
- Up
- Refresh
- Breadcrumbs

### C2. Home
Home springt auf den Startpfad des Profils.

Empfohlene Logik:
- wenn Profil `start_path` besitzt → diesen verwenden
- sonst fallback auf Benutzer-Home, falls zuverlässig ermittelbar
- sonst `/`

### C3. Up
- wechselt in das Parent-Verzeichnis des aktuellen Pfads
- bei `/` bleibt Up auf `/`
- kein Fehler bei Root

### C4. Refresh
- lädt den aktuellen Pfad neu
- ändert den Pfad nicht

### C5. Breadcrumbs
Beispiel:
`/` oder `Home / usr / home / virtuh / uploads`

Anforderungen:
- jeder Teil klickbar
- Klick navigiert genau auf den entsprechenden Zwischenpfad
- aktueller Abschnitt optisch hervorgehoben

---

## D. Sichere Navigation bei Permission Denied

### D1. Navigation nur bei erfolgreichem Listing committen
Beim Klick auf einen Ordner:
1. targetPath berechnen
2. targetPath lesen/listen versuchen
3. nur bei Erfolg:
   - currentPath = targetPath
   - UI aktualisieren
4. bei Fehler:
   - Fehlermeldung anzeigen
   - auf bisherigem gültigen currentPath bleiben

### D2. Fehlermeldung
Fehlertext soll sichtbar, aber nicht blockierend sein:
- Statusleiste
- Inline-Fehler
- Toast, falls vorhanden

Text z. B.:
- Failed to open '/path': permission denied

---

## E. Disconnect

### E1. Verhalten
Disconnect muss:
- aktive SFTP-Session schließen
- aktive SSH-Session schließen
- temporäre Connect-Daten verwerfen
- Dateiliste leeren
- currentPath zurücksetzen
- Verbindungsstatus auf disconnected setzen

### E2. UI-Zustand nach Disconnect
- Disconnect-Button deaktivieren oder ausblenden, wenn keine Verbindung aktiv
- Connect wieder verfügbar
- Refresh/Home/Up/Breadcrumbs deaktivieren oder neutral
- Dateibereich zeigt leeren Zustand

---

## Technische Leitplanken

### Backend
Prüfen / implementieren:
- Host-Key aus SSH-Session auslesen
- Known-hosts Prüfroutine
- Ergebnisstatus an Frontend zurückgeben
- Connect als mehrstufigen Flow modellieren, falls derzeit monolithisch
- Passwortspeicherung strikt hinter erfolgreiche Auth legen
- Disconnect-Befehl sauber implementieren
- SFTP list_dir darf Pfadwechsel nicht vorzeitig committen

### Frontend
Prüfen / implementieren:
- Modal/Dialog für Host-Key-Akzeptanz
- Toolbar im File Browser
- Breadcrumb-Komponente
- sauberer Connected/Disconnected State
- Fehleranzeige ohne UI-Sackgasse

---

## UX-Anforderungen

- kein externer Terminal-Schritt mehr nötig, um einen unbekannten Host zu akzeptieren
- keine doppelte Passwortabfrage innerhalb desselben Connect-Versuchs
- Dateibrowser darf sich nicht in ungültigen Zuständen festfahren
- Navigation soll mit wenigen Buttons klar und intuitiv bleiben
- Disconnect muss sofort sichtbar und eindeutig sein

---

## Akzeptanzkriterien

1. Verbindung zu einem unbekannten Host zeigt einen Fingerprint-Dialog statt still zu scheitern.
2. Nach "Accept once" oder "Accept and save" läuft derselbe Connect-Versuch automatisch weiter.
3. Das Passwort wird nicht erneut abgefragt, wenn es bereits in dieser Connect-Session eingegeben wurde.
4. Passwort wird nur gespeichert, wenn die Anmeldung erfolgreich war.
5. Bei fehlgeschlagener Anmeldung wird kein Passwort gespeichert.
6. Im Dateibrowser gibt es Disconnect, Home, Up, Refresh und Breadcrumbs.
7. Klick auf einen nicht lesbaren Ordner zeigt Fehler, lässt den Benutzer aber im letzten gültigen Pfad.
8. Disconnect trennt die Sitzung sauber und setzt den UI-Zustand korrekt zurück.
9. Nach Disconnect kann sofort wieder sauber verbunden werden.
10. Root `/` und Parent-Navigation verhalten sich stabil ohne Sonderfehler.

---

## Empfohlene Umsetzungsschritte / Todo-Reihenfolge

1. Connect-Flow analysieren
   - aktueller Ablauf TCP → handshake → auth → SFTP
   - Stelle identifizieren, an der Host-Key-Prüfung sauber eingebaut werden kann

2. Passwort-Speicherlogik absichern
   - sicherstellen, dass Persistenz erst nach erfolgreicher Auth geschieht

3. Host-Key-Dialog-Endpunkt / Statusmodell einführen
   - Backend liefert "unknown host" strukturiert zurück
   - Frontend kann darauf reagieren

4. Confirm-and-continue Flow implementieren
   - nach Benutzerentscheidung denselben Connect-Versuch fortsetzen

5. Disconnect sauber implementieren
   - Session schließen
   - UI-State resetten

6. Toolbar im File Browser bauen
   - Disconnect
   - Home
   - Up
   - Refresh

7. Breadcrumbs implementieren

8. Sichere Ordnernavigation implementieren
   - path switch nur nach erfolgreichem list_dir

9. End-to-End Tests / manuelle Tests durchführen

---

## Manuelle Testfälle

### Host-Key
- Neuer Host → Fingerprint-Dialog erscheint
- Accept once → Verbindung klappt
- Accept and save → Verbindung klappt, nächster Connect fragt nicht erneut
- Cancel → Verbindung abgebrochen, nichts gespeichert

### Passwortspeicherung
- Save password aktiv + falsches Passwort → nichts gespeichert
- Save password aktiv + erfolgreicher Login → gespeichert
- Unknown host + Cancel → nichts gespeichert

### Navigation
- Home funktioniert
- Up funktioniert
- Refresh funktioniert
- Breadcrumb-Klick funktioniert
- Klick auf nicht lesbaren Ordner → Fehler, aber alter Pfad bleibt nutzbar

### Disconnect
- aktive Verbindung trennen
- Dateiliste verschwindet oder neutralisiert sich
- erneuter Connect direkt möglich

---

## Hinweise zur Architektur

Wenn der aktuelle Connect-Befehl noch vollständig monolithisch ist, darf er für diese Phase in einen klareren State-Flow aufgeteilt werden, z. B.:

- idle
- connecting
- awaiting_host_trust
- authenticating
- connected
- disconnecting
- disconnected
- error

Das ist erlaubt, solange die Änderung gezielt und kontrolliert bleibt.

---

## Definition of Done

Die Phase ist abgeschlossen, wenn:
- der Benutzer unbekannte Hosts vollständig im UI bestätigen kann
- das Passwort erst nach erfolgreicher Auth gespeichert wird
- der Dateibrowser robuste Navigation besitzt
- Permission-Denied nicht mehr zum Festfahren führt
- Disconnect vollständig funktioniert
- die Umsetzung dokumentiert und testbar ist
