# Phase 5.5 — Connection State UX & Theme System

You are acting as a senior Rust, Tauri 2, and Vite/TypeScript desktop application engineer working on MurmurSSH.

Your priorities, in order, are:

1. preserve product scope
2. preserve architecture
3. preserve Linux-native behavior
4. improve correctness and maintainability
5. keep the implementation understandable for open-source contributors

You must follow the repository guidance files and execution workflow exactly.
Do not expand scope.
Do not redesign the application unless the phase explicitly requires it.
Use sub-agents for complex or cross-layer work.
Track issues during the process and resolve them explicitly.
Validate before reporting completion.

## Ziel

Phase 5.5 behebt inkonsistente UI-Zustände und führt ein vollständiges Theme-System ein.

Schwerpunkte:

1. Terminal darf NICHT automatisch starten
2. Buttons müssen den tatsächlichen Verbindungszustand widerspiegeln
3. Sauberes Connected/Disconnected State Management
4. Theme-Support: Dark / Light / System
5. Persistente Theme-Einstellung

---

## Ziele im Detail

### 1. Terminal Auto-Launch entfernen

Aktuelles Verhalten:
- Terminal startet automatisch nach erfolgreichem Login

Neues Verhalten:
- Terminal startet nur bei Klick auf den Terminal-Button
- Kein impliziter Start mehr im Connect-Flow

---

### 2. Verbindungszustand korrekt abbilden

UI muss exakt dem Backend-Status folgen.

#### Disconnected

- Connect aktiv
- Terminal deaktiviert
- Disconnect deaktiviert
- File-Browser in neutralem Zustand

#### Connecting

- Connect deaktiviert
- Aktionen deaktiviert

#### Connected

- Disconnect aktiv
- Terminal aktiv
- Navigation aktiv

#### Disconnecting

- Aktionen deaktiviert

---

### 3. Einheitliches Connection-State-Modell

Empfohlenes Statusmodell:

- idle
- connecting
- connected
- disconnecting
- error

UI darf sich nicht auf implizite Annahmen verlassen.

---

### 4. Button-Aktivierung

Button-Zustände müssen zentral gesteuert werden.

Nicht erlaubt:
- Verteilte ad-hoc Aktivierung im Code

Erlaubt:
- zentrale updateUIState() Funktion

---

### 5. Theme-System

Unterstützte Modi:

- Dark
- Light
- System (OS-Vorgabe)

---

### 6. System-Theme

Bei System-Modus:

- Nutze prefers-color-scheme
- Reagiere auf Änderungen zur Laufzeit

---

### 7. Theme-Persistenz

Theme-Auswahl wird in settings.json gespeichert.

Default: System

---

### 8. Theme-Anwendung

Empfohlen:

- CSS-Klassen auf Root-Element:
  - theme-dark
  - theme-light

Kein Inline-Styling.

---

### 9. Settings-Dialog Erweiterung

Neue Sektion:

Appearance → Theme

Optionen:

- System
- Light
- Dark

---

## Funktionale Anforderungen

### A. Terminal-Verhalten

- Entferne Auto-Launch aus Connect-Logik
- Terminalstart nur über Button

---

### B. UI-State Synchronisation

Implementiere zentrale State-Verwaltung.

Alle Komponenten müssen darauf reagieren:

- Toolbar
- Sidebar
- File Browser
- Status Bar

---

### C. Disconnect-Verhalten

Nach Disconnect:

- Alle Session-bezogenen Buttons deaktivieren
- Terminal nicht mehr startbar
- Zustand zurück auf idle

---

### D. Theme-System

Beim Start:

- Theme aus Settings laden
- Falls System → OS-Theme anwenden

Bei Änderung:

- sofort anwenden
- speichern

---

## Akzeptanzkriterien

1. Terminal startet nicht automatisch.
2. Terminal öffnet nur bei Button-Klick.
3. Connect/Disconnect/Terminal spiegeln den tatsächlichen Status.
4. Nach Disconnect sind Terminal und Navigation deaktiviert.
5. Dark-Mode funktioniert korrekt.
6. Light-Mode funktioniert korrekt.
7. System-Mode folgt OS-Einstellung.
8. Theme wird persistent gespeichert.
9. UI aktualisiert sich sofort bei Theme-Wechsel.

---

## Empfohlene Umsetzungsschritte

1. Connect-Flow analysieren und Auto-Terminal entfernen
2. Connection-State zentralisieren
3. UI-State-Update-Funktion implementieren
4. Button-Zustände daran koppeln
5. Theme-Option zu Settings hinzufügen
6. CSS-Klassen-System implementieren
7. System-Theme Listener hinzufügen
8. Integration testen

---

## Manuelle Testfälle

### Terminal

- Connect → kein Terminal
- Button klicken → Terminal startet
- Disconnect → Button deaktiviert

### Buttons

- Statuswechsel korrekt sichtbar
- Keine falschen aktivierten Aktionen

### Theme

- Wechsel Dark/Light/System
- Persistenz nach Neustart
- System-Modus reagiert auf OS-Änderung
