# Environment

Environment variables, external dependencies, and setup notes for the `PLAN-4-18` DWG mission.

**What belongs here:** sample-data locations, platform notes, workspace constraints, and external-dependency facts.  
**What does NOT belong here:** service commands/ports (use `.factory/services.yaml`) and feature history.

---

## Mission Environment

- No new services, ports, databases, or credentials are required.
- Validation is cargo-only and runs inside the existing Rust workspace.
- ACadSharp DWG/DXF samples are available at `D:/work/plant-code/cad/ACadSharp/samples`.
- The repository already contains unrelated in-progress changes; workers must only modify files required by their assigned feature and must not revert unrelated work.
- The mission is intentionally based on the current dirty worktree rather than a fresh checkout.

## Platform Notes

- The working environment is Windows.
- Prefer commands from `.factory/services.yaml`.
- If `.factory/init.sh` cannot be executed in the shell environment, workers may continue with direct cargo commands because no extra bootstrap is required.
- PowerShell formatting can be noisy on Windows; trust command exit codes plus captured output.

## Dependency / Scope Notes

- `acadrust` is an external dependency and must not be modified directly.
- `crates/h7cad-native-dwg` is the primary implementation surface for this mission.
- `crates/h7cad-native-model` is in scope where needed for resolved-document/INSERT behavior.
- `crates/h7cad-native-facade` is only in scope as a compile/fail-closed boundary.
- `src/io/mod.rs` is off-limits for rollout changes; validators may inspect it to confirm the compat DWG runtime path is unchanged.
- The desktop/runtime GUI path must not be launched as part of mission validation.
- AC1018+ parsing, DWG writer work, and runtime DWG rollout are out of scope.
