---
name: dxf-regression-worker
description: Harden the DXF writer and add sample-backed roundtrip/integration regression coverage.
---

# DXF Regression Worker

NOTE: Startup and cleanup are handled by worker-base. This skill defines the work procedure for writer and regression features.

## When to Use This Skill

Use this skill for features that modify:
- `crates/h7cad-native-dxf/src/writer.rs`
- `crates/h7cad-native-dxf/src/lib.rs`
- regression-focused native tests in the DXF/model/facade crates

Typical work:
- fixing writer data-loss bugs
- adding roundtrip tests
- building real-sample pipeline regressions
- validating facade DXF load/save smoke behavior

## Required Skills

- `tdd` — invoke before changing code; write failing regression tests first.
- `verification-before-completion` — invoke before ending the session.
- `systematic-debugging` — invoke if sample-backed or roundtrip tests fail unexpectedly.

## Work Procedure

1. Read `mission.md`, `validation-contract.md`, `AGENTS.md`, `.factory/library/architecture.md`, `.factory/library/user-testing.md`, and `.factory/services.yaml`.
2. Map the feature’s assertion IDs to concrete tests and sample inputs.
3. Add failing tests first in the smallest sensible place:
   - unit-style roundtrip tests for isolated writer bugs
   - real-sample regressions for pipeline assertions
4. Confirm the new tests fail for the intended reason before touching implementation.
5. Implement the minimal writer/test-support changes needed.
6. Re-run the tightest targeted commands until green.
7. Run broader regression commands:
   - `test_dxf`
   - `test`
   - `check_facade` or facade tests when the feature touches the DXF facade surface
8. For real-sample tests, assert semantic fields after reread; do not rely only on raw string matching or total entity counts.
9. If a full-pipeline assertion is in scope, verify referenced handles, ownership, or block-content observability explicitly.
10. Invoke `verification-before-completion`, then hand off with concrete evidence.

## Example Handoff

```json
{
  "salientSummary": "Fixed hatch polyline and classic polyline writer fidelity issues, then added roundtrip regressions proving bulges, widths, and mixed-document content survive write/read cycles.",
  "whatWasImplemented": "Added failing DXF roundtrip tests for hatch polyline paths and classic polyline widths in crates/h7cad-native-dxf/src/lib.rs, patched writer.rs to emit the missing data, and extended mixed-document regression coverage to assert per-entity semantic fields after reread.",
  "whatWasLeftUndone": "",
  "verification": {
    "commandsRun": [
      {
        "command": "cargo test -p h7cad-native-dxf -- --test-threads=16",
        "exitCode": 0,
        "observation": "DXF writer and roundtrip regressions all passed, including new hatch/polyline tests."
      },
      {
        "command": "cargo test -p h7cad-native-dxf -p h7cad-native-model -p h7cad-native-facade -- --test-threads=16",
        "exitCode": 0,
        "observation": "Native crate regression surface remained green after the writer fixes."
      }
    ],
    "interactiveChecks": [
      {
        "action": "Roundtrip real and synthetic DXF documents through write_dxf + reread in tests",
        "observed": "Known-risk fields (bulges, widths, ACIS payloads, attrib relationships) remained intact."
      }
    ]
  },
  "tests": {
    "added": [
      {
        "file": "crates/h7cad-native-dxf/src/lib.rs",
        "cases": [
          {
            "name": "roundtrip_hatch_polyline_boundary_preserves_bulges",
            "verifies": "VAL-WRITE-001"
          },
          {
            "name": "roundtrip_polyline_preserves_vertex_widths",
            "verifies": "VAL-WRITE-002"
          }
        ]
      }
    ]
  },
  "discoveredIssues": []
}
```

## When to Return to Orchestrator

- The required regression needs a GUI/runtime validation path instead of cargo-only evidence.
- The fix would require broad parser redesign outside the writer/regression scope.
- Real-sample expectations appear to conflict with current mission boundaries or with the accepted DXF support level.
