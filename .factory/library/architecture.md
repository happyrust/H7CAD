# Architecture

Environment-independent high-level architecture for the DWG parser mission.

This document captures the worker-facing structure of the current `h7cad-native-dwg` parser-only mission. It should describe stable pipeline stages, mission boundaries, invariants, and risk concentration areas. It should not be used for patch notes, task journaling, or implementation diffs.

Use this document for high-level intent and invariants. Use `validation-contract.md` for the exact observable requirements that define done.

## Mission Scope

This mission is parser-only and is intentionally bounded to `crates/h7cad-native-dwg`, with compile-surface awareness for `crates/h7cad-native-facade`.

Current implementation reality:
- recognize `AC1015` and `AC1018` from file magic
- parse version-specific header metadata needed to locate section descriptors
- decode section directory entries and extract raw section payload bytes
- convert extracted payloads into a parser-side pending graph
- resolve that pending graph into a seeded `h7cad-native-model::CadDocument`
- preserve facade compile compatibility without switching the application runtime DWG path

Target contract direction during this mission:
- replace zero-delimiter tokenization with decoded semantic record boundaries and explicit semantic-path failures
- replace section-index bucket heuristics with outward-facing semantic identity/provenance derived from decoded DWG meaning
- populate parser-facing layer/entity/block/layout semantics on `PendingDocument` before depending on resolved-model assertions
- strengthen resolver-facing invariants and semantic materialization without implying runtime rollout

Out of scope for this mission:
- changing `src/io` so the desktop/runtime DWG open path defaults to native
- broad bridge/runtime rollout work outside compile-surface compatibility
- full real-DWG object decoding across later DWG versions
- UI behavior, rendering, or desktop smoke as done criteria
- replacing the current placeholder pending/object classification with complete DWG semantic decoding in one step

The practical support boundary is therefore: parser crate progress is allowed, facade compile health matters, and runtime integration remains unchanged.

## Current Parser Pipeline

The current end-to-end flow in `h7cad-native-dwg` is:

1. **Version sniffing / file-header parse**  
   `sniff_version()` and `DwgFileHeader::parse()` identify a known DWG version and read the version-specific section-count/header fields.

2. **Section directory decode**  
   `SectionMap::parse()` turns header-driven directory bytes into ordered `SectionDescriptor` entries with stable `(index, offset, size)` metadata.

3. **Section payload extraction**  
   `SectionMap::read_section_payloads()` slices the file into raw payload buffers exactly as referenced by section descriptors.

4. **Pending graph construction**  
   `build_pending_document()` creates `PendingDocument`, copies section metadata into `PendingSection`, and is the primary seam where structural payload bytes are turned into parser-emitted semantic records. Today this still includes placeholder tokenization/classification; this mission replaces those seams with decoded semantic identity, handle/owner provenance, and typed pending surfaces such as layers/entities.

5. **Dispatch summarization surface**  
   `dispatch_object()` and `summarize_object()` expose a stable outward-facing interpretation of each parser-emitted record via `DispatchTarget` and `ParsedRecordSummary`. During this mission those outward summaries must become semantic/provenance-bearing enough to distinguish same-sized, reordered, and same-kind records without falling back to helper-only labels.

6. **Resolution into native model**  
   `resolve_document()` seeds a fresh `CadDocument`, preserves pending handles/owner handles, materializes parser-derived structures onto outward-facing native-model surfaces, advances handle allocation state, and restores document relationships. This mission hardens that step so parser-emitted provenance survives resolution and impossible semantic relationships fail closed.

This pipeline is deliberately shallow today: it proves parser structure and dataflow before full DWG object semantics are implemented.

## Current Implementation Status

Workers should keep the distinction between **current behavior** and **target contract** explicit:

- file-structure parsing is real for the current baseline versions, but supported-version breadth is intentionally narrow
- pending-record extraction is still placeholder tokenization over payload bytes, not real DWG semantic record decode
- pending-object classification is still a coarse synthetic bucket, not decoded table/entity/object/layout/block meaning
- `PendingLayer` and `PendingEntity` exist as parser-facing surfaces, but current parser flow barely populates them
- resolver output is still mostly native-document scaffolding plus generic projections derived from parser provenance
- many contract assertions are intentionally stronger than today’s evidence because this mission exists to close those semantic/provenance gaps

