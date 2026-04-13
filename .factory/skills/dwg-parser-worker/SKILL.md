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
2. Check whether the feature is already materially present in the shared worktree. If so, take a verification-first path: prove the current implementation satisfies the feature’s `fulfills` assertions, then make only the smallest follow-up changes still needed for clarity, coverage, or auditability. Do not manufacture churn just to create edits. Verification-first success still requires direct targeted assertions or named tests for the claimed outward-facing behavior; broad category filters alone are not enough if they do not actually exercise the feature contract.
3. If the feature is not already present, add or update failing tests first. Prefer crate-local unit tests or `crates/h7cad-native-dwg/tests/*.rs`. When a feature changes parser output, add assertions against `CadDocument`, pending structures, or record summaries before implementation.
4. Implement the smallest parser-only change that makes the new tests pass. Keep edits focused on `crates/h7cad-native-dwg` unless the feature explicitly calls for a compile-surface adjustment in `crates/h7cad-native-facade`.
5. Preserve DWG mission boundaries:
   - No edits to `src/io` or desktop DWG routing.
   - No new runtime services, background processes, or ports.
   - Prefer synthetic fixtures and inline byte layouts unless the feature explicitly requires a real DWG sample.
6. Re-run targeted tests during iteration until the new behavior is stable. Before handoff, run these sequential baseline commands unless the feature description requires a wider scope:
   - `cargo check -p h7cad-native-dwg`
   - `cargo test -p h7cad-native-dwg`
   - `cargo check -p h7cad-native-facade`
7. Manually inspect parser invariants through assertions or debug-oriented test expectations: version detection, section offsets/sizes, record counts, handle stability, owner relationships, and block/layout presence when relevant to the feature.
8. In the shared dirty workspace, create an isolated feature commit by staging only the files touched or intentionally audited for this feature. Never reuse an unrelated commit ID. If the verification-first path proves the feature is already present and the relevant files have no remaining diff, cite the latest truthful existing commit touching those feature files (for example via `git log -1 -- <paths>`) instead of manufacturing churn just to create a new commit. Return to the orchestrator only if you cannot identify truthful existing commit provenance for the already-present implementation, or if relevant and unrelated edits are interleaved in the same file.
9. Prepare a concrete handoff. List exact files changed, whether the work was new implementation or verification-first audit of existing in-worktree behavior, tests added, commands run with observations, and any unresolved parser gaps or format ambiguities. If anything is incomplete, say exactly what remains and why.

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
