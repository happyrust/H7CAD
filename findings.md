# Findings

## DXF Integration Snapshot

- H7CAD opens DXF through `h7cad_native_dxf::read_dxf_bytes`, producing `h7cad_native_model::CadDocument`.
- The runtime stores both native data (`Scene.native_store`) and an acadrust compatibility projection (`Scene.document`).
- Save prefers `NativeStore::save` and falls back to converting compat to native only when no native store exists.
- Native model-space render is limited to supported native entities; unsupported entities remain on compat/fallback paths.
- DXF open currently returns no `OpenNotice` diagnostics even when native DXF preserves unknown/proxy content.

## First Implementation Target

Add a DXF-specific diagnostic helper that inspects the parsed native document for `Unknown`, `ProxyEntity`, and unknown object records, then surfaces warning notices through the existing open-notice UI path.

## Implemented

- `src/io/diagnostics.rs` now has `from_native_dxf_document`.
- `src/io/mod.rs` now calls that helper for `.dxf` opens.
- Added unit tests for unknown/proxy entity and object notices.
- `src/scene/acad_to_truck.rs` now converts native `Ellipse` and `Spline`.
- `src/scene/mod.rs` now includes native `Ellipse` and `Spline` in native render-supported entity types.
- Existing `crates/h7cad-native-dxf/tests/entity_2d_roundtrip.rs` already covers `roundtrip_ellipse` and `roundtrip_spline`; the full file passes.
- `src/io/mod.rs` now has an integration-style unit test proving `load_file_native_blocking` surfaces unknown DXF entity notices from a real `.dxf` file.
- `src/scene/mod.rs` now has `NativeRenderStats` and `Scene::native_render_stats` for test/debug observability of native-rendered, compat-fallback, and preserved-only model-space entities.
- `src/entities/ellipse.rs` now has normal-aware ellipse tessellation and quadrant snap points; native ellipse conversion passes `entity.extrusion`.
- `src/scene/tessellate.rs` native dimension geometry now uses native dimstyle `dimasz * dimscale`, `dimexo * dimscale`, and `dimexe * dimscale`.
- `h7cad-native-model::DimStyleProperties` now includes `dimexe`; `h7cad-native-dxf` maps DIMSTYLE table `44=dimexe` and `147=dimgap`.
- Native INSERT rendering now propagates inherited ByBlock style through nested inserts; ByBlock child geometry can inherit nearest explicit insert color/linetype/lineweight.
- Native INSERT hatch rendering now also propagates inherited ByBlock style through nested inserts, so Hatch(ByBlock) inherits the nearest explicit insert color.
