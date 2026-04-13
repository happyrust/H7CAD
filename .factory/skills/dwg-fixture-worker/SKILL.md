---
name: dwg-fixture-worker
description: Build and verify DWG parser fixtures, manifests, and regression coverage for synthetic-first validation.
---

# DWG Fixture Worker

NOTE: Startup and cleanup are handled by `worker-base`. This skill defines the work procedure.

## When to Use This Skill

Use this skill for features that add or refine DWG fixtures, fixture manifests, parser summaries, regression harnesses, or validation-only support code for `crates/h7cad-native-dwg`.

This mission is synthetic-first. Real DWG samples are only allowed when the feature or milestone explicitly calls for them, and their provenance must be captured in the test-side manifest or comments.

## Required Skills

- `test-driven-development` — invoke before editing fixture harnesses or parser-facing regression tests so missing coverage is made explicit first.
- `verification-before-completion` — invoke before handoff so the fixture baseline is backed by passing cargo commands and concrete observations.
- `systematic-debugging` — invoke if a fixture failure could be caused by parser defects, malformed samples, or harness issues and you need to separate them methodically.

## Work Procedure

1. Read `mission.md`, `AGENTS.md`, and `.factory/library/user-testing.md`. Confirm which assertions the fixture feature is meant to unlock and whether the feature allows synthetic-only data or selective real DWG samples.
2. Write failing tests or manifest checks first. Make missing fixtures, bad payload layouts, or absent summaries fail loudly with actionable messages instead of silently skipping coverage.
3. Prefer synthetic fixtures generated in test code or compact binary files under `crates/h7cad-native-dwg/tests/fixtures`. Keep samples minimal and purpose-built for the parser behavior being validated.
4. If a real DWG sample is required:
   - keep it as small as possible,
   - record its source/provenance in the manifest or adjacent test comments,
   - avoid adding samples with unclear licensing or oversized engineering data.
5. Keep fixture work isolated to parser validation surfaces. Do not modify `src/io`, desktop app routing, or unrelated crates unless the feature explicitly requires a shared summary/helper update.
6. Before handoff, run sequential validation commands:
   - `cargo test -p h7cad-native-dwg --test read_headers`
   - `cargo test -p h7cad-native-dwg`
   - `cargo check -p h7cad-native-facade`
   Add any extra targeted cargo command that proves the new fixture coverage specifically exercised the intended path.
7. In the handoff, enumerate every added or changed fixture, what parser behavior it covers, and any remaining fixture gaps that still block later milestones.

## Example Handoff

```json
{
  "salientSummary": "Added a synthetic AC1018 fixture layout and a manifest-backed regression test for section decoding. The new fixture now fails clearly if the byte layout changes and the parser baseline remains green.",
  "whatWasImplemented": "Created a compact synthetic fixture helper for AC1018 section maps in crates/h7cad-native-dwg/tests/read_headers.rs and updated the fixture manifest logic so missing or malformed payload entries produce actionable test failures. The regression suite now covers version sniffing, section offsets, and payload reads for the new sample layout.",
  "whatWasLeftUndone": "",
  "verification": {
    "commandsRun": [
      {
        "command": "cargo test -p h7cad-native-dwg --test read_headers section_payloads_are_read_from_directory_offsets -- --nocapture",
        "exitCode": 0,
        "observation": "Targeted fixture regression passed against the new AC1018 sample."
      },
      {
        "command": "cargo test -p h7cad-native-dwg --test read_headers",
        "exitCode": 0,
        "observation": "All parser integration tests passed with the updated fixture set."
      },
      {
        "command": "cargo test -p h7cad-native-dwg",
        "exitCode": 0,
        "observation": "Full DWG parser crate test suite passed."
      },
      {
        "command": "cargo check -p h7cad-native-facade",
        "exitCode": 0,
        "observation": "Facade compile surface still succeeds after fixture-side helper updates."
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
            "name": "read_header_extracts_ac1018_section_descriptors",
            "verifies": "Synthetic AC1018 fixture bytes map to the expected section directory entries."
          },
          {
            "name": "section_payloads_are_read_from_directory_offsets",
            "verifies": "Fixture payload regions stay aligned with the directory offsets and sizes used by the parser."
          }
        ]
      }
    ]
  },
  "discoveredIssues": [
    {
      "severity": "low",
      "description": "The mission still lacks curated real-world DWG smoke samples for later milestone gates; a follow-up fixture feature should add selective provenance-tracked files once parser coverage broadens."
    }
  ]
}
```

## When to Return to Orchestrator

- A required real DWG sample cannot be sourced or its provenance/licensing is unclear.
- The feature would need parser behavior that has not been designed yet, making the fixture assertions speculative.
- A failing fixture appears to expose a broader parser design issue that belongs in a parser implementation feature instead of validation scaffolding.
- Baseline cargo commands fail due to unrelated workspace issues and you cannot isolate a safe fixture-only fix.
