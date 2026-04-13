# Architecture

High-level architecture for the DXF native migration mission.

This document captures the worker-facing structure of the current DXF mission. It describes stable data flows, mission boundaries, invariants, and risk concentration areas. It should not be used for patch notes or implementation journaling.

Use this document for high-level system intent. Use `validation-contract.md` for the exact observable behaviors that define done.

## Mission Scope

This mission is intentionally bounded to the DXF-native pipeline and its runtime bridge surfaces:

- `crates/h7cad-native-dxf`
- `crates/h7cad-native-model`
- `crates/h7cad-native-facade`
- `src/io/mod.rs`
- `src/io/native_bridge.rs`
- quick-win runtime files under `src/` that only need Handle/Color/LineWeight migration

Out of scope:

- changing the DWG parser or DWG runtime rollout
- modifying the `acadrust` dependency directly
- GUI automation or desktop smoke testing
- large refactors of deep compat-coupled systems in `src/app`, `src/scene`, and `src/entities`

## Current Runtime Shape

The runtime is mixed-mode today:

1. **DXF native read**  
   `src/io/mod.rs::load_dxf_native()` reads bytes and calls `h7cad_native_dxf::read_dxf_bytes()`, producing `h7cad_native_model::CadDocument`.

2. **Compat bridge for runtime**  
   Runtime/UI code still expects `acadrust::CadDocument`, so `native_bridge::native_doc_to_acadrust()` converts the native document into compat entities.

3. **Runtime mutation paths**  
   Most editing, scene, command, and UI code still works on compat types. Some paths synchronize compat changes back into native documents through `acadrust_doc_to_native()` or entity-level bridge helpers.

4. **DXF native save**  
   Native documents are written through `h7cad_native_dxf::write_dxf()`. The save path may start from an already-native document or from compat data first bridged into native.

The mission therefore improves the system by making the native read/write path and the compat bridge boundary trustworthy enough for M4 preparation, without replacing all compat runtime code.

## Canonical Flows

### Native DXF read flow
`bytes -> h7cad_native_dxf::read_dxf_bytes() -> native CadDocument`

This flow must preserve:
- DXF header/version metadata
- entities, layers, tables, blocks, and objects needed by supported DXF surfaces
- ownership and handle relationships needed by later bridge/write stages

### Runtime bridge flow
`native CadDocument -> native_bridge::native_doc_to_acadrust() -> compat CadDocument`

This is the key seam where silent loss currently occurs. Mission work here must:
- bridge prioritized native entities into compat equivalents
- preserve common fields and payload fields
- keep supported entities enumerable and usable by runtime surfaces
- preserve supported block-owned content and referenced-handle relationships

### Reverse bridge flow
`compat CadDocument / EntityType -> native_bridge::acadrust_doc_to_native() / acadrust_entity_to_native() -> native CadDocument`

This flow must preserve:
- entity family/type
- geometry and annotation payloads
- shared fields
- enough ownership intent for save and post-process logic to remain valid

### Native DXF write flow
`native CadDocument -> h7cad_native_dxf::write_dxf() -> DXF text`

This flow must emit DXF that rereads cleanly and does not lose:
- hatch boundary structure
- classic polyline widths and flags
- insert/attrib handle relationships
- ACIS-backed payloads

## Mission Focus Areas

### 1. Bridge completion
Primary risk is `src/io/native_bridge.rs`, where unsupported native entities currently fall through to `None` and disappear from compat runtime documents.

The mission completes bidirectional support for:
- ellipse and direct-map geometry entities
- polyline families
- hatch
- leader-like annotation entities
- image/wipeout
- payload-bearing direct-map entities including ACIS-backed types

Contract-critical bridge entities for this mission are:
- `ELLIPSE`
- classic polyline families (`POLYLINE`, `POLYLINE2D`, `POLYLINE3D`, `PolygonMesh`, `PolyfaceMesh`)
- `HATCH`
- `LEADER`
- `MLINE`
- `TOLERANCE`
- `IMAGE`
- `WIPEOUT`
- direct-map geometry entities (`Face3D`, `Solid`, `Ray`, `XLine`, `Shape`)
- text/opaque direct-map entities (`ATTDEF`, standalone `ATTRIB`, `PdfUnderlay`, `Unknown`)
- ACIS-backed entities (`Solid3D`, `Region`)