## Semantic Mission Focus

Workers should treat the following seams as the deliberate implementation focus for this mission:

1. **Semantic record boundaries and fail-closed decode.**  
   Replace delimiter-driven splitting with decoded record boundaries while keeping structural failures distinguishable from semantic failures.

2. **Parser-facing semantic surfaces.**  
   Make `PendingDocument.layers`, `PendingDocument.entities`, and related provenance summaries reflect decoded DWG handles, owners, layout/block identity, and semantic kinds directly from parse results.

3. **Provenance stability.**  
   Ensure parser-emitted records remain matchable across repeated parses, reordered-equivalent fixtures, and later resolved projections without relying on payload size or ordinal-only identity.

4. **Resolver semantic materialization.**  
   Move from generic unknown-object summaries toward outward-facing resolved relationships that preserve parser-emitted handles, owners, layout/block links, layer mappings, and entity placement.

## Canonical Observable Surfaces

For this mission, workers should treat the following as the canonical observable/test-facing surfaces:

- **Parser-facing truth:** `PendingDocument` (`sections`, `objects`, `layers`, `entities`)
- **Outward summary/provenance surface:** `dispatch_object()` and `ParsedRecordSummary`
- **Resolved truth:** the parser-emitted subset of resolved `CadDocument` projections, explicitly distinguished from resolver-seeded scaffold objects

If a change only looks correct through helper-only state or implicit scaffold behavior, it is not yet done.

## Key Data Structures and Flows

### `DwgFileHeader`
The header is the first version-aware checkpoint. It establishes which layout rules apply and how many section descriptors should be read. Everything downstream assumes header parsing chose the correct version-specific offsets.

### `SectionMap` and `SectionDescriptor`
`SectionMap` is the structural boundary between raw bytes and parser-addressable regions. Descriptor order is meaningful and must remain directory order, not offset-sorted order. Payload extraction is expected to be exact and fail closed on bounds violations.

### `PendingDocument`
`PendingDocument` is the parser-side staging graph between structural decode and model resolution. It currently contains:
- `sections`: stable per-section metadata plus raw payload bytes
- `objects`: parser-emitted pending records with synthetic handles, owner state, section provenance, and coarse record kind
- `layers` / `entities`: reserved collections for later typed expansion

### `PendingSection`
Each `PendingSection` mirrors one decoded section descriptor and carries:
- section identity (`index`)
- byte span metadata (`offset`, `size`)
- parser-emitted `record_count`
- raw `payload`

This is the bridge between file-structure validation and pending-graph validation.

### `PendingObject` and `PendingObjectKind`
Each emitted pending object records:
- a deterministic handle/provenance identity (currently often synthetic, but targeted to become decoded DWG handle/owner/semantic identity as this mission progresses)
- visible owner state (`owner_handle`)
- originating `section_index`
- coarse semantic bucket (`TableRecord`, `EntityRecord`, `ObjectRecord`) with `record_index` and `payload_size`

These objects are the current parser’s unit of provenance that flows into both dispatch summaries and final resolution.

### Object reader / dispatch surface
`object_reader` is currently a compile-stable semantic boundary, not yet a full DWG decoder. Its role is to keep outward-facing dispatch and summary behavior stable while the underlying parser evolves from synthetic bucketing toward real typed record readers.

### Resolver and native model
`resolve_document()` is the late-binding boundary. The resolver takes parser-side pending state and produces a valid native `CadDocument`. Important distinction: much of the base scaffold comes from `CadDocument::new()`, while the resolver’s DWG-specific job is to preserve pending handles/owners, insert pending-derived layers/objects, keep object order/provenance stable, and then rely on ownership repair to restore document relationships. Parser-emitted resolved records must stay outwardly distinguishable from resolver-seeded scaffold records.

