# Phase 5.7 — Beta Readiness & Local Path Integration

## Ziel

Phase 5.7 schließt Phase 5 funktional ab und macht MurmurSSH bereit für eine erste Beta-Version.

Schwerpunkte:

1. Lokaler Arbeitsordner pro Profil (Local Path)
2. Konsistentes Download-/Upload-Verhalten
3. Connected Profile Locking
4. Letzte UI-State-Härtung
5. Kleine UX-Verbesserungen für produktiven Einsatz

---

## 1. Local Path pro Profil

### Neues Profilfeld

Local Path (optional):

- Lokales Arbeitsverzeichnis für Upload/Download
- Wird im Profil gespeichert
- Plattformunabhängig als String

UI:

- Textfeld
- Button „Folder auswählen“
- Icon: Folder + (oder nur Folder)

Button öffnet System-Ordnerdialog.

---

### Verhalten

Wenn Local Path gesetzt:

- Upload startet aus diesem Verzeichnis
- Download speichert standardmäßig dorthin
- Kein Dialog erforderlich

Wenn nicht gesetzt:

- Download → Speicherortdialog öffnen
- Upload → Dateiauswahl wie bisher

---

## 2. Download-Verhalten

Aktuelles Problem:
Download kann ohne sichtbare Aktion enden.

Neues Verhalten:

### Mit Local Path
- Datei wird direkt gespeichert
- Erfolgsmeldung optional

### Ohne Local Path
- Save-Dialog anzeigen
- Benutzer wählt Zielpfad

---

## 3. Upload-Verhalten

Upload sollte ebenfalls den Local Path berücksichtigen:

- Datei-Auswahl startet im Local Path
- Fallback: letzter verwendeter Ordner oder System-Default

---

## 4. Connected Profile Locking

Während ein Profil verbunden ist:

### Gesperrt:

- Edit
- Delete
- Auth-Parameter ändern
- Remote Path ändern

UI:

- Buttons disabled
- Optional Tooltip: „Profil ist verbunden“

Disconnect hebt Sperre auf.

---

## 5. File-Browser State Hardening

Probleme nach Verzeichniswechsel müssen ausgeschlossen werden.

Sicherstellen:

- Toolbar-Buttons bleiben klickbar
- Connection-State bleibt erhalten
- Navigation beeinflusst nicht die Session

---

## 6. Plus-Button (Create)

Im File Browser unten:

Button „+“

Öffnet Menü:

- New File
- New Folder

---

## 7. Breadcrumb Input Mode

Breadcrumbs erhalten optionalen Edit-Modus:

- Klick oder Shortcut aktiviert Input-Feld
- Benutzer kann Pfad manuell eingeben
- Enter → Navigation
- Escape → Abbruch

---

## 8. Download Dialog Integration

Download-Funktion muss garantiert eine sichtbare Aktion auslösen:

- Direkt speichern (Local Path)
- Oder Speicherort wählen

Keine „silent failure“.

---

## 9. Persistenz

Local Path wird im Profil gespeichert.

Keine Migration alter Profile erforderlich.

---

## Akzeptanzkriterien

1. Local Path kann pro Profil gesetzt werden.
2. Folder-Dialog funktioniert plattformübergreifend.
3. Downloads speichern korrekt.
4. Upload startet im Local Path.
5. Verbundenes Profil kann nicht editiert oder gelöscht werden.
6. Toolbar bleibt funktionsfähig nach Navigation.
7. Plus-Button ermöglicht Datei-/Ordnererstellung.
8. Breadcrumb-Pfad kann manuell eingegeben werden.
9. Verhalten ist konsistent und ohne Silent-Fails.

---

## Empfohlene Umsetzungsschritte

1. Profilmodell erweitern
2. Settings-Dialog anpassen
3. Download-Logik umbauen
4. Upload-Startverzeichnis setzen
5. Connected-Locking implementieren
6. File-Browser UI erweitern
7. Breadcrumb Edit-Modus
8. Integrationstest

---

## Manuelle Tests

### Local Path

- Setzen → speichern → neu laden → erhalten

### Download

- Mit Local Path → direkt gespeichert
- Ohne Local Path → Dialog erscheint

### Upload

- Datei-Auswahl startet im Local Path

### Locking

- Verbinden → Edit/Delete deaktiviert
- Disconnect → wieder aktiv

### Breadcrumb Input

- Pfad eingeben → Navigation
