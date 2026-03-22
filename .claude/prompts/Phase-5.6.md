# Phase 5.6 — Browser State Hardening, Path UX & Download/Create Flow

## Ziel

Phase 5.6 behebt die nächsten drei sinnvollen Produktprobleme und integriert die neu gemeldeten UX-Anforderungen:

1. Browser-/Toolbar-State hardening
2. Pfad- und Navigation-UX verbessern
3. Download-/Create-Flow vervollständigen

Zusätzlich soll die Phase den Hetzner-/Symlink- bzw. Pfad-Sonderfall sauber prüfen, da `public_html` im Terminal erreichbar ist, im File-Browser aber nicht sauber nutzbar erscheint.

---

## Die 3 nächsten sinnvollen Punkte

### 1. Browser State Hardening
Die Toolbar-Buttons (`Disconnect`, `Terminal`, `Home`, `Up`, `Refresh`) dürfen nach Verzeichniswechsel oder Fehlerzuständen nicht unbenutzbar werden, solange eine Verbindung aktiv ist.

### 2. Path UX / Manual Path Navigation
Breadcrumbs sollen nicht nur klickbar, sondern zusätzlich als editierbares Pfadfeld nutzbar sein:
- aktueller Pfad sichtbar
- manuell editierbar
- Enter navigiert
- bei Fehler im alten gültigen Pfad bleiben

### 3. Download + Create Flow vervollständigen
- Download muss einen echten Zielpfad-Flow haben
- wenn kein Standard-Downloadpfad für das Profil hinterlegt ist, Speicherort abfragen
- unten einen Plus-Button ergänzen für:
  - Datei erstellen
  - Ordner erstellen

---

## Zusätzlich gemeldete Anforderungen

### A. Hetzner / public_html / directory listing prüfen
Es muss geprüft werden, warum `public_html` im Dateibrowser nicht sauber als Ordner nutzbar ist, obwohl:
- der Ordner im Terminal vorhanden ist
- direkter Startpfad funktioniert

Mögliche Ursachen, die geprüft werden müssen:
- Symlink-Behandlung in SFTP-Listing
- falsche is_dir-Erkennung
- Sonderfall bei Dateiattributen / lstat vs stat
- UI rendert Eintrag falsch
- Navigation/Filter schließt den Eintrag fälschlich aus

Wichtig:
Nicht raten, sondern Root Cause ermitteln und dokumentieren.

### B. Buttons nach Verzeichniswechsel
Nach Navigation oder Permission-Error dürfen verbundene Aktionen nicht unclickable werden.

### C. Editierbares Pfadfeld
Breadcrumb-Zeile soll um ein Path-Input erweitert oder ersetzt werden:
- klickbare Breadcrumbs dürfen erhalten bleiben
- zusätzlich manuelles Path-Editing
- Enter = navigate
- Escape oder Blur kann optional reset auf aktuellen gültigen Pfad machen

### D. Download-Zielpfad
Wenn kein profilbezogener Standardpfad existiert:
- system file save dialog / directory picker verwenden
- Download darf nicht still scheitern
- Erfolg/Fehler sichtbar anzeigen

### E. Create-Menü unten
Unten im Filebrowser:
- Plus-Button
- kleines Menü oder 2 Aktionen:
  - New File
  - New Folder

Minimal ausreichend:
- Dialog mit Name
- Erstellung im aktuellen Verzeichnis
- anschließend Refresh

---

## Nicht-Ziele

Diese Phase macht noch nicht:
- vollständigen Mehrfach-Download
- Drag & Drop
- Inline-Umbenennen kompletter Listenansichten
- umfassende Dateibearbeitung
- Upload-Warteschlangen
- Rechte-/Owner-Editor
- komplexe Symlink-Verwaltung über die UI

---

## Funktionale Anforderungen

## 1. Toolbar/Connection State Hardening

### 1.1
Solange Verbindung aktiv:
- Disconnect klickbar
- Terminal klickbar
- Home klickbar
- Up klickbar
- Refresh klickbar

### 1.2
Verzeichniswechsel oder Listing-Fehler dürfen diese Buttons nicht deaktivieren, sofern keine echte Disconnect-Situation vorliegt.

### 1.3
Button-Aktivierung muss am tatsächlichen Connection State hängen, nicht am aktuell gerenderten Listenstatus.

---

## 2. public_html / Directory Detection Fix

### 2.1 Analyse
Prüfen:
- wie Directory Entries aus SFTP gelesen werden
- ob `public_html` als symlink geliefert wird
- ob symlinks auf Verzeichnisse korrekt als navigierbare Ordner behandelt werden
- ob Startpfad-Navigation und Listing unterschiedliche Codepfade nutzen

### 2.2 Erwartetes Verhalten
Wenn `public_html` ein Verzeichnis oder Verzeichnis-Symlink ist und betreten werden kann:
- im Filebrowser als navigierbarer Ordner behandeln
- Doppelklick / Enter / Klick soll funktionieren

### 2.3 Fehlerfall
Wenn ein Eintrag tatsächlich nicht betretbar ist:
- klare Fehlermeldung
- im bisherigen gültigen Pfad bleiben

