# PDF Export

H7CAD ships a print-quality PDF exporter that mirrors the SVG pipeline
described in [`svg_export.md`](svg_export.md).  All behaviour is configurable
through `PdfExportOptions` and driven by the `PDFEXPORT` / `EXPORTPDF`
command-line aliases, the graphical options dialog opened via
`PDFEXPORTDIALOG` / `PDFOPTIONS`, the headless `h7cad … --export-pdf` CLI
(R36+), or the `--options <PATH>` JSON override (R38+).

## Usage

### Headless CLI (R36 + R37 multi-input + R38 JSON override)

```
h7cad INPUT.dxf --export-pdf OUTPUT.pdf
h7cad INPUT.dxf --export-pdf                    # infer OUTPUT = INPUT.pdf
h7cad A.dxf B.dxf --export-pdf OUTDIR\          # batch into directory
h7cad INPUT.dxf --export-pdf OUT.pdf --options opts.json
```

Exit code is `0` on full success and `1` if any input failed (others still
run).  Diagnostics go to stderr.

### Interactive (options dialog)

```
PDFEXPORTDIALOG    -- open the PDF Export options window
PDFOPTIONS         -- alias for PDFEXPORTDIALOG
```

The dialog groups toggles into **Color & Strokes**, **Text**, **Geometry**,
and **Images**.  Hit *Export…* to chain into the save-file picker.

The **Color & Strokes** section exposes the R41 three-way `ColorPolicy`
pill group (Full / Mono / Gray) plus a legacy `Monochrome` bool toggle —
the bool stays in sync with the pill so external consumers / JSON
exports do not see drift.

### Programmatic

```rust
use crate::io::pdf_export::{export_pdf_full, PdfExportOptions, PdfFontChoice};
use crate::io::color_policy::ColorPolicy;

let opts = PdfExportOptions {
    color_policy: ColorPolicy::Grayscale,   // R41 — three-way enum
    font_family: PdfFontChoice::TimesRoman,
    ..PdfExportOptions::default()
};

export_pdf_full(
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
| `color_policy` | `Monochrome` | `ColorPolicy` (0/1/2) | **R41**: `Full` / `Monochrome` / `Grayscale` three-way enum, mirroring SVG R40. Wins over the legacy `monochrome` bool whenever non-default. Grayscale uses BT.601 luminance (`0.299R + 0.587G + 0.114B`). |
| `monochrome` | `true` | `ColorPolicy=1` | **Legacy** — retained for JSON backwards compatibility with pre-R41 `--options` files. Honoured only when `color_policy` is left at its default (`Monochrome`). New code should set `color_policy` directly. |
| `text_as_geometry` | `false` | `TextAsGeometry` | Skip native PDF text, emit wires |
| `font_family` | `Helvetica` | Standard 14 PDF font | `Helvetica` / `TimesRoman` / `Courier` |
| `font_size_scale` | `0.8` | post-process `font_size_scale` | Applied to DXF `height` |
| `include_hatches` | `true` | — (PDF extension) | Emit solid HATCH fills as polygons |
| `hatch_patterns` | `true` | — (R33 Phase 2) | Emit `HatchPattern::Pattern` line families as real PDF lines |
| `gradient_hatches` | `true` | — (R39 Phase 3) | Emit `HatchPattern::Gradient` as 48-strip linear interpolation. Mono mode uses a `0.15 + 0.70u` grey ramp; Grayscale mode (R41) projects each strip through BT.601 luminance |
| `include_images` | `true` | raster pipeline | Embed RasterImage entities as XObjects |
| `embed_images` | `true` | — | When `false`, behaves identical to `include_images=false` (PDF cannot reference external image URIs) |
| `image_base` | `None` | `ImageBase` | Resolve relative `file_path` values on IMAGE entities |
| `native_curves` | `true` | — (H7CAD extension) | Emit `Circle` / `Arc` / `Ellipse` as native PDF bezier paths instead of WireModel tessellation |
| `native_splines` | `true` | — (R35 Phase 3) | Degree-1 control polyline / degree-2/3 piecewise cubic bezier / fit-point fallback |
| `native_dimension_text` | `true` | — (Phase 8) | Emit dimension measurement as native PDF text instead of SHX-tessellated wires |

## Color policy (R41 — PDF mirror of SVG R40)

The three-way `ColorPolicy` enum is shared with the SVG exporter
(`crate::io::color_policy`) so both pipelines agree byte-for-byte on what
each mode means:

| Mode | Use case |
|---|---|
| `Full` | Web / preview where original ACI or true-colour information must survive |
| `Monochrome` (default) | Black-and-white print output; keeps byte-level parity with every pre-R41 export |
| `Grayscale` | Printing on single-toner laser printers where gradient / hatch direction must still be distinguishable — BT.601 luminance preserves relative brightness ordering |

### GUI

The PDF dialog's **Color policy** pill group shows the current effective
mode.  Flipping it updates both `color_policy` (enum) and the legacy
`monochrome` bool so external consumers / JSON exports stay consistent.

### CLI

```bash
# Full colour:
echo '{"color_policy":"full"}' > full.json
h7cad drawing.dxf --export-pdf out.pdf --options full.json

