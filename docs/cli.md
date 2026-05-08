# H7CAD Headless CLI

`h7cad` ships with a headless batch path so CI / automation pipelines can
convert DXF (and DWG, where the runtime supports it) to PDF and SVG without
launching the iced GUI.  Pass any batch flag to divert from the GUI entry
point; otherwise `main.rs` falls through to the GUI.

## Quick start

```
h7cad                                            # Launch the GUI
h7cad drawing.dxf                                # GUI, opens drawing.dxf
h7cad drawing.dxf --export-pdf out.pdf           # Headless DXF → PDF
h7cad drawing.dxf --export-svg out.svg           # Headless DXF → SVG
h7cad A.dxf B.dxf C.dxf --export-pdf OUT_DIR/    # Batch into directory
h7cad drawing.dxf --export-pdf --options o.json  # Override export options
h7cad drawing.dxf --list-layouts                 # Print available layouts
h7cad drawing.dxf --export-pdf --layout Layout1  # Export one paper layout
h7cad --help                                     # Show usage
```

Exit code is `0` on full success and `1` if any input failed.  Failures are
non-fatal — remaining inputs still attempt export and a per-file
diagnostic is printed to stderr.

## Flags

### `--export-pdf [OUTPUT]` / `--export-svg [OUTPUT]` (R36 / R37)

Pick the output format.  `OUTPUT` resolution rules:

| `OUTPUT` | Behaviour |
|---|---|
| Omitted | Each input's parent dir + stem + `.pdf` / `.svg` |
| Ends with `/` or `\` | Treated as a directory (required for multi-input) |
| Existing directory | Same as above |
| Otherwise | Single output file (only valid when exactly one input is given) |

Multi-input invocation requires either omitting `OUTPUT` (per-input
inferred) or passing a directory.  Passing a single file with multiple
inputs early-exits with a clear error.

### `--options <PATH>` (R38)

JSON file overriding any field of the default `PdfExportOptions` /
`SvgExportOptions`.  All fields are optional — missing keys fall back to
the built-in default.  Shared across every input in a multi-input
invocation.

```json
{
  "monochrome": false,
  "color_policy": "grayscale",
  "font_family": "TimesRoman",
  "include_hatches": true
}
```

The flag is order-agnostic; can appear before or after `--export-*`.
Malformed JSON / missing files fail fast before any input is processed
(so a bad config does not produce N partial outputs).

### `--layout NAME` (R42-A)

Select which layout to export.  `NAME` is case-sensitive and must match
a value returned by `--list-layouts`.  Default: `Model`.

```
h7cad drawing.dxf --export-pdf --layout "Layout1" out.pdf
```

Unknown layout names produce a clear error containing the available
list:

```
h7cad: drawing.dxf failed: unknown layout "Foo".  Available: Model, Layout1, Layout2
```

The flag is order-agnostic relative to `--export-*` and `--options`.
**The CLI flag wins over the JSON `layout` field (R42-B).**

### JSON `layout` field (R42-B)

The `--options` JSON file (R38) supports a top-level `layout` key so a
config file can bake in the layout choice:

```json
{
  "color_policy": "grayscale",
  "layout": "Layout1"
}
```

```
h7cad drawing.dxf --export-pdf --options gray-l1.json out.pdf
```

When both `--layout` flag and JSON `layout` field are present, the **CLI
flag wins**.  Both default to `None`, which falls through to the Scene's
"Model" default for byte-identical pre-R42 output.

| Source | Resolution |
|---|---|
| Neither set | Scene default `Model` (pre-R42 behaviour) |
| Only JSON | JSON value |
| Only CLI flag | CLI flag value |
| Both set | CLI flag wins (JSON value ignored) |

### `--list-layouts` (R42-A)

Print available layouts for each input (one per line) and exit.  Used to
discover which `NAME`s to feed into `--layout`.

Single-input output:

```
$ h7cad drawing.dxf --list-layouts
Model
Layout1
Layout2
```

Multi-input output prefixes each block with `=== <path> ===`:

```
$ h7cad a.dxf b.dxf --list-layouts
=== a.dxf ===
Model
Layout1

=== b.dxf ===
Model
```

Useful in shell loops:

```bash
for L in $(h7cad drawing.dxf --list-layouts); do
    h7cad drawing.dxf --export-pdf --layout "$L" "out_${L}.pdf"
