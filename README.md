# H7CAD

<img width="1920" height="940" alt="resim" src="https://github.com/user-attachments/assets/e80191a4-d14c-4b3e-ae72-3b1a7c0be418" />

A CAD application for 2D/3D drawing and design, built with Rust.

## Features

- 2D drafting and 3D modeling
- DXF file import/export
- Native DWG parser work is in progress under `crates/h7cad-native-dwg`
- GPU-accelerated rendering via WebGPU
- Snap and annotation tools
- Modular ribbon interface (Home, Annotate, Insert, View, Manage)

## Architecture (developers)

- [docs/README.md](docs/README.md) — documentation index.
- [docs/ARCHITECTURE-TUTORIAL.md](docs/ARCHITECTURE-TUTORIAL.md) — layered overview, startup split, and links to SVG/HTML diagrams under `docs/diagrams/`.
- [docs/DEVELOPMENT-PLAN.md](docs/DEVELOPMENT-PLAN.md) — phased roadmap (native path hardening, DWG runtime, QA).

## Native DWG Status

### Read

The native DWG reader (`crates/h7cad-native-dwg`) is wired into both the facade
and the desktop runtime.

- **AC1015 (R2000)** — full decode pipeline: file header → section map →
  handle map → pending document → resolved `CadDocument` → real entity
  enrichment. The `sample_AC1015.dwg` baseline currently recovers 238
  entities across LINE / CIRCLE / ARC / POINT / TEXT / LWPOLYLINE /
  HATCH and other supported families, with ratchets guarding the core
  families individually.
- **AC1018 (R2004)** — full decode pipeline: encrypted metadata → page
  map → section descriptor map → LZ77 section payload → AC1015-style
  bridge → reuses the AC1015 downstream. The `sample_AC1018.dwg`
  baseline currently recovers 11 entities (CIRCLE / HATCH / INSERT /
  VIEWPORT), 2 block records, 2 layouts, and 281 objects.
- **AC1012 / AC1014 / AC1021 / AC1024 / AC1027 / AC1032** — the version
  is sniffed but native parsing is still fail-closed for these layouts
  (currently surfaced as explicit unsupported-version/header-layout
  errors depending on the path). The runtime falls back to
  `acadrust::DwgReader` automatically; users see an `OpenNotice::Warning`
  describing the fallback.

Supported entity families on the AC1015/AC1018 native path: TEXT, ATTRIB,
ATTDEF, INSERT, ARC, CIRCLE, LINE, DIMENSION (Ordinate / Linear /
Aligned / Ang3pt / Ang2ln / Radius / Diameter), POINT, FACE3D, SOLID,
VIEWPORT, ELLIPSE, SPLINE, RAY, XLINE, MTEXT, LWPOLYLINE, HATCH.

### Write

The product save path still uses acadrust, but the native crate now has an
early AC1015 writer tracer bullet.

- `h7cad_native_facade::save(NativeFormat::Dwg, _)` deliberately returns
  `Err("native DWG writer not implemented yet")`.
- `h7cad_native_dwg::write_dwg` can emit AC1015/R2000 empty documents plus
  LINE, CIRCLE, ARC, POINT, LWPOLYLINE, TEXT, and ATTRIB documents that round-trip through the native reader.
  Other entity types still return `DwgWriteError::Unsupported(... pending F5.M5)`,
  and AC1018 writing is not implemented.
- The desktop runtime's `save_dwg` keeps routing through
  `acadrust::DwgWriter`, with the output version honestly carried from
  `doc.header.version` (no silent downgrade). See
  `docs/plans/2026-04-21-dwg-save-version-honesty-plan.md`.

Expanding the native writer beyond the LINE/CIRCLE/ARC/POINT/LWPOLYLINE/TEXT/ATTRIB tracer bullets and deciding
when to expose it through facade/runtime remain the largest open items; the
staged plan lives in `docs/plans/2026-05-08-dwg-next-step-plan.md` §F5.

### Useful commands

```bash
cargo test -p h7cad-native-dwg -- --test-threads=1
cargo check -p h7cad-native-facade
cargo test --workspace --all-targets
```

## Installation

### Flatpak (Linux)

Download `H7CAD.flatpak` from the [latest release](https://github.com/HakanSeven12/H7CAD/releases/latest), then:

```bash
flatpak remote-add --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
flatpak install H7CAD.flatpak
flatpak run io.github.HakanSeven12.H7CAD
```

### Build from Source

Requirements: Rust 1.75+

```bash
git clone https://github.com/HakanSeven12/H7CAD.git
cd H7CAD
cargo build --release
./target/release/H7CAD
```

## License

GPL-3.0-only — see [LICENSE](LICENSE)