## Invariants To Preserve

Workers should treat the following as mission-level invariants:

- **Known parser phases remain explicit.** Invalid magic, truncated headers, known-but-unsupported baseline-adjacent versions, structural decode failure, pending-graph construction, and resolution should stay distinguishable at the observable API level.
- **Failure boundaries stay explicit.** Malformed/truncated structure must fail as structural error with no semantic output; structurally valid but semantically undecodable records must fail as semantic decode errors with no placeholder downgrade.
- **Version-specific header rules are authoritative.** AC1015 and AC1018 currently define the supported baseline and must use their own offsets/boundaries.
- **Section descriptor order is stable.** Directory order drives emitted section order, payload order, pending section order, and downstream record provenance.
- **Payload extraction is exact and fail-closed.** Out-of-bounds or truncated spans must error rather than returning partial results.
- **Pending graph accounting must stay synchronized.** `PendingSection.record_count`, per-section emitted objects, and aggregate `PendingDocument.objects.len()` must agree.
- **Pending provenance remains externally visible.** Section index, record index, payload size, dispatch target, handle, and owner state must stay assertable from public/test-facing surfaces.
- **Handle behavior is deterministic.** Identical inputs must yield the same pending handle/order and stable resolved object summaries.
- **Resolution preserves ownership/handle intent.** Resolver work must not silently drop supplied owner handles or regress seeded native-document scaffold relationships.
- **Facade compatibility is compile-surface only for this mission.** Parser evolution may not break `h7cad-native-facade` compilation, but should not assume runtime activation.

## Current Gaps / Risk Concentration

Current complexity is concentrated in the places where the parser is intentionally still synthetic or incomplete:

1. **Record extraction is placeholder-driven.**  
   `classify_section_records()` currently tokenizes payloads by zero delimiters instead of decoding semantic record boundaries.

2. **Record typing is bucketed, not decoded.**  
   `classify_record_kind()` assigns table/entity/object kind from section index modulo arithmetic, which is only a temporary scaffold for pipeline validation.

3. **Pending graph is broader than current semantic population.**  
   `PendingLayer` and `PendingEntity` exist, but most parser output still lands in generic `PendingObject` form and block/layout identity is still too implicit.

4. **Resolver currently preserves structure more than semantics.**  
   Parser-derived objects resolve to generic summaries more often than to outward-facing semantic native-model relationships.

5. **Version support is intentionally narrow.**  
   The mission baseline is AC1015 and AC1018. Later versions and richer decode paths should not be implied by architecture text.

6. **Facade relation is easy to over-read.**  
   The parser must not drift into runtime-integration assumptions; the facade is only part of the validation surface for compile compatibility.

These are the main areas where workers should expect implementation churn while still preserving the outward architecture contract.

## Validation Surface Mapping

### Fixtures
Synthetic fixtures are the primary validation surface for this mission. They provide deterministic coverage for:
- version sniffing
- version-specific header boundaries
- section directory ordering and bounds
- payload extraction exactness
- pending-graph record accounting
- resolver determinism

Selective real DWG fixtures are only milestone-gate supplements, not the default development surface.

### Pending graph
The pending graph is the core parser-facing regression seam. It links structural file decode to resolver behavior and exposes the mission’s most important intermediate artifacts:
- section metadata
- emitted object counts
- stable per-record provenance
- visible owner/handle state

### Resolver
The resolver validates that parser-side state can be turned into a stable `CadDocument` without losing handles, ownership intent, or seeded document scaffolding. It is the architectural handoff from parser internals to native-model observability.

### Facade compile surface
`h7cad-native-facade` is part of the validation map only as a compile-surface consumer of the DWG parser crate. For this mission, it confirms API compatibility and linkage stability; it is not evidence that the application runtime DWG path has switched to the native parser.

### Relationship summary
The architecture relationship for workers is:

`fixtures -> header/section decode -> pending graph -> dispatch summaries -> resolver -> native model`  
with `facade compile checks` observing the public surface from outside the parser crate.
