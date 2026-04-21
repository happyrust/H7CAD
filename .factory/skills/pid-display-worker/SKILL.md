---
name: pid-display-worker
description: Implement and verify PID real-sample open/display fidelity and fit behavior.
---

# PID Display Worker

NOTE: Startup and cleanup are handled by `worker-base`. This skill defines the WORK PROCEDURE.

## When to Use This Skill

Use this skill for features that change the approved PID sample’s open path, preview construction, display density, or viewport-fit behavior. Typical files include `src/io/pid_import.rs`, `src/app/update.rs`, `src/io/mod.rs`, and directly related scene code.

## Required Skills

None.

## Work Procedure

1. Read the mission files, `.factory/library/architecture.md`, `.factory/library/environment.md`, `.factory/library/user-testing.md`, and the assigned feature carefully before touching code.
2. Inspect existing target-sample tests in `src/io/pid_import.rs` first. Prefer tightening or extending those tests over creating duplicate coverage elsewhere.
3. Follow TDD:
   - add or adjust the smallest failing target-sample test first
   - run only that focused test to see the real failure
   - implement the minimum change needed
   - re-run the focused test
4. If the failure is preview-density or layer-balance related, prefer fixing `src/io/pid_import.rs` rather than pushing ad hoc UI hacks into app/runtime code.
5. If the failure is clearly open-flow or fit-related, make the narrowest change in `src/app/update.rs` or scene fit helpers needed to keep the main PID drawing visually dominant.
6. Manually review whether decorative PID layers (`PID_META`, `PID_FALLBACK`, `PID_CROSSREF`, etc.) are accidentally being allowed to dominate the viewport.
7. Run the required feature verification commands from the feature and include exact observations:
   - `cargo test open_target_pid_sample_builds_dense_preview -- --nocapture`
   - `cargo test target_pid_preview_layout_is_primary_visual_focus -- --nocapture`
   - `cargo test target_pid_sample_fit_layers_matching_succeeds_for_main_drawing_layers -- --nocapture`
   - `cargo test target_pid_sample_scene_has_fittable_geometry_and_native_doc -- --nocapture`
   - `cargo test pid_import -- --nocapture`
   - `cargo check`
8. If any required verification fails and the root cause is outside the feature or ambiguous, return to orchestrator instead of guessing.

## Example Handoff

```json
{
  "salientSummary": "Hardened the approved PID sample’s display path so the main drawing remains the visual focus after open. Tightened the target-sample tests around preview density and layer-targeted fit, then adjusted the open/fit behavior to preserve a usable PID-first view.",
  "whatWasImplemented": "Updated the PID display/open path in src/io/pid_import.rs and src/app/update.rs so the approved sample at D:\\work\\plant-code\\cad\\pid-parse\\test-file\\工艺管道及仪表流程-1.pid continues to produce non-trivial preview density, keeps primary PID layers visible, and fits to the main drawing before falling back to whole-document fit. Added/updated target-sample tests to anchor these behaviors.",
  "whatWasLeftUndone": "",
  "verification": {
    "commandsRun": [
      {
        "command": "cargo test open_target_pid_sample_builds_dense_preview -- --nocapture",
        "exitCode": 0,
        "observation": "Approved sample opened and printed stable layout/object/native-entity counts above the regression floor."
      },
      {
        "command": "cargo test target_pid_preview_layout_is_primary_visual_focus -- --nocapture",
        "exitCode": 0,
        "observation": "Primary PID drawing layer entities remained present and decorative layers no longer became the only visible anchor."
      },
      {
        "command": "cargo test target_pid_sample_fit_layers_matching_succeeds_for_main_drawing_layers -- --nocapture",
        "exitCode": 0,
        "observation": "Main-layer-focused fit succeeded for the approved sample without relying on unconditional fit_all."
      },
      {
        "command": "cargo test target_pid_sample_scene_has_fittable_geometry_and_native_doc -- --nocapture",
        "exitCode": 0,
        "observation": "Scene contained native preview, compat entities, and non-empty wires so fit_all remained meaningful."
      },
      {
        "command": "cargo test pid_import -- --nocapture",
        "exitCode": 0,
        "observation": "Broader PID import test slice stayed green after the display-path change."
      },
      {
        "command": "cargo check",
        "exitCode": 0,
        "observation": "Workspace compiled successfully for the PID-related surfaces touched by the feature."
      }
    ],
    "interactiveChecks": [
      {
        "action": "Reviewed the target-sample layer distribution and fit path assumptions against the mission architecture and the current PID open branch.",
        "observed": "The final change still routes through the normal open flow and keeps the main drawing visually prioritized over decorative PID layers."
      }
    ]
  },
  "tests": {
    "added": [
      {
        "file": "src/io/pid_import.rs",
        "cases": [
          {
            "name": "open_target_pid_sample_builds_dense_preview",
            "verifies": "The approved sample continues to derive a non-trivial layout and preview."
          },
          {
            "name": "target_pid_preview_layout_is_primary_visual_focus",
            "verifies": "Primary PID drawing entities remain the visual focus over decorative layers."
          }
        ]
      }
    ]
  },
  "discoveredIssues": []
}
```

## When to Return to Orchestrator

- The approved sample path is unavailable or no longer stable in the environment
- A required display fix depends on creating a broader rendering architecture not scoped to PID
- Focused PID display tests pass but the feature still requires true UI automation evidence outside available tooling
