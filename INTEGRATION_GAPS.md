# acadrust Integration Gaps

Missing or incomplete integrations between acadrust entity definitions and H7CAD rendering/interaction systems. Ordered by priority within each category.

---

## Entity Field Gaps (acadrust fields ignored in tessellation)

### High Impact

| Entity | Ignored Field(s) | Effect |
|---|---|---|
| **INSERT** | `column_count`, `row_count`, `column_spacing`, `row_spacing` | MINSERT array inserts not expanded — only one copy rendered |
| **LWPolyline** | `constant_width`, vertex `start_width` / `end_width` | Polylines always render at zero width; thick/tapered strokes lost |
| **Polyline (legacy)** | `Vertex2D.start_width` / `end_width` | Same as above for old-format polylines |

### Medium Impact

| Entity | Ignored Field(s) | Effect |
|---|---|---|
| **Dimension** | `DIMSCALE`, `DIMASZ`, `DIMEXO`, `DIMEXE` (from dimstyle) | Arrow size hardcoded to `0.12`; extension line offsets fixed regardless of style |
| **Spline** | `weights` (rational NURBS control point weights) | Circles/conics stored as NURBS lose curvature precision; treated as uniform B-spline |
| **Spline** | `flags.closed` / `flags.periodic` | Closed splines may have a gap or C1 discontinuity at the join |
| **Text / MText** | `normal` extrusion XY | Entities on tilted planes render flat in XY (rare in 2D drawings) |
| **Hatch** | `BoundaryEdge::Spline` tessellation | Spline boundary edges plotted point-to-point instead of smooth B-spline curve — boundary appears angular |
| **MultiLeader** | `MultiLeaderPathType::Spline` | Spline-type leader lines rendered as straight segments instead of smooth curves |
| **RasterImage** | `clip_boundary` (polygonal/rectangular) | Clip boundary read but not applied — image renders full rectangle regardless of clip region |

### Low Impact

| Entity | Ignored Field(s) | Effect |
|---|---|---|
| **Arc / Circle / Line / Polyline** | `thickness` | No 3D extrusion along Z; invisible in pure 2D views |
| **LWPolyline** | `plinegen` flag | Linetype pattern resets at each vertex instead of continuing |

---

## Entity Type Gaps (entire types with missing subsystems)

### Renders Nothing / Placeholder Only

| Entity | Status |
|---|---|
| **Face3D** | No tessellation — renders as small cross only; no snap, grip, or properties |
| **OLE2Frame** | Bounding box + X mark only; no interactivity |

### Wire Fallback Only (no full mesh)

| Entity | Missing |
|---|---|
| **Solid3D / Region / Body** | Only ACIS wire fallback; no grip, no properties panel |

### Partial Render

| Entity | Missing |
|---|---|
| **Viewport** (paper space) | Only frame rendered; interior model-space view not composited |

---

## Systemic Gap — OCS→WCS Transform

17 entity types carry a `normal` extrusion vector defining their Object Coordinate System but **none apply the arbitrary-axis OCS→WCS transform** during tessellation. When `normal ≠ (0, 0, 1)` the entity is incorrectly rendered flat in the XY plane.

**Affected entities:** Arc, Circle, Ellipse, Point, Line, Spline, LwPolyline, Polyline, AttributeDefinition, AttributeEntity, Dimension, Hatch, MLine, Leader, Insert, Shape

**Only mitigation today:** `Arc` checks `normal.z < 0` to reverse sweep direction — not a real OCS transform.

**Impact:** Low for typical 2D plan files (nearly all normals are `(0,0,1)`); high for 3D DXF files with entities on non-horizontal planes.

The DXF arbitrary-axis algorithm:
```
if |Wx| < 1/64 and |Wy| < 1/64:
    Ax = (0,0,1) × N   (Y-world cross N)
else:
    Ax = (0,1,0) × N   (X-world cross N)
Ax = normalize(Ax)
Ay = N × Ax
```
Then transform each OCS point: `WCS = origin + x*Ax + y*Ay + z*N`

---

## Render Style Gaps (color, linetype, lineweight resolution)

### High Impact

| Gap | Effect | Location |
|---|---|---|
| **ByBlock color** not resolved through INSERT chain | ByBlock entities render white instead of inheriting INSERT's color | `render.rs::render_style_for()` |
| **ByBlock linetype** not resolved through INSERT chain | ByBlock entities use default linetype instead of INSERT's linetype | `render.rs::render_style_for()` |

### Medium Impact

| Gap | Effect | Location |
|---|---|---|
| **ByBlock lineweight** resolved from layer instead of INSERT entity | Block children with ByBlock weight don't inherit INSERT's line weight | `render.rs` lines 213-228 |

---

## Text & Style Gaps

### Medium Impact

| Gap | Effect | Location |
|---|---|---|
| **TextStyle `is_backward`** flag ignored | Text with backward flag renders left-to-right instead of mirrored | `src/entities/text_support.rs::resolve_text_style()` |
| **TextStyle `is_upside_down`** flag ignored | Text with upside-down flag renders normally instead of flipped vertically | `src/entities/text_support.rs::resolve_text_style()` |