done
```

### `--list-layouts --json` (R42-C)

Add `--json` after `--list-layouts` to emit a structured JSON document
instead of line-formatted text.  The schema is friendly to `jq` and
similar tools:

```json
{
  "inputs": [
    { "path": "drawing.dxf", "layouts": ["Model", "Layout1", "Layout2"] },
    { "path": "broken.dxf",  "error":   "failed to load: ..." }
  ]
}
```

Failed inputs appear with an `error` field instead of `layouts`; the
exit code still reflects the failure (exit 1 if any input failed).
The output is always valid JSON regardless of partial failures so `jq`
parsing never breaks.

```bash
h7cad drawing.dxf --list-layouts --json | jq -r '.inputs[0].layouts[]'
```

## DWG input (R44-A)

The CLI accepts `.dwg` inputs as transparently as `.dxf`:

```
h7cad drawing.dwg --export-pdf out.pdf
h7cad drawing.dwg --list-layouts
```

DWG opens follow H7CAD's standard reader pipeline:

1. The primary reader is **acadrust** (covers AutoCAD 2010+ ACAD files).
2. If acadrust rejects the bytes, the loader falls back to
   **`h7cad-native-dwg`** for AC1015-vintage files.  Fallback success
   surfaces a `[Warning]` notice on stderr and continues with the
   recovered document; fallback failure returns the original acadrust
   error and the export aborts.

### Notice output

Both code paths may surface non-fatal notices on stderr — one line each:

```
h7cad: drawing.dwg: notice [Warning] native DWG fallback opened file after acadrust failed: ... (recovered 84 entities)
h7cad: drawing.dwg: notice [NotImplemented] some DWG record skipped: ...
```

Notices are **additive** — they never block the export.  If your CI
wants silent stderr, redirect:

```bash
h7cad drawing.dwg --export-pdf out.pdf 2>/dev/null   # POSIX
h7cad drawing.dwg --export-pdf out.pdf 2> NUL         # PowerShell / cmd
```

`stdout` stays clean for the file output (PDF / SVG bytes are written
to disk, not stdout) and for `--list-layouts` text / JSON output, so
piping `stdout` to `jq` / file remains safe regardless of notice
volume.

## PID input (R44-B)

The CLI accepts SmartPlant `.pid` inputs as transparently as
`.dxf` / `.dwg`:

```
h7cad drawing.pid --export-pdf out.pdf
h7cad drawing.pid --list-layouts
h7cad drawing.pid --export-svg out.svg
```

PID opens go through `pid_parse`'s `PidParser::parse_package`, followed
by sidecar merge (`_Data.xml` / `_Meta.xml`) and layout derivation. The
resulting native document carries the laid-out objects, relationships,
and a fallback grid when the object graph stream is unavailable.

### Notice output

Successful PID loads can still surface non-fatal notices when the
parsed package has incomplete diagnostic signals — same one-line
shape as the DWG path:

```
h7cad: drawing.pid: notice [Warning] PID has 3 unresolved relationships (dropped from layout)
h7cad: drawing.pid: notice [NotImplemented] PID object graph stream missing; layout falls back to grid
```

The `[Warning]` line means some relationships in the PID document point
at endpoints that could not be resolved (missing object handle,
placeholder GUID, etc.) and were dropped from the layout. The
`[NotImplemented]` line means the package has no object graph stream
at all, so the layout falls back to a static grid.

Both notices are **additive** — the PDF / SVG output is still produced.
Same redirect rules as the DWG path apply (pipe stderr to /dev/null
or NUL to silence).

## R41 SVG compression

`.svgz` extensions auto-enable gzip compression.  Pass a path ending in
`.svgz`, or set `"compress_gzip": true` in the options JSON, to produce
Inkscape / browser-friendly compressed SVG (typical compression ratio
5-8× on engineering drawings).

```
h7cad drawing.dxf --export-svg out.svgz
h7cad drawing.dxf --export-svg out.svg --options '{"compress_gzip":true}'
```

## R40/R41 ColorPolicy three-way

PDF and SVG share `crate::io::color_policy::ColorPolicy { Full,
Monochrome, Grayscale }`.  Default is `Monochrome` for both.  Override
via JSON:

```json
{ "color_policy": "full"      }
{ "color_policy": "monochrome" }
{ "color_policy": "grayscale" }
```

The legacy `monochrome` bool is honoured when `color_policy` is left at
its default; see `docs/svg_export.md` and `docs/pdf_export.md` for the
full conflict-resolution table.

## Defaults

The headless path uses each exporter's `Default` impl, which matches
the GUI dialog defaults:

- **Color**: `Monochrome` (every stroke forced to black)
- **Curves**: native `<circle>` / `<ellipse>` / `<path A>` (SVG) /
  native bezier paths (PDF)
- **Splines**: native (degree-1 control polyline / degree-2/3 Bezier /
  fit-point fallback)
- **Hatches**: solid + pattern + gradient (R32 / R33 / R39)
- **Text**: native `<text>` (SVG) / Standard 14 PDF font (PDF)
- **Images**: embedded as base64 / XObject

Override any of these via `--options <PATH>`; see the field tables in
`docs/svg_export.md` and `docs/pdf_export.md`.

## Future flags

R43+ candidates (not yet implemented):

- SVG `<pattern>` element optimisation (R43-A2; needs a PDF pattern
  parity follow-up first — R43-A polyline path is already shipped on
  both PDF and SVG)
- Rational NURBS export (degree-2/3 + non-unit weights) (R43-B)
- OLE2_FRAME entity rendering (R43-C)
- Real `.dwg` end-to-end integration tests with fixtures (R44-D; PID
  end-to-end is shipped in R44-C with a synthesised CFB fixture, and
  multi-format batch coverage is shipped in R44-E)
- Paper Space multi-viewport composition (`setupActiveLayoutViews`
  parity) (R45)
- Hoist the synthesised PID fixture into `h7cad-native-testkit` for
  cross-crate sharing (R46; needed once R44-D arrives)
