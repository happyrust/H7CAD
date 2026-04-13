---
name: dwg-parser-worker
description: Implement parser-side DWG milestones inside h7cad-native-dwg with strict test-first verification.
---

# DWG Parser Worker

NOTE: Startup and cleanup are handled by `worker-base`. This skill defines the work procedure.

## When to Use This Skill

Use this skill for features that implement or refactor DWG parsing logic in `crates/h7cad-native-dwg`, including header decoding, section decoding, record classification, pending graph extraction, resolver behavior, and parser-facing compile-surface checks in `crates/h7cad-native-facade`.

This mission is parser-only. Do not modify `src/io`, do not switch the desktop app's default DWG loading path, and do not introduce new services, ports, or credentials.

## Required Skills

- `test-driven-development` — invoke before changing tests or parser code so the feature starts from failing assertions and then turns green.
- `verification-before-completion` — invoke before final handoff to confirm the required cargo commands were actually run and passed.
- `systematic-debugging` — invoke if the existing parser baseline or compile-surface checks fail unexpectedly and the root cause is not immediately obvious.

## Work Procedure

1. Read `mission.md`, `AGENTS.md`, `.factory/library/architecture.md` if present, and `.factory/library/user-testing.md`. Restate the feature scope, constraints, and `fulfills` assertion IDs in your notes before editing.
2. Add or update failing tests first. Prefer crate-local unit tests or `crates/h7cad-native-dwg/tests/*.rs`. When a feature changes parser output, add assertions against `CadDocument`, pending structures, or record summaries before implementation.
3. Implement the smallest parser-only change that makes the new tests pass. Keep edits focused on `crates/h7cad-native-dwg` unless the feature explicitly calls for a compile-surface adjustment in `crates/h7cad-native-facade`.
4. Preserve DWG mission boundaries:
   - No edits to `src/io` or desktop DWG routing.
   - No new runtime services, background processes, or ports.
   - Prefer synthetic fixtures and inline byte layouts unless the feature explicitly requires a real DWG sample.
5. Re-run targeted tests during iteration until the new behavior is stable. Before handoff, run these sequential baseline commands unless the feature description requires a wider scope:
   - `cargo check -p h7cad-native-dwg`
   - `cargo test -p h7cad-native-dwg`
   - `cargo check -p h7cad-native-facade`
6. Manually inspect parser invariants through assertions or debug-oriented test expectations: version detection, section offsets/sizes, record counts, handle stability, owner relationships, and block/layout presence when relevant to the feature.
7. Prepare a concrete handoff. List exact files changed, tests added, commands run with observations, and any unresolved parser gaps or format ambiguities. If anything is incomplete, say exactly what remains and why.

## Example Handoff

```json
{
  "salientSummary": "Implemented AC1015 section payload extraction and pending-object classification for the targeted parser milestone. Added failing tests first, then wired the parser path until both the new unit coverage and crate baseline passed.",
  "whatWasImplemented": "Updated crates/h7cad-native-dwg/src/section_map.rs and src/lib.rs so AC1015 directory entries now produce bounded payload slices, and the pending-document builder converts each decoded section into typed PendingObject records with stable synthetic handles. Added regression coverage in crates/h7cad-native-dwg/tests/read_headers.rs for payload extraction and section-to-record mapping.",
  "whatWasLeftUndone": "",
  "verification": {
    "commandsRun": [
      {
        "command": "cargo test -p h7cad-native-dwg read_header_extracts_ac1015_section_count -- --nocapture",
        "exitCode": 0,
        "observation": "Targeted red/green coverage passed after the parser change."
      },
      {
        "command": "cargo check -p h7cad-native-dwg",
        "exitCode": 0,
        "observation": "Parser crate compiled cleanly with no new warnings."
      },
      {
        "command": "cargo test -p h7cad-native-dwg",
        "exitCode": 0,
        "observation": "All DWG parser unit and integration tests passed."
      },
      {
        "command": "cargo check -p h7cad-native-facade",
        "exitCode": 0,
        "observation": "Facade compile surface still builds against the updated parser crate."
      }
    ],
    "interactiveChecks": []
  },
  "tests": {
    "added": [
      {
        "file": "crates/h7cad-native-dwg/tests/read_headers.rs",
        "cases": [
          {
            "name": "section_payloads_are_read_from_directory_offsets",
            "verifies": "AC1015 section directory offsets resolve to the expected payload bytes."
          },
          {
            "name": "pending_document_preserves_section_directory_entries",
            "verifies": "PendingDocument keeps section metadata and typed object counts aligned with decoded payloads."
          }
        ]
      }
    ]
  },
  "discoveredIssues": [
    {
      "severity": "medium",
      "description": "Record classification still uses synthetic section buckets rather than real DWG object metadata; a later feature must replace this heuristic with format-aware decoding."
    }
  ]
}
```

## When to Return to Orchestrator

- The feature requires changing `src/io`, desktop DWG routing, or any mission boundary that is currently off-limits.
- A required DWG format rule is ambiguous enough that multiple incompatible parser designs seem valid.
- The work depends on a missing fixture, missing object metadata source, or unresolved upstream model change not covered by the current feature.
- Baseline cargo commands fail for reasons unrelated to your feature and `systematic-debugging` does not produce a clear, local fix.