### Low-Medium Impact

| Gap | Effect | Location |
|---|---|---|
| **Complex linetype text shapes** not rendered | Linetypes with embedded text elements (e.g. GAS_LINE with "GAS" text) show only geometry gaps | `src/scene/complex_lt.rs` |

---

## Polyline3D Vertex Type Gap

acadrust `Vertex3D` carries `VertexFlags` (SPLINE_VERTEX, SPLINE_CONTROL) but `tessellate_polyline3d()` ignores them entirely — all vertices treated as plain points regardless of spline flag.

**Effect:** Polyline3D entities with spline fitting render as straight segments instead of smooth curves.  
**Location:** `src/entities/polyline.rs::tessellate_polyline3d()`  
**Impact:** Medium.

---

## Snap Point Gaps

### Critical / High

| Entity | Missing Snap | Effect | Location |
|---|---|---|---|
| **INSERT** | `Insertion` snap point entirely absent | Cannot snap to block reference insertion point with OSNAP Insertion | `src/entities/insert.rs` — no `snap_pts` in TruckEntity |
| **INSERT** | Nested entity snap points not traversed | Cannot snap to geometry inside a block without exploding it | `src/snap/mod.rs` — only top-level wire snap_pts checked |
| **Hatch** | All snap points empty | Cannot snap to hatch boundary vertices/arcs with Endpoint or Midpoint | `src/scene/tessellate.rs:558` — returns `snap_pts: vec![]` |

### Medium

| Entity | Missing Snap | Effect | Location |
|---|---|---|---|
| **Dimension** | No snap hints on geometry | Cannot snap to dimension defpoints, line endpoints | `src/scene/tessellate.rs:247` — `snap_pts: vec![]` |
| **Spline** | Fit/control points not in snap_pts | Cannot snap to spline construction points | `src/entities/spline.rs:42` |
| **MultiLeader** | Vertices not in snap_pts | Cannot snap to leader line endpoints | `src/entities/multileader.rs:116` |
| **MLine** | Vertices not in snap_pts | Cannot snap to multiline segment endpoints | `src/entities/mline.rs:97` |

### Low

| Entity | Missing Snap | Effect | Location |
|---|---|---|---|
| **Ellipse** (partial arc) | Endpoints not in pre-baked snap_pts | Arc endpoints emitted only as `Center`; functional via wire tessellation but semantically wrong | `src/entities/ellipse.rs:108` |
| **Hatch** | Elevation Z ignored | Snap Z is always 0 instead of `hatch.elevation` | `src/scene/tessellate.rs:564` — hardcoded `0.0` |

---

## Grip Gaps

| Entity | Missing Grip | Effect | Location |
|---|---|---|---|
| **LWPolyline** | Midpoint grips between vertices (arc segment handles) | Cannot drag a segment midpoint to adjust arc bulge | `src/entities/lwpolyline.rs` |

---

## Text Rendering Gaps

| Gap | Effect | Location |
|---|---|---|
| **TextStyle `is_backward`** flag not applied | Text with backward flag renders left-to-right | `src/entities/text_support.rs::resolve_text_style()` |
| **TextStyle `is_upside_down`** flag not applied | Text with upside-down flag renders normally | `src/entities/text_support.rs::resolve_text_style()` |
| **Unicode characters** not in CXF fonts silently dropped | Non-ASCII characters render as blank gaps with no warning | `src/scene/cxf.rs:174` |

---

## DXF Reader Unit Gaps (acadrust bugs we work around)

These are fixed in our post-load `fix_dxf_dimension_rotations()` in `src/io/mod.rs`, documented here for reference.

| Entity | Field | DXF Code | Bug | Fix Location |
|---|---|---|---|---|
| **Dimension (Linear)** | `rotation` | 50 | Stored in degrees, used as radians | `io/mod.rs::fix_dxf_dimension_rotations()` |
| **Dimension (all)** | `text_rotation` | 53 | Code 53 never parsed; always 0 | `tessellate.rs::dimension_text_natural_rotation()` |
| **AttributeEntity** | `rotation` | 50 | No `.to_radians()` in reader | Needs fixup (not yet applied) |
| **AttributeDefinition** | `rotation` | 50 | No `.to_radians()` in reader | Needs fixup (not yet applied) |
| **Shape** | `rotation` | 50 | No `.to_radians()` in reader | Shape not currently rendered — low priority |

---

## Coverage Summary

| Subsystem | Coverage |
|---|---|
| Tessellation | 34/41 entity types fully, 4 legacy fallback, 3 missing |
| Snap points | 36/41 (Face3D, Solid3D, Region, Body, OLE2Frame missing) |
| Grip points | 36/41 (same 5 missing) |
| Properties panel | 36/41 (same 5 missing) |
| Hit testing | 41/41 (all via fallback) |
