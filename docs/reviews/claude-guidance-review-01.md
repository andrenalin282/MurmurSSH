# Claude Guidance Review 01

Date: 2026-03-20

---

Here is the file-by-file review.

---

## CLAUDE.md

This file is the most outdated — it was written before the guidance docs existed and hasn't been updated.

**1. Stale "Project Status" section (line 7)**
> "No source code, build system, or README exists yet. The PRD and product scope files are present but empty."

All five guidance files are now populated. This claim is false and misleading for any future Claude instance.

**2. Speculative/uncertain language in "Project Intent" (line 11)**
> "Based on the name and scaffolding, this appears to be an SSH-related tool or application."

The project is now well-defined. Replace with a factual one-line description drawn from the PRD, e.g. _"MurmurSSH is a minimal open-source Linux desktop client for SSH and SFTP, built with Tauri and Rust, targeting Ubuntu and distributed as a `.deb` package."_

**3. Conditional that no longer applies (line 11)**
> "Once the PRD.md and `.claude/skills/product-scope.md` are filled in, consult them for requirements and scope before implementing anything."

The condition is already met. The instruction should be unconditional: "Before implementing anything, read the files listed below."

**4. Missing files in Repository Layout (lines 15–20)**
`architecture-rules.md`, `linux-integration.md`, and `open-source-guidelines.md` are all absent from the listing.

**5. No mention of tech stack**
There is no reference to Tauri or Rust anywhere in CLAUDE.md. A future Claude instance would have no quick anchor for the preferred stack without digging into the PRD.

**6. Development Notes only points to two files (lines 24–25)**
Three skills files go unmentioned. The guidance to read all of them before starting work should be here.

---

## PRD.md

Generally well-written. Three targeted issues:

**1. Profile field `SSH private key path` listed without a conditional (§7.1, line 86)**
The profile model lists "authentication type" and "SSH private key path" as separate fields, but gives no indication that key path is conditional on auth type. If a user picks password auth (even in a future build), the field is irrelevant. Suggest adding "(required when auth type is key-based)" or marking it optional.

**2. Weak/tentative frontend language (§11, line 188)**
> "TypeScript-based UI is acceptable"

"Acceptable" reads like a reluctant concession rather than a chosen direction. Given the Tauri stack, TypeScript is the expected frontend choice. Suggest: "TypeScript-based frontend" (drop "is acceptable").

**3. Problematic qualifier in security section (§9, line 168)**
> "avoid storing secrets insecurely unless explicitly chosen"

The "unless explicitly chosen" clause implies the app can be intentionally configured to store secrets insecurely. This is the kind of escape hatch that erodes the security stance. If there's no in-scope feature that would use it, the qualifier should be removed: "avoid storing secrets insecurely."

---

## .claude/skills/product-scope.md

Tight and consistent. No contradictions found. One minor wording note:

**1. "Out of Scope" list has no qualifier phrasing (line 19)**
The heading says _"Do not add these unless the user explicitly changes scope"_ — which is good. However `AppImage` appears here while PRD §4 lists it as a "possible later community extension." This is not a contradiction (MVP vs. future), but a quick parenthetical like `AppImage (MVP only restriction)` would prevent a future reader from thinking AppImage is permanently banned.

---

## .claude/skills/architecture-rules.md

Solid. One gap worth flagging:

**1. Profile file format unspecified (lines 36–37)**
> "Profiles should be stored as individual local files where practical. Use human-readable formats when possible."

"Human-readable formats" is vague — it could mean JSON, TOML, YAML, INI. PRD §7.5 lists `settings.json`, implying JSON is the chosen format for at least one file. This inconsistency in specificity could lead to different formats being used for profiles vs. settings. A concrete call — even just "prefer JSON or TOML" — would remove ambiguity.

---

## .claude/skills/linux-integration.md

No contradictions or ambiguities found. Consistent with PRD and product-scope throughout.

---

## .claude/skills/open-source-guidelines.md

No issues found. Clean and consistent.

---

## .claude/prompts/phase-1-foundation.md

**1. Task 7 is vague (line 36)**
> "Prepare the project for Linux development and future `.deb` packaging"

Every other task in the list is concrete (initialize, create structure, add GUI shell, add models, etc.). "Prepare" is undefined. Suggest specifying what this means in context, e.g.: "Ensure Tauri is configured for Linux builds and that packaging metadata supports `.deb` generation."

**2. File watcher service not explicitly listed in Task 6 (line 33)**
`architecture-rules.md` names a "file watcher service" as a distinct suggested module, but Task 6 only lists "workspace/edit flow." The watcher is implied but not stated. Adding it explicitly would keep the prompt aligned with the architecture doc.

---

## Cross-File Consistency Check

| Claim | PRD | product-scope | architecture | linux | phase-1 |
|---|---|---|---|---|---|
| System terminal for SSH | ✓ | ✓ | — | ✓ | ✓ |
| `.deb` only | ✓ | ✓ | ✓ | ✓ | ✓ |
| No embedded terminal | ✓ | ✓ | — | ✓ | ✓ |
| `~/.config/murmurssh/` | ✓ | — | — | ✓ | — |
| No database for MVP | ✓ | ✓ | ✓ | — | ✓ |
| Key auth preferred | ✓ | ✓ | — | ✓ | — |
| Tauri + Rust | ✓ | — | ✓ | — | — |

Minor gap: `~/.config/murmurssh/` is stated in PRD and linux-integration, but not in product-scope or architecture-rules. Not a contradiction, just light coverage. The Tauri+Rust stack is only stated in PRD and architecture-rules — product-scope and linux-integration don't mention it, which is fine given their purpose.

---

**Summary — highest priority to fix:**
1. CLAUDE.md needs a full rewrite of its top sections (stale content, missing stack, incomplete file list)
2. PRD §9 security qualifier should be removed
3. Phase-1 Task 7 should be made concrete
4. Architecture-rules should call out a preferred profile file format

Let me know if you'd like me to apply any or all of these changes.
