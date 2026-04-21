---
name: pid-screenshot-worker
description: Implement and verify deterministic PID screenshot export and command-path behavior.
---

# PID Screenshot Worker

NOTE: Startup and cleanup are handled by `worker-base`. This skill defines the WORK PROCEDURE.

## When to Use This Skill

Use this skill for features that modify `PIDSHOT`, deterministic PNG export, screenshot helper behavior, or screenshot-focused tests. Typical files include `src/io/pid_screenshot.rs`, `src/app/commands.rs`, and directly related test surfaces.

## Required Skills

None.

## Work Procedure

1. Read the feature, mission files, and the current `src/io/pid_screenshot.rs` plus `PIDSHOT` command branch before deciding whether new implementation is actually needed.
2. Treat the current deterministic non-GPU PNG pipeline as the default architecture. Do not replace it with GPU readback unless the feature explicitly requires it and evidence shows the current approach cannot satisfy the contract.
3. Follow TDD:
   - add or tighten the smallest failing export/command test first
   - run only that focused test
   - implement the minimum change
   - re-run the focused test
4. Keep command UX strict:
   - active PID tab required
   - `.png` destination required
   - clear success and failure strings
5. Keep screenshot output deterministic:
   - fixed dimensions stay stable
   - output must be decodable and non-empty
   - avoid adding platform-dependent rendering paths
6. Run required verification commands and capture exact observations:
   - `cargo test export_pid_preview_png_writes_file -- --nocapture`
   - `cargo test export_rejects_empty_document -- --nocapture`
   - `cargo test pid_screenshot -- --nocapture`
   - `cargo check`
7. If command-path verification cannot be proven by tests in the repo, note exactly what command/path evidence exists and whether a follow-up validation feature is needed.

## Example Handoff

```json
{
  "salientSummary": "Completed the deterministic PID screenshot export surface and verified the command/helper path stays stable for the approved sample. The active PID tab can export PNG successfully, invalid contexts are rejected cleanly, and the helper remains suitable for regression testing.",
  "whatWasImplemented": "Updated src/io/pid_screenshot.rs and src/app/commands.rs so PIDSHOT consistently exports a deterministic PNG from the active PID tab, rejects invalid usage with clear command-line errors, and preserves fixed-dimension output suitable for the approved sample regression path.",
  "whatWasLeftUndone": "",
  "verification": {
    "commandsRun": [
      {
        "command": "cargo test export_pid_preview_png_writes_file -- --nocapture",
        "exitCode": 0,
        "observation": "The approved sample exported to a non-trivial PNG file on disk."
      },
      {
        "command": "cargo test export_rejects_empty_document -- --nocapture",
        "exitCode": 0,
        "observation": "Empty preview export still failed with the expected bounding-box error."
      },
      {
        "command": "cargo test pid_screenshot -- --nocapture",
        "exitCode": 0,
        "observation": "The full screenshot helper test slice stayed green, including deterministic regression-oriented checks."
      },
      {
        "command": "cargo check",
        "exitCode": 0,
        "observation": "The workspace compiled successfully after the screenshot-path changes."
      }
    ],
    "interactiveChecks": [
      {
        "action": "Reviewed PIDSHOT command-path constraints against mission requirements.",
        "observed": "The final command behavior still requires an active PID tab, a .png output path, and reports user-facing success/failure strings."
      }
    ]
  },
  "tests": {
    "added": [
      {
        "file": "src/io/pid_screenshot.rs",
        "cases": [
          {
            "name": "export_pid_preview_png_writes_file",
            "verifies": "The approved sample can be exported to a PNG file."
          },
          {
            "name": "export_rejects_empty_document",
            "verifies": "The helper rejects empty preview documents instead of writing meaningless output."
          }
        ]
      }
    ]
  },
  "discoveredIssues": []
}
```

## When to Return to Orchestrator

- The feature requires a fundamentally different screenshot architecture than the current deterministic export design
- The command path cannot be validated convincingly with existing repository tests and needs a separate UI confirmation feature
- PNG determinism conflicts with other in-progress PID display changes in a way that requires re-sequencing features
