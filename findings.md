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
- Native Hatch boundary rendering now projects OCS boundary points through entity `extrusion` before producing the 2D hatch model boundary.
- Native Hatch circular and elliptic arc boundary sampling now treats start/end angles as DXF degrees and converts them to radians for tessellation.

## DWG Native Writer Bring-up

- F5.M1 (`BitWriter` mirroring `BitReader`) and F5.M2.T1+T2 (`writer/file_header.rs`) live in `crates/h7cad-native-dwg`.
- AC1015 file header prefix is **fixed at 0x19 bytes**: only `magic` (`0x00..0x06`) and `section_count` (`0x15..0x19`, LE u32) are reader-relevant; `release marker / preview seeker / undocumented / codepage` are writer-side defaults (zeros / `0xFFFFFFFF` / zeros / `30`).
- AC1015 section locator directory entries are **9 bytes each**: `record_number: u8` then LE `offset: u32` then LE `size: u32`.
- Writer **shares the reader's section-count cap** (`section_map::MAX_SECTION_RECORDS = 128`); writers that would exceed it are rejected with `DwgWriteError::InvalidValue` instead of producing a file the reader would refuse on read-back.
- F5.M2 closed (T3 + T4 landed 2026-05-09): six known sections now have empty-payload composers, `write_dwg(empty CadDocument)` round-trips through `read_dwg`, and `tests/roundtrip_minimal.rs` locks the resulting invariants.
- F5.M3 closed (landed 2026-05-09): `write_ac1015_handle_map_payload` is the byte-aligned inverse of `parse_handle_map`. Single-chunk strategy with `MAX_CHUNK_PAYLOAD = 2032`; entries auto-sorted ascending by handle (reader assumes monotonicity); duplicate handles → `InvalidDocument`; over-budget entry stream → `SectionTooLarge { section: "AcDb:Handles", .. }`. CRC trailer is `0x00 0x00` placeholder because the reader does not validate per the long-standing advisory comment in `handle_map.rs`.
- F5.M4 in progress (sub-plan: `docs/plans/2026-05-09-dwg-m4-object-stream-writer-plan.md`). M4.A and M4.B both landed 2026-05-09:
  - M4.A: `write_ac1015_object_header` / `write_ac1015_object_self_header` mirror `read_ac1015_object_header` (BS/RL/H three-field layout). Self-header bit count is 58 bits when the handle fits in one byte, matching `reader_positioned_exactly_after_header`.
  - M4.B: `compose_ac1015_object_slice(header, &main_stream, &handle_stream)` glues M4.A with caller-supplied bit streams, validates `main_size_bits == header_bit_count + main_stream.bits()` up-front, byte-aligns the body, prepends a modular-short MS prefix, appends a two-byte CRC stub, and round-trips through `split_ac1015_object_streams`.
  - M4.C: `write_ac1015_entity_common_minimal(EntityCommonMinimal, &mut BitWriter, &mut BitWriter)` mirrors `parse_ac1015_entity_common_after_extended_data` for a hard-coded minimal flag configuration. caller-controlled fields: `layer_handle / color_index / linetype_scale / invisible`. Known limitations carried forward to M5: `owner_handle` decodes to `NULL` regardless of caller input (entity_mode = 0b01 path), and `lineweight` decodes to `-3` ByDefault regardless of caller input (lineweight_index hard-coded to 31). Both limitations are explicitly locked by `round_trips_lineweight_decodes_to_by_default`.
  - M4.D: `write_line_geometry(LineGeometry, &mut BitWriter)` mirrors `read_line_geometry`'s 9-field bit-stream (B z_are_zero / RD start / DD end / RD start_y / DD end_y / optional 3D / BT thickness / BE extrusion). z_are_zero is auto-detected; IEEE 754 quirk (`-0.0 == +0.0`) causes `-0.0` z to take the compact path and the reader recovers `+0.0`. Minimum LINE payload size is exactly **135 bits** (z=0 + thickness=0 + extrusion default + DD start==end), locked by `dd_default_path_used_when_end_equals_start`.
  - Remaining sub-milestones: M4.E (`write_dwg` integration + LINE roundtrip). Beyond M4: F5.M5 (21 more entity body writers) → F5.M6 (facade switch) → F5.M7 (AC1018 writer).
- AC1015 object slice physical layout (memorised for M4.B): `[MS body_size][BS object_type][RL main_size_bits][H handle][...main bits + handle bits...][2-byte CRC]`. `main_size_bits` is the *absolute* bit position (counted from the first body bit, i.e. right after the MS prefix) where the handle stream begins. `ObjectStreamCursor::object_slice_by_handle` deliberately drops the trailing CRC before invoking `read_ac1015_object_header`, so the writer's M4.B composer is the only place that needs to emit the CRC stub.
- Local test harness flakiness: on Windows, repeated `cargo test -p h7cad-native-dwg --all-targets` runs occasionally trip `LNK1104: cannot open file h7cad_native_dwg-*.exe` because the prior test runner still holds the executable. A 3–5 s sleep before retrying clears it. Recorded so future agents do not chase this as a code regression.
- **`CadDocument::new()` is not "empty"**. It pre-seeds the default `0` layer, `Continuous / ByLayer / ByBlock` linetypes, `Standard` text + dim styles, `*Active` vport, `*Model_Space` / `*Paper_Space` block records, and `Model` / `Layout1` layouts. The reader's `resolve_document` echoes these defaults verbatim, so writer round-trip tests must assert `parsed.layers == fresh.layers` (not `parsed.layers.is_empty()`).
- Reader path is friendly to the empty-payload writer: `classify_section_records_for_section` short-circuits on empty payload (`Ok(Vec::new())`); `enrich_with_real_entities` early-returns on empty `pending.handle_offsets`; `resolve_document` walks empty `pending.layers` / `pending.objects` without error. No reader-side fixes were needed for F5.M2 closure.
- Header / Classes 16-byte sentinels (`KnownSection::start_sentinel` / `end_sentinel`) are intentionally *not* emitted at the F5.M2 milestone — the native reader does not validate them, and emitting them would commit the writer to specific magic bytes before the M5/M6 acadrust-interop story is ready.

## Build Environment

- Full-workspace `cargo test --workspace --all-targets` on this Windows machine occasionally fails to link the `H7CAD` bin with `rustc-LLVM ERROR: out of memory` (status `0xc0000409`). Root cause is the heavy iced/wgpu/truck dependency graph in the main bin, not any specific change. Fallbacks that succeed:
  - `cargo test -p h7cad-native-dwg --all-targets` (and other native subcrates) — no link of the main bin.
  - `cargo check --workspace --all-targets` — no link, only typecheck.
  Use these for per-PR verification of work scoped to the native crates.