# Grayscale:
echo '{"color_policy":"grayscale"}' > gray.json
h7cad drawing.dxf --export-pdf out.pdf --options gray.json

# Monochrome (default — no --options file needed):
h7cad drawing.dxf --export-pdf out.pdf
```

### JSON backwards compatibility

Pre-R41 `--options` files using `{"monochrome": true}` or
`{"monochrome": false}` continue to parse and behave exactly as before.
When both keys are present, the new `color_policy` enum wins whenever its
value is non-default, otherwise the legacy bool takes effect — giving every
possible combination a single, well-defined resolution:

| `color_policy` | `monochrome` | effective |
|----------------|--------------|-----------|
| `Full` | _ignored_ | `Full` |
| `Grayscale` | _ignored_ | `Grayscale` |
| `Monochrome` (default) | `true` | `Monochrome` |
| `Monochrome` (default) | `false` | `Full` |

### Special cases (preserved across the three modes)

- **Solid HATCH fills** under `Monochrome` keep the R32+ `(0.80, 0.80, 0.80)`
  light-grey override so hatches stay visible without overpowering strokes
  on a B&W print.  `Grayscale` and `Full` route the source colour through
  the shared helper.
- **Gradient HATCH** (R39) under `Monochrome` keeps the special
  `0.15 + 0.70u` grey ramp so the gradient direction stays legible on a
  B&W printer (a literal `apply_color_policy(Mono, ...)` would collapse
  every strip to black).  `Grayscale` and `Full` route through the helper.
- **Native PDF text** has no per-entity colour pipe yet, so all three modes
  currently emit `(0, 0, 0)`.  The R41 refactor still routes through the
  policy helper so a future change to pipe entity colour through here gets
  the policy mapping for free.

## Element layer order

The PDF page is painted bottom-to-top:

1. Solid HATCH fills (`Op::DrawPolygon` with `PaintMode::Fill`).
2. Pattern HATCH lines (R33 Phase 2).
3. Gradient HATCH strip polygons (R39 Phase 3).
4. RasterImage XObjects.
5. WireModel strokes (LINE / CIRCLE tess / ARC tess / LWPOLYLINE / …),
   skipping wires whose entity is rendered natively above.
6. Native curves (`Circle` / `Arc` / `Ellipse` bezier paths).
7. Native splines (degree-2/3 piecewise cubic bezier).
8. Native PDF text (`Op::ShowText` with built-in Standard 14 font).

## Tests

`cargo test --package H7CAD --bin H7CAD io::pdf_export` runs the full
suite (32 tests after R41).  Highlights:

- `fixture_pdf_options_monochrome_forces_black_strokes` — pre-R41
  byte-level canary; ensures the legacy `monochrome` bool path keeps
  emitting black strokes.
- `pdf_three_color_policies_produce_distinct_bytes` — R41 sanity check
  that `Full`, `Monochrome`, and `Grayscale` all produce pairwise
  distinct byte streams for the same fixture.
- `pdf_grayscale_red_wire_uses_bt601_luminance` — proves the new
  Grayscale branch is reached and emits a non-black colour for a
  pure-red source wire.
- `pdf_gradient_grayscale_strips_use_luminance_ramp` — confirms the R39
  gradient strip pipeline routes through `apply_color_policy` when
  Grayscale is active, producing bytes distinct from both Mono and Full.
- `pdf_legacy_json_monochrome_*` — JSON backwards-compat canaries that
  every pre-R41 `--options` file resolves to the same effective policy
  it did before.
- `pdf_new_json_color_policy_*` — coverage for the R41 `color_policy`
  enum across all three variants and the conflict-resolution rule
  (`color_policy=full` overrides `monochrome=true`).
