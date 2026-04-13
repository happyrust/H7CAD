# User Testing

## Validation Surface

### DXF native cargo surface
- Mission scope is cargo-only: validate `h7cad-native-dxf`, `h7cad-native-model`, `h7cad-native-facade`, `src/io/native_bridge.rs`, and the quick-win runtime migration files through cargo tests and compile checks.
- Primary validation path is sequential:
  - `cargo check -p h7cad-native-dxf -p h7cad-native-model -p h7cad-native-facade`
  - `cargo test -p h7cad-native-dxf -p h7cad-native-model -p h7cad-native-facade -- --test-threads=16`
  - `cargo check`
  - `cargo test -- --test-threads=16` for milestone 4 or when a feature touches runtime files under `src/`
- Prefer focused test filters during implementation, but milestone validation must show the full command set required by the feature’s milestone.
- Use ACadSharp sample DXF files from `D:/work/plant-code/cad/ACadSharp/samples` for real-sample assertions.
- No GUI/browser/desktop automation is part of this mission.

## Validation Readiness

- Dry run confirmed the cargo validation path is executable in the current environment.
- Existing baselines already pass:
  - `cargo test -p h7cad-native-dxf` (56 tests)
  - `cargo test -p h7cad-native-model` (9 tests)
  - `cargo test -p h7cad-native-facade` (1 test)
- Sample DXF fixtures are locally available and do not require network access.
- PowerShell formatting can be noisy on Windows; validators should trust exit codes and captured output.

## Validation Concurrency

### DXF native cargo surface
- Max concurrent validators: 1
- Rationale: the user explicitly requested sequential cargo-only validation. The machine has ample headroom (64 GB RAM, 32 logical processors), but deterministic single-lane execution is preferred for this mission.

## Accepted Limitations

- Do not start the H7CAD desktop app.
- Do not treat GUI rendering as required evidence.
- Do not change `acadrust` directly.
- DWG parser milestones and runtime DWG rollout remain out of scope.

## Flow Validator Guidance: DXF native cargo surface
- Operate only through cargo commands in `D:/work/plant-code/cad/H7CAD`.
- Do not edit source files or mission metadata while validating.
- Prefer evidence from assertion-named tests, command output, and sample-based regressions.
- For pipeline assertions, verify supported-entity preservation, handle resolvability, ownership validity, and explicit accounting for unsupported entities where relevant.

