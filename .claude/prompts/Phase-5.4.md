# Phase 5.4 — Terminal Agent Support, Scroll Fix & Portable Profile Storage

## Ziel

Phase 5.4 verbessert:

1. Terminal-Login ohne erneute Passwortabfrage (SSH-Agent-basierte Weitergabe)
2. File-Browser Scrollbarkeit
3. Frei wählbarer Profil-Speicherort (Cloud-/NAS-kompatibel)
4. Konfliktsichere Speicherung mit Backup-Dateien
5. Schnellzugriff auf Profilordner
6. Settings-Zugriff über kleinen Button unten links

Die Phase bleibt strikt innerhalb bestehender Architektur und erweitert nur UX und Storage.

---

## Ziele im Detail

### 1. Terminal ohne erneute Passwortabfrage

Der Terminal-Button startet eine neue SSH-Session.

Aktuell:
- Passwort wird erneut abgefragt

Neu:
- Terminal nutzt vorhandene Auth über SSH-Agent

Anforderungen:

- Passwortlogin bleibt möglich
- keine unsichere Passwortweitergabe an externe Prozesse
- Agent nur für aktuelle Session gültig
- Disconnect entfernt Agent-Credentials

---

### 2. Terminal-Button Zustand

Toolbar-Verhalten:

Disconnected:
- Connect sichtbar
- Terminal ausgeblendet oder deaktiviert

Connected:
- Disconnect sichtbar
- Terminal sichtbar und aktiv

Terminal verbindet zum selben Host/Port/User.

---

### 3. File Browser Scroll Fix

Dateiliste muss vertikal scrollbar sein.

Anforderungen:

- Container passt sich an verfügbare Höhe an
- Scroll nur innerhalb der Liste, nicht im gesamten Fenster
- Funktioniert mit Toolbar + Breadcrumbs

---

### 4. Frei wählbarer Profil-Speicherort

Benutzer kann Profile außerhalb des Standardpfads speichern.

Use Cases:
- Nextcloud
- OneDrive
- NAS
- USB-Stick
- mehrere PCs

Standardpfad bleibt unverändert.

---

### 5. Backup-Dateien bei Konflikten

Beim Speichern von Profilen:

- Vor Überschreiben wird Backup erstellt
- Namensschema z. B.:

profiles.json.bkp
profiles.json.bkp-YYYYMMDD-HHMMSS

Keine komplexe Konfliktauflösung — nur sichere Sicherung.

---

### 6. Profilordner öffnen

Kleiner Button unten links:

- Icon: Folder oder ähnlich
- Aktion: öffnet Profilverzeichnis im System-Dateimanager

---

### 7. Settings-Button

Ebenfalls unten links:

- Kleiner Button mit Wrench-Icon
- Öffnet Einstellungsdialog

Settings enthalten mindestens:

- Profil-Speicherort:
  - Default
  - Custom Pfad
  - Browse-Button

---

## Funktionale Anforderungen

### A. SSH-Agent Support

- Beim erfolgreichen Login kann Agent-Credential bereitgestellt werden
- Terminal nutzt diesen Agent
- Keine Speicherung des Passworts für Terminalstart nötig
- Agent-Daten werden bei Disconnect gelöscht

---

### B. File Browser Scroll

- Liste erhält overflow-y: auto
- Elterncontainer korrekt dimensioniert (Flex/Grid)

---

### C. Custom Profile Storage

#### C1. Speicherpfad

App verwendet einen konfigurierbaren Root-Pfad:

Default:
~/.config/murmurssh/

Custom:
vom Benutzer gewählt

---

#### C2. Wechsel des Pfads

Nach Änderung:

- Profile neu laden
- kein App-Neustart erforderlich (wenn einfach möglich)
- sonst Hinweis anzeigen

---

#### C3. Validierung

Custom-Pfad muss:

- existieren oder erstellt werden können
- beschreibbar sein

---

### D. Backup-Mechanismus

Beim Schreiben von Profil-Dateien:

1. Falls Datei existiert:
   - Kopie als .bkp erstellen
2. Dann neue Datei schreiben

---

### E. Profilordner öffnen

Systemabhängig:

Linux:
xdg-open <pfad>

---

### F. Settings-Dialog

Minimalanforderungen:

- Profil-Speicherort anzeigen
- Default / Custom Auswahl
- Browse-Dialog für Verzeichnis
- Apply / Cancel

---

## UX-Anforderungen

- Buttons unten links klein und unaufdringlich
- Keine Überfrachtung der Hauptoberfläche
- Terminalstart nur bei aktiver Verbindung
- Profil-Storage intuitiv für nicht-technische Nutzer

---

## Akzeptanzkriterien

1. Terminal kann geöffnet werden, ohne erneut Passwort einzugeben.
2. Terminal nutzt dieselbe Verbindungskonfiguration wie der File Browser.
3. Dateiliste ist korrekt scrollbar.
4. Benutzer kann einen Custom-Profilpfad wählen.
5. Profile werden an diesem Ort gespeichert und geladen.
6. Beim Speichern werden Backup-Dateien erstellt.
7. Profilordner kann direkt geöffnet werden.
8. Settings sind über Wrench-Button erreichbar.
9. Disconnect entfernt Agent-Daten und verhindert Terminalstart.
10. Feature funktioniert ohne Neustart der App (oder mit klarer Aufforderung).

---

## Empfohlene Umsetzungsschritte

1. File Browser Scroll fix
2. Terminal Button Zustand implementieren
3. SSH-Agent Integration
4. Custom Profile Root einführen
5. Backup beim Schreiben implementieren
6. Open Profile Folder Button
7. Settings-Dialog erstellen
8. Integration und Tests

---

## Manuelle Testfälle

### Terminal
- Verbinden → Terminal öffnen → kein Passwort erforderlich
- Disconnect → Terminal-Button deaktiviert

### Scroll
- Viele Dateien → Liste scrollt sauber

### Profil-Speicherort
- Custom Pfad wählen → Profile dort gespeichert
- App neu starten → Profile werden geladen

### Backup
- Profil ändern → Backup-Datei vorhanden

### Buttons
- Folder-Button öffnet Profilordner
- Wrench öffnet Settings

---

## Definition of Done

Phase 5.4 ist abgeschlossen, wenn:

- Terminal ohne erneute Passwortabfrage funktioniert
- Profil-Speicherort frei wählbar ist
- Backup-Dateien erzeugt werden
- File Browser scrollt
- Settings und Profilzugriff vorhanden sind
- Funktionen stabil mit Cloud-Sync-Ordnern nutzbar sind

