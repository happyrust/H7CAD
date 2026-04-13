# Environment

Environment variables, external dependencies, and setup notes for the DXF native migration mission.

**What belongs here:** sample-data locations, platform notes, workspace constraints, and external-dependency facts.
**What does NOT belong here:** service commands/ports (use `.factory/services.yaml`) and feature history.

---

## Mission Environment

- No new services, ports, databases, or credentials are required.
- Validation is cargo-only and runs inside the existing Rust workspace.
- ACadSharp DXF samples are available at `D:/work/plant-code/cad/ACadSharp/samples`.
- The repository already contains unrelated in-progress changes; workers must only modify files required by their assigned feature and must not revert unrelated work.

## Platform Notes

- The working environment is Windows.
- Prefer commands from `.factory/services.yaml`.
- If `.factory/init.sh` cannot be executed in the shell environment, workers may continue with direct cargo commands because no extra bootstrap is required.

## Dependency / Scope Notes

- `acadrust` is an external dependency and must not be modified directly.
- `h7cad-native-dwg` is out of scope for this mission except for keeping the workspace compiling.
- The desktop/runtime GUI path must not be launched as part of mission validation.
- DXF runtime currently flows through `src/io/mod.rs`: native DXF load/save plus compat-bridge runtime usage. That boundary is in scope.
