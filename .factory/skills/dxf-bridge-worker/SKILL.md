---
name: dxf-bridge-worker
description: Implement bidirectional DXF native/compat bridge features with strict test-first coverage.
---

# DXF Bridge Worker

NOTE: Startup and cleanup are handled by worker-base. This skill defines the work procedure for bridge-focused features.

## When to Use This Skill

Use this skill for features that modify:
- `src/io/native_bridge.rs`
- `src/io/mod.rs` when needed for bridge integration
- `crates/h7cad-native-model/src/lib.rs` only when bridge support needs small native-model additions or re-exports

Typical work:
- adding native↔compat entity mappings
- preserving shared fields across bridge conversions
- preventing document-level silent drops for prioritized entities
- covering block-owned or referenced-handle bridge behaviors

## Required Skills

- `tdd` — invoke before changing code; add failing bridge tests first.
- `verification-before-completion` — invoke before ending the session.
- `systematic-debugging` — invoke if new bridge tests or compile checks fail unexpectedly.

## Work Procedure

1. Read `mission.md`, `validation-contract.md`, `AGENTS.md`, `.factory/library/architecture.md`, `.factory/library/environment.md`, and `.factory/services.yaml`.
2. Identify the exact assertion IDs this feature fulfills and the exact bridge surfaces they touch.
3. Inspect existing bridge tests in `src/io/native_bridge.rs` and any related native-model tests.
4. Add or update failing tests first. Prefer:
   - one test per assertion or per tightly related entity family
   - explicit field assertions, not just entity counts
   - document-level count/type assertions when the feature is about silent drops
5. Run only the most relevant targeted tests to confirm they fail for the intended reason.
6. Implement the minimal bridge changes needed. Keep changes scoped; do not refactor unrelated compat-heavy runtime modules.
7. Re-run targeted tests until green.
8. Run broader validation from `.factory/services.yaml`:
   - `check_native`
   - `test` if the feature only touches native crates / bridge
   - `check` if the feature touches wider runtime compile surfaces
   - use `cargo test --bin H7CAD native_bridge -- --test-threads=16` for the bridge-focused target surface when the manifest shortcut is insufficient or being verified
9. If the feature involves block-owned content or referenced handles, include at least one document-level regression check in addition to unit-style entity checks.
10. Invoke `verification-before-completion`, confirm no required evidence is missing, then hand off.

Before trusting a narrow manifest command, verify it actually matches the current workspace layout. If a manifest command is stale, use the working equivalent and note the deviation in the handoff.
If the assigned target files were already dirty before you started, you should still create a feature commit by staging the assigned file set only. Note any mixed file ancestry in the handoff rather than returning solely because the files were pre-modified.

## Example Handoff

```json
{
  "salientSummary": "Completed bidirectional bridge support for ellipse plus direct-map geometry entities and added document-level count regression coverage so prioritized entities are no longer silently dropped during native->compat conversion.",
  "whatWasImplemented": "Added failing bridge tests for ELLIPSE, RAY, XLINE, and Face3D, implemented native_doc_to_acadrust/acadrust_entity_to_native mappings in src/io/native_bridge.rs, and added a document-level bridge regression that asserts prioritized entity type counts survive conversion.",
  "whatWasLeftUndone": "",
  "verification": {
    "commandsRun": [
      {
        "command": "cargo test --bin H7CAD native_bridge -- --test-threads=16",
        "exitCode": 0,
        "observation": "New bridge tests passed, including per-field equality and document-level count assertions."
      },
      {
        "command": "cargo check -p h7cad-native-dxf -p h7cad-native-model -p h7cad-native-facade",
        "exitCode": 0,
        "observation": "Bridge changes compile cleanly across the native DXF surface."
      }
    ],
    "interactiveChecks": [
      {
        "action": "Construct native document with prioritized bridge entities and convert native -> compat -> native in tests",
        "observed": "Entity types and shared fields remained present; no silent drops in the covered family."
      }
    ]
  },
  "tests": {
    "added": [
      {
        "file": "src/io/native_bridge.rs",
        "cases": [
          {
            "name": "bridge_ellipse_roundtrip_preserves_geometry",
            "verifies": "VAL-BRIDGE-001"
          },
          {
            "name": "document_bridge_preserves_prioritized_entity_counts",
            "verifies": "VAL-BRIDGE-008"
          }
        ]
      }
    ]
  },
  "discoveredIssues": []
}
```

## When to Return to Orchestrator

- The feature requires adding or changing deep runtime systems outside the bridge boundary (`src/scene`, `src/entities`, large `src/app` dispatch code).
- The only viable solution appears to require modifying `acadrust`.
- Existing block/layout/reference behavior is too ambiguous to encode safely without a broader mission change.
