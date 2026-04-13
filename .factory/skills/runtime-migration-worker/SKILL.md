---
name: runtime-migration-worker
description: Reduce low-risk acadrust dependency surface while preserving mixed compat/native compile and regression behavior.
---

# Runtime Migration Worker

NOTE: Startup and cleanup are handled by worker-base. This skill defines the work procedure for quick-win runtime migration features.

## When to Use This Skill

Use this skill for features that:
- migrate Handle/Color/LineWeight-only usage in `src/` files
- add thin aliases or re-exports needed for those quick wins
- strengthen compile/regression coverage around mixed compat/native runtime paths

This skill is **not** for deep refactors of `src/entities`, `src/scene`, or large `src/app` dispatch code.

## Required Skills

- `tdd` — invoke before changing code; add failing compile/test coverage first where possible.
- `verification-before-completion` — invoke before ending the session.
- `systematic-debugging` — invoke if compile errors cascade beyond the assigned quick-win scope.

## Work Procedure

1. Read `mission.md`, `validation-contract.md`, `AGENTS.md`, `.factory/library/architecture.md`, `.factory/library/environment.md`, and `.factory/services.yaml`.
2. Confirm the assigned files are truly quick-win migration targets (mostly Handle/Color/LineWeight consumers). If the feature expands into deeper compat/entity-model logic, stop and return to the orchestrator.
3. Characterize the current compile/test surface before changes:
   - identify dependent command/app files
   - add or expand regression coverage only where it provides clear evidence
4. Write failing coverage first where practical:
   - compile-surface oriented tests or assertions in existing regression files
   - existing runtime sync/property tests expanded when the feature fulfills a runtime regression assertion
5. Implement the smallest possible migration:
   - prefer aliases/re-exports or direct type substitution over architectural rewrites
   - keep behavior identical
6. Run targeted compile checks first, then broader workspace checks:
   - `cargo check`
   - `cargo test -- --test-threads=16` when the feature touches existing runtime regression tests
7. If the feature fulfills `VAL-CROSS-002`, make sure at least one existing native-backed sync/property test still passes after the migration.
8. Invoke `verification-before-completion`, then hand off with a precise list of files changed and any remaining deeper-coupling boundaries.

## Example Handoff

```json
{
  "salientSummary": "Migrated quick-win Handle-only module files to native handle usage, kept mixed compat/native compile surfaces green, and preserved an existing native-backed property sync regression.",
  "whatWasImplemented": "Updated selected src/modules and src/ui files that only consumed Handle/Color/LineWeight, added the minimal aliasing required for mixed compat/native code, and verified the dependent command/app compile surface still builds without changing deeper entity-dispatch architecture.",
  "whatWasLeftUndone": "",
  "verification": {
    "commandsRun": [
      {
        "command": "cargo check",
        "exitCode": 0,
        "observation": "Workspace compile surface stayed green after the quick-win migration."
      },
      {
        "command": "cargo test -- --test-threads=16",
        "exitCode": 0,
        "observation": "Existing runtime regression coverage still passed after the migration."
      }
    ],
    "interactiveChecks": [
      {
        "action": "Compile mixed compat/native runtime surfaces and rerun native-backed sync regression tests",
        "observed": "Migrated files compiled cleanly and the targeted sync/property path still passed."
      }
    ]
  },
  "tests": {
    "added": [
      {
        "file": "src/app/properties.rs",
        "cases": [
          {
            "name": "commit_entity_syncs_native_viewport_in_paper_space",
            "verifies": "VAL-CROSS-002"
          }
        ]
      }
    ]
  },
  "discoveredIssues": []
}
```

## When to Return to Orchestrator

- The change requires rewriting deep compat-coupled systems instead of staying in the quick-win lane.
- The feature cannot be completed without changing `acadrust`.
- The compile fallout indicates the selected files were not actually low-risk migration targets.
