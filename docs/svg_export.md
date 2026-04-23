# SVG Export

H7CAD ships with a first-class SVG exporter that reproduces the ODA
`OdSvgExportEx` algorithm end-to-end and replaces the Python post-processing
script ODA users typically run after the C++ exporter.  All behaviour is
configurable through `SvgExportOptions` and driven by the
`SVGEXPORT` / `EXPORTSVG` command-line aliases or the graphical options
dialog opened via `SVGEXPORTDIALOG` / `SVGOPTIONS`.

## Usage

### Interactive (command line)

```
SVGEXPORT                   -- monochrome, embed images, native curves (defaults)
SVGEXPORT COLOR             -- keep ACI/true-colour strokes
SVGEXPORT MONO              -- force all strokes to black (ColorPolicy=1)
SVGEXPORT TEXT              -- emit <text> elements (default)
SVGEXPORT TEXTGEOM          -- tessellate text into paths (TextAsGeometry=true)
SVGEXPORT NOHATCH           -- skip hatch fills
SVGEXPORT NOIMAGE           -- drop raster images entirely
SVGEXPORT EXTIMG            -- reference images via ./filename instead of embedding
SVGEXPORT NOCURVES          -- disable native <circle>/<ellipse>/<path A> curves
SVGEXPORT NOSPLINES         -- force Spline entities through WireModel tessellation
```

Subcommands may be combined: `SVGEXPORT COLOR EXTIMG NOHATCH`.

### Interactive (options dialog)

```
SVGEXPORTDIALOG    -- open the SVG Export options window
SVGOPTIONS         -- alias for SVGEXPORTDIALOG
```

The dialog groups toggles into **Color & Strokes**, **Text**, **Geometry**,
and **Images**.  Hit *Export…* to chain into the save-file picker.  Numeric
fields (font size scale, min stroke width, line-weight scale) accept free
text and fall back to the live value when the input fails to parse.

### Programmatic

```rust
use crate::io::svg_export::{export_svg_full, SvgExportOptions};

let opts = SvgExportOptions {
    monochrome: false,
    embed_images: false,
    image_url_prefix: "images/".into(),
    ..Default::default()
};

export_svg_full(
    &wires,
    &hatches,
    Some(&native_doc),
    paper_w, paper_h,
    offset_x, offset_y,
    rotation_deg,
    &output_path,
    plot_style_ref,
    &opts,
)?;
```

## Options reference

| Field | Default | ODA mapping | Notes |
|---|---|---|---|
| `monochrome` | `true` | `ColorPolicy=1` | Force black strokes |
| `text_as_geometry` | `false` | `TextAsGeometry` | Skip `<text>`, emit wires |
| `font_family` | `"SimSun, 宋体"` | `getSubstituteFont` / `getPreferableFont` | CSS font stack |
| `font_size_scale` | `0.8` | post-process `font_size_scale` | Applied to DXF `height` |
| `min_stroke_width` | `0.1` | `MinimalWidth=0.1` | Floor in mm |
| `line_weight_scale` | `0.2646` | `LineWeightScale` | px → mm at 96 dpi |
| `include_hatches` | `true` | `HatchAsPolygon` | `<polygon>` fills |
| `use_block_defs` | `true` | — (H7CAD extension) | `<defs>`+`<use>` dedup |
| `include_images` | `true` | raster pipeline | Emit `<image>` |
| `embed_images` | `true` | — | Inline `data:` URI |
| `image_url_prefix` | `"./"` | `ImageUrl` | Used when `embed_images=false` |
| `image_base` | `None` | `ImageBase` | Resolve relative paths |
| `native_curves` | `true` | — (H7CAD extension) | `<circle>`/`<ellipse>`/`<path A>` |
| `native_splines` | `true` | — (H7CAD extension) | Degree-1 control polygon / fit-point approximation |

## Element layer order

Inside the root Y-flipped group `<g transform="translate(0,ph) scale(1,-1)">`,
the exporter stacks from bottom to top:

1. `<defs>` — shared block symbols (Insert/Block deduplication).
2. `<image>` — raster entities.
3. `<polygon>` — hatch fills.
4. `<polyline>` — fallback WireModel strokes.
5. `<use>` — Insert references pointing at the `<defs>` entries.
6. `<circle>` / `<ellipse>` / `<path d="... A ...">` — native curves.
7. `<text>` — Text / MText entities (counter-flipped for the outer scale).

## Algorithm parity with ODA

### Covered

- `UseTextOut=true` — native `<text>` emission
- `ExplodeShxTexts=false` — SHX fonts substituted to the configured family
- `ColorPolicy=1` — monochrome mode
- `UseLineTypes=true` — `stroke-dasharray` built from linetype pattern
- `MinimalWidth=0.1` — `min_stroke_width` floor
- `TextAsGeometry=false` — default; togglable
- `ImageBase` / `ImageUrl` / `DefaultImageExt` — raster image pipeline
- `LineWeightScale` — lineweight multiplier
- Responsive viewBox + `width/height=100%` + `preserveAspectRatio`
- MText control codes: `\P`, `\f`, `\H`, `\C`, `\S` stacking, `\\`, `\{`, `\}`
- Layer visibility (`frozen` / `off`) — skipped everywhere

### H7CAD extras (not in ODA)

- Block defs/use deduplication (shared block geometry)
- CTB plot-style colour and lineweight resolution
- Self-contained SVG output (base64-embedded images)
- Native `<circle>` / `<ellipse>` / arc `<path>` for top-level curves
- Native `<path d="... A ...">` for LwPolyline with bulge segments
- Degree-1 Splines and fit-point Splines emitted as native polylines

### Intentional differences

- ODA tessellates curves via `2dExportDevice`; H7CAD keeps them native for
  smaller, resolution-independent SVG output.  Set `native_curves=false`
  to force WireModel emission if a downstream consumer cannot handle
  native SVG curves.
- ODA runs a separate `svg_postprocess.py` script after export to fix
  glyph-encoded text and scale fonts.  H7CAD emits `<text>` directly, so
  no post-processing is needed.

## Frequently asked

**Q: The SVG looks mirrored vertically.**
A: Check that the `<text>` elements carry `transform="... scale(1,-1) ..."`.
The global Y-flip is applied by the outer `<g>`; text needs a counter-flip
so character glyphs stay upright.  `emit_text_element` handles this — if
you are calling `build_svg_full` directly, make sure `text_as_geometry` is
not set to `true` unintentionally.

**Q: My raster images appear upside-down.**
A: Verify that `u_vector` and `v_vector` on the `IMAGE` entity match the
DXF convention (u along image +X, v along image +Y, both in world units
per pixel).  The transform matrix `matrix(u.x, u.y, -v.x, -v.y, ...)`
assumes that orientation.

**Q: Exported SVG is huge.**
A: Likely `embed_images=true` with large raster images.  Try
`SVGEXPORT EXTIMG` to reference images via `./filename` instead, and copy
the image files next to the SVG file.

**Q: A specific block reference still appears as a tessellated polyline.**
A: The block's eligibility checker rejects any block containing Text,
MText, Hatch, nested Insert, Dimension, Spline, Face3D, or Solid.  Blocks
with those child entities continue to rely on the WireModel pipeline.
LwPolylines (bulged or straight) and Ellipses are supported as of Phase 4.

## Tests

`cargo test --package H7CAD --bin H7CAD io::svg_export` runs the full
suite.  The end-to-end fixture `tests/fixtures/sample.dxf` covers the
Line / Circle / Arc / Text path and is mirrored by
`sample_dxf_end_to_end_produces_all_native_shapes`.
