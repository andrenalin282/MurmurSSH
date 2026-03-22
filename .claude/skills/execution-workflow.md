# Execution Workflow Rules – MurmurSSH

## Purpose

These rules define how Claude Code should execute work in this repository.

They complement the product, architecture, Linux, and open-source skills by enforcing a consistent delivery process:
- decompose before changing
- use sub-agents for complex work
- track issues and warnings explicitly
- validate before declaring completion
- keep scope tight and architecture stable

These rules are mandatory for all implementation tasks unless the user explicitly instructs otherwise.

---

## Primary Execution Principles

1. Read before changing
   - Read `CLAUDE.md`
   - Read `PRD.md`
   - Read all guidance skills listed in `CLAUDE.md`
   - Read the current phase prompt and any earlier phase prompts that are referenced
   - Read the relevant source files before editing them

2. Preserve the existing architecture
   - Keep the layering: services -> commands -> frontend API -> UI
   - Do not move business logic into the wrong layer
   - Do not collapse modules for convenience
   - Do not introduce heavy abstractions unless they clearly reduce complexity

3. Stay within scope
   - Follow MVP and phase boundaries strictly
   - Do not add adjacent features “while here”
   - Do not redesign the UI when only flow or validation changes are required
   - Do not change storage strategy unless the task explicitly requires it

4. Prefer small, verifiable steps
   - Break work into narrow implementation steps
   - Change one concern at a time where practical
   - Validate intermediate steps before moving on

5. Surface problems clearly
   - Do not hide failed attempts
   - Do not silently ignore warnings, broken assumptions, or incomplete behavior
   - Record issues and resolve them in priority order

---

## Required Work Sequence

For any non-trivial task, execute in this order:

1. Context loading
   - Read all required project guidance
   - Read the phase prompt
   - Read the relevant code paths

2. Constraint extraction
   - Identify what must be preserved
   - Identify what is explicitly out of scope
   - Identify compatibility requirements
   - Identify Linux/Tauri/Rust/Vite constraints that matter

3. Affected file mapping
   - List the files that will likely be touched
   - Separate them by layer:
     - frontend UI
     - frontend API/types
     - backend commands
     - backend services
     - backend models
     - docs

4. Task decomposition
   - Split the work into small units
   - Order them by dependency
   - Prefer foundation first, UI wiring after, cleanup last

5. Sub-agent delegation when needed
   - Use sub-agents when the work crosses multiple layers or has unclear root cause
   - Consolidate findings before implementation

6. Implementation
   - Apply changes in small coherent batches
   - Keep naming consistent with existing code
   - Preserve backward compatibility where required

7. Issue and warning pass
   - Review compiler warnings, type issues, state inconsistencies, and UX regressions
   - Fix them one by one where they are in scope
   - Document anything intentionally deferred

8. Validation
   - Validate against the phase requirements
   - Validate affected flows end-to-end
   - Validate build/dev assumptions as far as possible

9. Final report
   - Provide the required final report structure exactly
   - Include remaining issues if any

---

## Sub-Agent Rules

Sub-agents should be used for complex, cross-layer, or high-risk tasks.

### Mandatory sub-agent usage

Use sub-agents when any of the following are true:

- the task touches both `src/` and `src-tauri/src/`
- the task includes backend model changes plus frontend UI changes
- the task includes authentication, secrets, host-key, or security-sensitive logic
- the task includes state flow across multiple UI components
- the task requires root-cause analysis for unclear bugs
- the task affects more than 3 core modules
- the task mixes implementation and structural review
- the task includes both behavior changes and migration/backward-compatibility requirements

### Recommended sub-agent roles

Use role-focused sub-agents where helpful, for example:

- **architecture reviewer**
  - checks layering, module boundaries, persistence rules, and scope discipline

- **rust backend implementer**
  - focuses on models, services, commands, error propagation, and Linux-native backend behavior

- **frontend/state implementer**
  - focuses on `src/`, DOM components, typed API usage, state transitions, and UI validation/messages

- **qa/validation reviewer**
  - checks edge cases, regressions, acceptance criteria, and end-to-end flow consistency

- **warning cleanup reviewer**
  - identifies compiler warnings, dead code, naming inconsistencies, and incomplete cleanup

### Sub-agent operating rules

- Sub-agents should investigate or implement a narrow concern only
- They must not expand scope
- Their findings must be consolidated before broad changes are applied
- If sub-agents disagree, prefer the simpler architecture-preserving option

---

## Issue Log Requirement

For every substantial task, maintain a live issue/finding list during the work.

Each item should include:

- ID or short label
- problem
- likely root cause
- affected file(s)
- status: `open` | `in_progress` | `resolved` | `deferred`
- verification note

### Priority order

Resolve in this order unless the phase says otherwise:

1. correctness / data loss / auth / security issues
2. broken user flows
3. type or compile errors
4. warnings that indicate real logic or lifecycle problems
5. UX clarity issues
6. cleanup items

### Rules

- Do not mark an item resolved without stating how it was verified
- Do not omit deferred issues from the final report
- Do not bury warnings under a summary paragraph

---

## Validation Rules

Before declaring completion, validate:

### Functional validation
- each required phase behavior is present
- existing supported flows still work
- error paths remain understandable
- storage behavior remains compatible with project rules

### Architectural validation
- logic remains in the correct layer
- no unnecessary dependency or framework was introduced
- persistence strategy remains aligned with project guidance
- frontend/backend type alignment is preserved

### UX validation
- no new browser-native prompts where in-app dialogs are expected
- no confusing silent failures
- minimal UI remains minimal
- labels and status messages are clear and specific

### Safety validation
- no sensitive secret handling was weakened unintentionally
- no local-only data was moved to broader scope
- no security messaging was made misleading

If full runtime validation is not possible, state exactly what was checked and what remains unverified.

---

## Required Final Report Format

Every implementation response must end with this structure:

### 1. Summary of changes
Short explanation of what was implemented.

### 2. Files changed
List each modified file with a one-line reason.

### 3. Issue log
List the key issues found during the work and their final status.

### 4. Root cause(s)
Explain the actual cause of the main problems solved.

### 5. Validation performed
State what was checked:
- code paths reviewed
- manual flow checks
- build/type/logic checks
- limits of verification

### 6. Remaining issues / deferrals
Anything intentionally not solved yet.

### 7. Scope confirmation
Explicitly confirm that the work stayed within phase and product scope.

---

## Anti-Patterns

Do not do the following:

- change many unrelated files without first decomposing the task
- mix architecture refactor with feature work unless required
- silently introduce additional features
- leave warnings unexplained
- report success without checking acceptance criteria
- rewrite working modules just because another approach seems cleaner
- move Linux-native behavior toward a less native abstraction
- replace simple maintainable code with framework-like structure

---

## Decision Bias

When multiple valid implementations exist, prefer:

1. the smaller change
2. the more Linux-native option
3. the option with clearer layer boundaries
4. the option with fewer dependencies
5. the option easier for open-source contributors to understand
