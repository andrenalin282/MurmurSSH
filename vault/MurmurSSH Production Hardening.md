# MurmurSSH Production Hardening

Quelle: `docs/reviews/production-readiness-audit.md`
PRD: `.claude/prompts/production-hardening-followup.md`

## Offene Punkte (Reihenfolge A → D)

| # | Ziel | Status |
|---|------|--------|
| A | F6 — SFTP Connect- und Transfer-Timeout trennen | pending |
| B | Echte Transfer-Cancellation (Single + rekursiv, Cleanup, UI-Reset) | pending |
| C | F9 — Download-Overwrite-Schutz lokal | pending |
| D1 | F17 — ControlMaster-Socket aus `/tmp` in App-Dir mit 0700 (optional) | pending |
| D2 | F14 — Portable `editor_command` First-Use-Bestätigung (optional) | pending |
| D3 | F8 — FTP Streaming (wahrscheinlich defer) | pending |

## Notizen

- `Obsidian-MCP-Server` in dieser Session nicht verfügbar → Notiz direkt in `vault/`.
- Jeder Punkt bekommt einen kleinen Commit / kurze Status-Zeile.
- Bei Größenüberschreitung: defer mit 1–2 Sätzen Begründung.

## Tests vor Release

1. Große Datei über gedrosselten Link (≥100 MB, ≤50 KB/s) hoch- und runterladen
2. Cancel während Einzeldatei- und Ordner-Transfer; Cleanup + UI prüfen
3. Download auf belegten lokalen Pfad ⇒ Overwrite-Dialog
4. Unreachable Host ⇒ Fail in ≤15 s
