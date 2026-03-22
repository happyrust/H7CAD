# Flatpak Packaging

## App ID
`io.github.HakanSeven12.H7CAD`

## Requirements

```bash
# flatpak-builder
sudo apt install flatpak-builder

# Required runtime and SDK
flatpak install flathub org.freedesktop.Platform//24.08
flatpak install flathub org.freedesktop.Sdk//24.08
flatpak install flathub org.freedesktop.Sdk.Extension.rust-stable//24.08
```

## Generating cargo-sources.json

Flathub builds run without internet access, so all Cargo dependencies must be
pre-downloaded:

```bash
# Download flatpak-cargo-generator.py
git clone https://github.com/flatpak/flatpak-builder-tools.git
cd flatpak-builder-tools/cargo

# Generate cargo-sources.json (run from the H7CAD repo root)
python3 flatpak-cargo-generator.py /path/to/H7CAD/Cargo.lock -o /path/to/H7CAD/flatpak/cargo-sources.json
```

## Local Testing

```bash
# Run from the flatpak/ directory
flatpak-builder --force-clean --user --install ../build-dir io.github.HakanSeven12.H7CAD.local.yml
flatpak run io.github.HakanSeven12.H7CAD
```

## Submitting to Flathub

1. Fork https://github.com/flathub/flathub
2. Create a new repository named `io.github.HakanSeven12.H7CAD` from the `new-pr` branch
3. Copy the files from this directory + `cargo-sources.json` into the new repo
4. Fill in the `commit:` field in the manifest with the actual commit hash
5. Follow the Flathub submission guide:
   https://docs.flathub.org/docs/for-app-authors/submission

## Notes

- A GitHub release/tag `v0.1.0` must exist before submitting to Flathub
- Get the commit hash with `git rev-parse v0.1.0` and replace `FILL_IN_COMMIT_HASH` in the manifest
- Validate the manifest with `flatpak-builder-lint` before opening a Flathub PR
