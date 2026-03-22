# H7CAD

<img width="1920" height="940" alt="resim" src="https://github.com/user-attachments/assets/25bc2bb2-c35c-477d-a3e9-45c86690b5c9" />

A CAD application for 2D/3D drawing and design, built with Rust.

## Features

- 2D drafting and 3D modeling
- DXF file import/export
- GPU-accelerated rendering via WebGPU
- Snap and annotation tools
- Modular ribbon interface (Home, Annotate, Insert, View, Manage)

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
