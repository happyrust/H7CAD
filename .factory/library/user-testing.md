# User Testing

## Validation Surface

### DWG parser cargo surface
- Mission scope is parser-only: validate `crates/h7cad-native-dwg` behavior through cargo-driven tests and compile checks.
- Primary validation path is sequential:
  - `cargo check -p h7cad-native-dwg`
  - `cargo test -p h7cad-native-dwg`
  - `cargo check -p h7cad-native-facade`
- Use synthetic DWG fixtures first. Add selective real DWG samples only at milestone gates where synthetic data cannot cover the target behavior.
- The current desktop app DWG path in `src/io` remains on `acadrust`; validators should not treat UI opening of DWG files as part of this mission's done criteria.

## Validation Readiness

- Dry run confirmed the cargo-based parser validation path is executable in the current environment.
- Existing parser skeleton and current test baseline run without requiring new services, ports, credentials, or desktop automation setup.
- Resource demand was reported as low-to-moderate during the dry run, but the user explicitly selected sequential validation for this mission.

## Validation Concurrency

### DWG parser cargo surface
- Max concurrent validators: 1
- Rationale: user-directed sequential validation strategy, parser-only scope, and shared fixture/test state make deterministic single-lane execution preferable to parallel cargo jobs for this mission.

## Accepted Limitations

- Do not validate or change the desktop DWG open path in `src/io` during this mission.
- `h7cad-native-facade` is only a compile-surface check unless a later milestone explicitly expands scope.
- Real DWG fixtures are selective milestone-gate evidence, not a requirement for every feature.
- Current uncommitted DWG parser skeleton is baseline mission context and should be extended rather than restarted.
