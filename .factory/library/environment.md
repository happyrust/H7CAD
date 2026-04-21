# Environment

Environment variables, external dependencies, and setup notes for the PID real-sample display and screenshot mission.

**What belongs here:** sample-data locations, platform notes, workspace constraints, and external dependency facts.  
**What does NOT belong here:** service commands/ports (use `.factory/services.yaml`) and feature history.

---

## Mission Environment

- No new services, ports, databases, or credentials are required.
- Validation runs inside the existing Rust workspace with local files only.
- The primary acceptance sample is:
  - `D:\\work\\plant-code\\cad\\pid-parse\\test-file\\工艺管道及仪表流程-1.pid`
- The app depends on sibling-path `pid-parse` via `Cargo.toml -> pid-parse = { path = \"../pid-parse\" }`.
- The repository already contains unrelated in-progress changes; workers must only modify files required by their assigned PID feature and must not revert unrelated work.
- The mission is intentionally based on the current dirty worktree rather than a fresh checkout.

## Platform Notes

- The working environment is Windows.
- Prefer commands from `.factory/services.yaml`.
- If `.factory/init.sh` cannot be executed in the shell environment, workers may continue with direct cargo commands because no extra bootstrap is required beyond `cargo fetch --locked`.
- PowerShell formatting can be noisy on Windows; trust exit codes plus captured output.

## Dependency / Scope Notes

- `pid-parse` is a local sibling dependency and part of the effective runtime truth for PID ingestion.
- `image` is already present in `Cargo.toml` and is the preferred existing dependency for deterministic PNG export.
- `acadrust` remains in the repo runtime surface but is not the primary PID truth source for this mission.
- `src/io/pid_import.rs`, `src/app/update.rs`, `src/app/commands.rs`, `src/io/pid_screenshot.rs`, and directly related scene code are in scope.
- Unrelated DWG/DXF work is out of scope unless directly required to keep PID-related changes compiling.
- UI automation is desired as a second-layer validation path, but deterministic PNG regression is the mandatory first-layer proof.
