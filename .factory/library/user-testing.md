# User Testing

## Validation Surface

### PID deterministic cargo/test surface
- This mission’s required validation surface is local Rust tests plus compile checks against the H7CAD workspace.
- Primary validation path is sequential and PID-focused:
  - `cargo test open_target_pid_sample_builds_dense_preview -- --nocapture`
  - `cargo test target_pid_preview_layout_is_primary_visual_focus -- --nocapture`
  - `cargo test target_pid_sample_fit_layers_matching_succeeds_for_main_drawing_layers -- --nocapture`
  - `cargo test target_pid_sample_scene_has_fittable_geometry_and_native_doc -- --nocapture`
  - `cargo test export_pid_preview_png_writes_file -- --nocapture`
  - `cargo test target_pid_sample_screenshot_matches_baseline -- --nocapture`
  - `cargo test pid_import -- --nocapture`
  - `cargo test pid_screenshot -- --nocapture`
  - `cargo check`
- Use focused named tests during implementation, but milestone validation must show the commands required by the feature’s milestone and contract assertions.
- The primary acceptance sample is `D:\\work\\plant-code\\cad\\pid-parse\\test-file\\工艺管道及仪表流程-1.pid`.

### UI-level confirmation surface
- Preferred second-layer validation surface: real app-style open of the approved sample followed by screenshot export confirmation.
- This surface is desired but secondary to deterministic PNG regression.
- If no reliable automation harness exists, validators must record a concrete blocker rather than silently skip.

## Validation Readiness

- The local cargo validation path is executable in the current environment.
- The approved PID sample path exists and is already referenced by current target-sample tests in `src/io/pid_import.rs`.
- `src/io/pid_screenshot.rs` already exists and includes deterministic PNG export plus regression-oriented tests.
- `src/app/commands.rs` already includes a `PIDSHOT` command branch.
- `src/app/update.rs` already includes PID-specific main-layer-first fit behavior in the open flow.
- PowerShell formatting can be noisy on Windows; validators should trust exit codes and captured output.

## Validation Concurrency

### PID deterministic cargo/test surface
- Max concurrent validators: 1
- Rationale: the machine has ample resources (~67.8 GB RAM, 32 logical processors), but screenshot-sensitive validation is intentionally sequential to preserve deterministic baseline behavior and simplify evidence review.

### UI-level confirmation surface
- Max concurrent validators: 1
- Rationale: any app-style confirmation should run as a single-instance flow to avoid viewport/export instability and sample-path conflicts.

## Accepted Limitations

- Deterministic PNG regression is the required primary evidence; UI automation is a second layer and may return a blocker if the surface is not yet reliably automatable.
- Workers/validators must not broaden scope into unrelated DWG/DXF or generic rendering redesign.
- Source inspection is acceptable supporting evidence for command-path and open-path boundary assertions, but not a replacement for required screenshot/open regression tests.

## Flow Validator Guidance: PID deterministic cargo/test surface
- Operate through cargo commands and read-only source inspection in `D:/work/plant-code/cad/H7CAD`.
- Do not edit source files or mission metadata while validating.
- Prefer evidence from assertion-named tests, command output, and target-sample regressions.
- For display assertions, require target-sample evidence proving the main drawing remains the visual focus and the scene is fittable.
- For screenshot assertions, require evidence from `PIDSHOT`/`pid_screenshot` tests showing non-empty PNG output and baseline/tolerance checks.
- For cross-area assertions, require combined evidence from open/display tests and screenshot regression tests.

## Flow Validator Guidance: UI-level confirmation surface
- Attempt only after deterministic screenshot validation is already green.
- Prefer the narrowest user-style flow that proves: open approved sample -> trigger screenshot export -> confirm PNG created.
- If the environment lacks a reliable automation harness, return a blocker with exact missing capability/tooling rather than marking the assertion passed.

