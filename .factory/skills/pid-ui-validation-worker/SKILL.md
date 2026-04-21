---
name: pid-ui-validation-worker
description: Verify end-to-end PID confirmation flows, including deterministic regression and attempted UI-level screenshot confirmation.
---

# PID UI Validation Worker

NOTE: Startup and cleanup are handled by `worker-base`. This skill defines the WORK PROCEDURE.

## When to Use This Skill

Use this skill for features that harden the approved sample’s end-to-end validation story: screenshot regression, failure-mode clarity, and the second-layer UI-style confirmation path after deterministic export is already in place.

## Required Skills

- `agent-browser` — use when a real UI-level confirmation path is available and the feature explicitly reaches the point of attempting app-style open-and-shot validation.

## Work Procedure

1. Read the feature, mission files, `.factory/library/user-testing.md`, and the current deterministic screenshot tests before changing anything.
2. Treat deterministic screenshot regression as the required primary validation surface. Make that green first.
3. Follow TDD:
   - add or tighten the failing regression/confirmation test first
   - run the focused failing test
   - implement the minimum change
   - re-run the focused test
4. Ensure regression evidence distinguishes the failure stage:
   - sample open failure
   - sparse/incorrect display failure
   - screenshot export failure
   - baseline/tolerance failure
5. Only after deterministic validation is green, inspect whether a reliable UI-level confirmation surface exists:
   - if yes, invoke `agent-browser` and attempt the smallest real flow that proves open -> shot -> PNG creation
   - if no, return a concrete blocker with exact missing capability/tooling
6. Run required verification commands:
   - `cargo test target_pid_sample_screenshot_matches_baseline -- --nocapture`
   - `cargo test pid_screenshot -- --nocapture`
   - `cargo test pid_import -- --nocapture`
   - `cargo check`
7. Do not claim UI confirmation success without tool evidence. If the second layer is not feasible, the handoff must say so explicitly.

## Example Handoff

```json
{
  "salientSummary": "Finished the approved sample’s deterministic screenshot regression path and attempted the second-layer UI confirmation flow. The deterministic baseline is now explicit about failure mode; UI automation was either completed with evidence or returned as a concrete tooling blocker.",
  "whatWasImplemented": "Updated the screenshot regression surface for the approved PID sample so open/display/export regressions fail at the correct assertion stage, and then attempted a real user-style confirmation path for opening the sample and triggering screenshot export. Deterministic regression remains the mission’s primary proof; UI confirmation is additive and evidence-backed.",
  "whatWasLeftUndone": "UI-level confirmation could not be completed because no reliable app automation harness was available for driving the desktop PID open-and-shot flow in this environment.",
  "verification": {
    "commandsRun": [
      {
        "command": "cargo test target_pid_sample_screenshot_matches_baseline -- --nocapture",
        "exitCode": 0,
        "observation": "Approved sample regression stayed within the expected PNG baseline/tolerance bounds."
      },
      {
        "command": "cargo test pid_screenshot -- --nocapture",
        "exitCode": 0,
        "observation": "Deterministic export helpers remained green across the screenshot test slice."
      },
      {
        "command": "cargo test pid_import -- --nocapture",
        "exitCode": 0,
        "observation": "Target-sample open/display tests remained green after the regression changes."
      },
      {
        "command": "cargo check",
        "exitCode": 0,
        "observation": "Workspace compiled successfully after validation-surface changes."
      }
    ],
    "interactiveChecks": [
      {
        "action": "Assessed whether a reliable app-style automation path existed for opening the approved sample and triggering PIDSHOT.",
        "observed": "No trustworthy desktop harness was available in-repo, so deterministic PNG regression remains the verified primary path and UI confirmation is reported as an environment/tooling blocker."
      }
    ]
  },
  "tests": {
    "added": [
      {
        "file": "src/io/pid_screenshot.rs",
        "cases": [
          {
            "name": "target_pid_sample_screenshot_matches_baseline",
            "verifies": "The approved sample’s deterministic PNG output remains within the allowed baseline/tolerance band."
          }
        ]
      }
    ]
  },
  "discoveredIssues": [
    {
      "severity": "medium",
      "description": "UI-level open-and-shot confirmation still depends on desktop automation capability that may not exist reliably in the current repository/tool environment.",
      "suggestedFix": "If mission policy requires full second-layer confirmation, add or standardize a desktop automation harness before claiming the UI path complete."
    }
  ]
}
```

## When to Return to Orchestrator

- Deterministic screenshot regression is green but UI-level confirmation remains infeasible due to missing automation capability
- Baseline/tolerance design becomes unstable and requires orchestrator judgment on acceptable regression policy
- The approved sample path or screenshot output is no longer stable enough to act as mission truth
