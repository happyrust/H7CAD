# Architecture

High-level architecture for the `PLAN-4-18` DWG mission.

This document is worker-facing guidance for how the AC1015 parser lane currently works, where this mission is allowed to change it, and which seams must stay untouched. It is intentionally high-level. Use `validation-contract.md` for the exact observable definition of done.

## Mission Scope

This mission is intentionally bounded to the parser side of the native DWG lane:

- `crates/h7cad-native-dwg`
- `crates/h7cad-native-model`
- `crates/h7cad-native-facade` only as a compile/fail-closed boundary
- `CHANGELOG.md` for phase/status wording

Out of scope:

- runtime DWG rollout in `src/io/mod.rs`
- switching desktop/app DWG load/save to native
- DWG writer work
- AC1018+ parser expansion
- modifying `acadrust`
- GUI/browser/TUI validation

## Current System Shape

There are two DWG tracks in the repository today:

1. **Native parser track**  
   `crates/h7cad-native-dwg::read_dwg(bytes)` reads bytes, builds a pending AC1015 document, resolves document structure, and then best-effort enriches real entities into `h7cad_native_model::CadDocument`.

2. **Runtime compatibility track**  
   `src/io/mod.rs` contains broader native-first helper wrappers from ongoing repo migration work, but the `.dwg` branches still route open/save through `acadrust::io::dwg::DwgReader` and `DwgWriter`. For this mission, that compat-backed `.dwg` behavior is the invariant that must remain unchanged.

The mission therefore improves native parser truth without changing product runtime truth.

## Canonical DWG Flow

### Native parser flow
`DWG bytes -> sniff_version -> DwgFileHeader::parse -> SectionMap::parse -> build_pending_document -> resolve_document -> enrich_with_real_entities -> native CadDocument`

This is the flow the mission is allowed to deepen.

### Structural AC1015 observation flow
`sample_AC1015.dwg -> section locator / handle map / object stream cursor / object header diagnostics`

This flow is not just debugging output. It is the ground truth that explains why recovery counts move. Today the visible structural diagnostics already expose `slice_miss`, `header_fail`, and `handle_mismatch`; this mission extends that into named supported-family recovery diagnostics instead of silent skips.

### Native model consumption flow
`native CadDocument -> entity/block/layout/owner queries -> resolve_insert_block(entity)`

This is the downstream contract for recovered entities. Recovery is not complete unless entities survive into this resolved document surface with usable metadata and relationships.

### Runtime boundary flow
`src/io/mod.rs -> compat DwgReader/DwgWriter`

This path is intentionally unchanged. The native facade must stay fail-closed for DWG so parser progress cannot be mistaken for runtime availability.

## Mission Focus Areas

### 1. Recovery closure for already-supported AC1015 entities
Current native recovery already handles these families on the mission-start baseline:

- `TEXT`
- `ARC`
- `CIRCLE`
- `LINE`
- `POINT`
- `LWPOLYLINE`
- `HATCH`

The mission is not mainly about adding more unrelated types. It is about:

- raising recovery floors on the AC1015 real sample
- making regression floors explicit in tests
- surfacing why supported objects fail to recover
- preserving non-default common metadata on recovered entities

`INSERT` is the new family entering this mission; it is not part of the pre-mission recovered-family baseline above.

### 2. Recovery diagnostics
The highest-risk current behavior is silent loss inside enrichment. The mission needs a named diagnostics surface that explains:

- handle-map misses
- object-header decode failures
- supported-family decode failures
- unsupported-type skips

Diagnostics are part of the product of this mission, not just temporary debugging.

### 3. INSERT entry into the AC1015 read-path
`h7cad-native-model` already has `EntityData::Insert`, and DXF/bridge layers already know how to carry it. The missing link is the DWG parser lane:

- object type dispatch
- INSERT body decode
- common metadata attachment
- block resolution in the resolved native document

This mission treats `INSERT` as the first new entity family because the model surface already exists and the value is high.

### 4. Boundary preservation
Even while the parser grows, these truths must remain stable:

- `h7cad-native-facade` DWG load/save stays unavailable
- `src/io/mod.rs` keeps `.dwg` behavior on the compat DWG runtime
- no new services, ports, or credentials appear
- no GUI/runtime rollout is implied

## Canonical Observable Surfaces

Workers should treat these as the authoritative observable surfaces:

- **Parser truth:** `crates/h7cad-native-dwg::read_dwg`
- **Structural truth:** `crates/h7cad-native-dwg/tests/real_samples.rs` for both recovery-baseline assertions and handle-map/object-header diagnostics
- **Resolved document truth:** `h7cad_native_model::CadDocument`
- **Boundary truth:** `crates/h7cad-native-facade` plus `src/io/mod.rs`
- **Phase/status truth:** `CHANGELOG.md`

If a change only exists inside a helper function but cannot be observed through tests, cargo output, or boundary inspection, it is not complete.

## Invariants To Preserve

- **AC1015 remains the only active version target for this mission.**
- **Supported-family recovery must never regress silently.**
- **Recovery gains must be explainable by named diagnostics, not just higher counts.**
- **Recovered entities must carry usable common metadata, not only geometry payloads.**
- **INSERT is not complete until it resolves to block records on the native model surface.**
- **Facade/runtime DWG boundaries remain fail-closed and compat-based.**
- **The mission must remain cargo-only and parser-only.**

## Risk Concentration

### `crates/h7cad-native-dwg/src/lib.rs`
This is the highest-risk seam because it owns:

- the `read_dwg()` orchestration path
- supported-type dispatch
- enrichment
- silent loss behavior during best-effort recovery

### `crates/h7cad-native-dwg/tests/real_samples.rs`
This is the main regression truth surface. If counts rise or fall here, the mission has materially changed behavior.

### `crates/h7cad-native-model/src/lib.rs`
This is where recovered entities become usable document state. `INSERT` must be validated here through `resolve_insert_block(...)` and ownership/block relationships.

### `crates/h7cad-native-facade/src/lib.rs` and `src/io/mod.rs`
These two files define the user-visible DWG boundary. They are not implementation targets for rollout in this mission, but they are mandatory validation boundaries.

## Validation Surface Mapping

### Real sample regression
Use `sample_AC1015.dwg` and `tests/real_samples.rs` for:

- total and per-family recovery floors
- metadata checks
- structural diagnostics
- AC1015-specific regressions

### Focused parser tests
Use crate-local tests for:

- failure classification
- object/body decode rules
- INSERT payload decoding
- resolver/document invariants

### Boundary checks
Use cargo tests and source inspection for:

- facade fail-closed behavior
- compat runtime DWG routing
- changelog wording consistency

## Relationship Summary

The architecture relationship for workers is:

`AC1015 DWG bytes -> native parser pipeline -> resolved native document -> recovery diagnostics and entity assertions`

while

`facade DWG APIs` and `src/io/mod.rs` continue to assert that runtime rollout has not happened yet.
