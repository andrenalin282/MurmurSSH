## Execution Protocol

You must follow this execution protocol during the work:

1. Read all listed guidance files first.
2. Extract the exact constraints and non-goals before changing code.
3. Identify the affected files and separate them by layer:
   - frontend UI
   - frontend API/types
   - backend commands
   - backend services
   - backend models
   - docs
4. Break the task into small implementation steps before broad editing.
5. Use sub-agents for complex, cross-layer, or unclear-root-cause work.
6. Maintain a live issue/finding list during implementation.
7. Resolve problems in priority order:
   - correctness/security/auth
   - broken flows
   - compile/type issues
   - meaningful warnings
   - UX clarity issues
8. Do not hide failed attempts or unresolved blockers.
9. Validate all required behaviors before declaring completion.

## Sub-Agent Triggers

Use sub-agents when one or more of the following are true:

- the task touches both `src/` and `src-tauri/src/`
- the task changes models plus UI behavior
- the task affects auth, secrets, host-key verification, or storage safety
- the root cause is not obvious
- more than 3 core modules are involved
- validation requires a separate review pass

Recommended sub-agent roles:
- architecture reviewer
- rust backend implementer
- frontend/state implementer
- qa/validation reviewer
- warning cleanup reviewer

## Required Final Report

At the end of the phase, provide:

1. Summary of changes
2. Files changed with one-line reason per file
3. Issue log with final status
4. Root cause(s)
5. Validation performed
6. Remaining issues / deferrals
7. Scope confirmation
