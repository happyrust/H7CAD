# Environment

Environment variables, external dependencies, and setup notes for the DWG parser mission.

**What belongs here:** required env vars, external dependencies, platform notes, and setup constraints.
**What does NOT belong here:** service commands/ports (use `.factory/services.yaml`) and feature/task history.

---

## Mission Environment

- No new services, ports, databases, or credentials are required.
- Validation is cargo-only and runs inside the existing Rust workspace.
- The current uncommitted `h7cad-native-dwg` skeleton is the baseline for this mission; workers should extend it rather than restart from scratch.

## Platform Notes

- The working environment is Windows.
- Workers should prefer commands from `.factory/services.yaml`.
- If `.factory/init.sh` cannot be executed in the shell environment, workers may continue with direct cargo commands because no extra environment bootstrap is required.

## External Dependency Notes

- `h7cad-native-facade` is a compile-surface consumer only for this mission.
- The desktop/runtime DWG loading path remains unchanged and out of scope.
- Selective real DWG fixtures are allowed only at milestone gates when synthetic fixtures cannot prove the target behavior.