---

## 3. Path UX

### 3.1 Breadcrumbs
Breadcrumbs bleiben klickbar.

### 3.2 Path Input
Zusätzlich editierbares Pfadfeld:
- zeigt aktuellen gültigen Pfad
- Benutzer kann Pfad manuell eingeben
- Enter startet Navigation
- bei Fehler:
  - Inline-Fehler anzeigen
  - currentPath bleibt unverändert
  - Input springt zurück auf gültigen Pfad

### 3.3 Home / Up Verhalten
- Home auf Profil-Startpfad
- Up parent path
- Root stabil behandeln

---

## 4. Download Flow

### 4.1 Profiloption
Optionaler profilbezogener Default-Downloadpfad darf unterstützt werden, falls sinnvoll im bestehenden Modell.

### 4.2 Kein Default gesetzt
Wenn kein Default vorhanden:
- Save-/Folder-Dialog öffnen
- Zielpfad vom Benutzer wählen lassen

### 4.3 Mit Default gesetzt
Wenn Default vorhanden:
- dorthin speichern
- bei Konflikt/Fehler sichtbare Meldung

### 4.4 UX
Download darf nie “nichts tun”.
Es muss mindestens eines passieren:
- Dialog
- Fortschritt / Status
- klare Fehlermeldung
- Erfolgsmeldung

---

## 5. Create Flow

### 5.1 Plus-Button
Im unteren Bereich des Filebrowsers einen Plus-Button ergänzen.

### 5.2 Aktionen
Mindestens:
- New File
- New Folder

### 5.3 Dialog
Nach Auswahl:
- Namen abfragen
- im aktuellen Verzeichnis anlegen
- Liste refreshen
- Fehler anzeigen, falls Erstellung fehlschlägt

---

## 6. Settings / Profile-Modell nur wenn nötig erweitern

Nur falls für Downloadpfad sinnvoll:
- profilbezogener default_download_path

Nicht unnötig erweitern, wenn der Systemdialog ohne Modelländerung für diese Phase reicht.

---

## Akzeptanzkriterien

1. Toolbar-Buttons bleiben nach Verzeichniswechsel benutzbar, solange verbunden.
2. `Disconnect` und `Terminal` werden nicht durch Listing-/Navigation-Fehler unclickable.
3. `public_html`-Problem ist analysiert und behoben oder mit klar dokumentierter Ursache abgegrenzt.
4. Verzeichnis-Symlinks werden korrekt behandelt, falls dies die Ursache ist.
5. Breadcrumbs bleiben klickbar.
6. Es gibt ein editierbares Pfadfeld.
7. Manueller Pfadwechsel per Enter funktioniert.
8. Bei ungültigem Pfad bleibt der Browser im letzten gültigen Verzeichnis.
9. Download öffnet einen Zielpfad-Dialog, wenn kein Standardpfad gesetzt ist.
10. Download zeigt sichtbares Ergebnis statt still nichts zu tun.
11. Unten gibt es einen Plus-Button für Datei/Ordner erstellen.
12. Neue Datei/Ordner werden im aktuellen Verzeichnis erstellt und danach angezeigt.

---

## Empfohlene Umsetzungsschritte

1. Guidance-Dateien lesen, Constraints/Nicht-Ziele extrahieren
2. Root Cause für unclickable Toolbar-Buttons finden
3. Connection/UI state gegen Listing state entkoppeln
4. `public_html`/symlink/listing Verhalten untersuchen
5. Directory detection fixen
6. Pfadfeld + Enter-Navigation implementieren
7. Download-Flow mit Zielpfad-Dialog ergänzen
8. Plus-Button + create file/folder Flow ergänzen
9. Manuelle Validierung
10. Abschlussreport mit Issue Log

---

## Manuelle Testfälle

### A. Toolbar State
- verbinden
- mehrere Verzeichniswechsel
- Permission denied provozieren
- prüfen, dass Disconnect/Terminal/Home/Up/Refresh weiter klickbar bleiben

### B. public_html
- Hetzner-Profil nutzen
- `public_html` im Listing öffnen
- wenn Symlink: prüfen, dass Navigation funktioniert
- mit direktem Startpfad vergleichen

### C. Path Input
- gültigen Pfad manuell eingeben → Enter → Navigation erfolgreich
- ungültigen Pfad eingeben → Fehler, alter Pfad bleibt

### D. Download
- Datei auswählen
- ohne Default-Zielpfad → Dialog erscheint
- mit Ziel speichern → Datei landet dort
- Fehler sichtbar

### E. Create
- New Folder → Name eingeben → erscheint in Liste
- New File → Name eingeben → erscheint in Liste

---

## Definition of Done

Phase 5.6 ist abgeschlossen, wenn:
- Toolbar-State robust ist
- manuelle Pfadnavigation vorhanden ist
- Download nicht mehr still scheitert
- Datei/Ordner-Erstellung möglich ist
- das `public_html`-Problem sauber gelöst oder belastbar dokumentiert ist
- der Abschlussreport vollständig dem Ausführungsprotokoll entspricht