### 2. Writer hardening
Primary risk is field-level data loss in `crates/h7cad-native-dxf/src/writer.rs`.

The mission focuses on known high-risk cases:
- hatch polyline boundaries
- classic polyline vertex widths
- insert/attrib/seqend handle integrity
- 3DSOLID and REGION ACIS payload fidelity

### 3. Integration regression
Primary risk is that individually-correct pieces still lose data in the full pipeline:

`read_dxf_bytes -> native_bridge::native_doc_to_acadrust -> native_bridge::acadrust_doc_to_native -> write_dxf -> read_dxf`

This mission uses real ACadSharp DXF samples to validate:
- supported-entity preservation
- ownership/model-space/layout validity
- referenced-handle resolvability
- block-owned content observability

### 4. Dependency reduction
Primary risk is widening scope too early. This mission only targets quick-win files that are mostly Handle/Color/LineWeight consumers.

Concrete examples of quick-win targets:
- `src/modules/home/groups/*`
- `src/modules/home/clipboard/paste.rs`
- `src/modules/home/select.rs`
- other similar files whose main compat dependency is `Handle`, `Color`, or `LineWeight`

The mission does **not** rewrite:
- `src/entities/*`
- `src/scene/*`
- large `src/app/*` compat dispatch code

Instead it reduces low-risk dependency surface while keeping the mixed compat/native runtime compiling.

## Canonical Observable Surfaces

Workers should treat the following as test-facing truths:

- **Native document truth:** `h7cad_native_model::CadDocument`
- **Compat bridge truth:** `native_doc_to_acadrust()` / `acadrust_doc_to_native()`
- **Writer truth:** `write_dxf()` followed by reread through `read_dxf()` / `read_dxf_bytes()`
- **Public API truth:** `h7cad-native-facade` DXF load/save functions
- **Runtime compile truth:** targeted `src/` quick-win files plus dependent command/app compile surfaces

If a change only looks correct inside helper functions but cannot be asserted through cargo tests or cargo checks, it is not done.

## Invariants To Preserve

- **No silent entity drops for prioritized bridge families.**
- **Shared entity fields remain stable across bridge round-trips.**
- **Supported block-owned content remains observable where the contract requires it.**
- **Block-owned content is most likely to disappear at document-level bridge boundaries, so workers must validate it explicitly when a feature fulfills those assertions.**
- **Referenced handles remain resolvable, not merely unique or monotonic.**
- **Writer output remains rereadable with field fidelity on known-risk entities.**
- **Facade DXF load/save remains healthy without GUI activation.**
- **Quick-win runtime migration must not expand into deep compat refactors.**

## Risk Concentration

### `src/io/native_bridge.rs`
This is the highest-risk seam because:
- it currently handles only a subset of native entity families
- document-level bridging can silently omit entities
- runtime correctness depends on compat-side observability after native load

### `crates/h7cad-native-dxf/src/writer.rs`
This is the main write-path risk because small omissions create structurally valid DXF with silently degraded semantics.

### `src/io/mod.rs`
This is the integration boundary where native read/write and compat runtime expectations meet.

### Quick-win runtime files under `src/`
These are low-risk migration targets only when they stay in the Handle/Color/LineWeight lane. If a task discovers deeper entity-model coupling, it should return to the orchestrator.

## Validation Surface Mapping

### Synthetic tests
Use synthetic docs/tests to isolate:
- specific bridge mappings
- common-field preservation
- writer bug regressions
- handle and owner invariants

### Real ACadSharp samples
Use sample DXF files for milestone-gate regression of:
- real header/version handling
- end-to-end pipeline preservation
- ownership/block/layout behavior on realistic content

### Compile surfaces
Use `cargo check` and `cargo test` to validate quick-win runtime migration without launching the app.

## Relationship Summary

The architecture relationship for workers is:

`DXF bytes -> native document -> compat bridge -> native bridge-back -> DXF writer -> native reread`

with `facade DXF APIs` and `runtime compile surfaces` observing that pipeline from outside the native crates.
