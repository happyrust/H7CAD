# User Testing

## Validation Surface

### DWG parser cargo surface
- Mission scope is cargo-only and parser-only: validate `h7cad-native-dwg`, `h7cad-native-model`, `h7cad-native-facade`, and mission-relevant boundary files through cargo tests, compile checks, and source inspection.
- Primary validation path is sequential:
  - `cargo check -p h7cad-native-dwg`
  - `cargo test -p h7cad-native-dwg -- --test-threads=1`
  - `cargo test -p h7cad-native-dwg --test real_samples -- --nocapture --test-threads=1`
  - `cargo test -p h7cad-native-facade -- --test-threads=1`
  - `cargo check -p h7cad-native-facade`
  - `cargo check -p H7CAD` when a feature explicitly widens compile-surface verification or the orchestrator requests an app-level boundary check
- Use focused named tests during implementation, but milestone validation must show the commands required by the feature’s milestone and contract assertions.
- Use ACadSharp samples from `D:/work/plant-code/cad/ACadSharp/samples` for real-sample assertions, especially `sample_AC1015.dwg`.
- No GUI/browser/desktop automation is part of this mission.

## Validation Readiness

- Dry run confirmed the cargo validation path is executable in the current environment.
- Confirmed dry-run commands passed:
  - `cargo check -p h7cad-native-dwg`
  - `cargo test -p h7cad-native-dwg --test read_headers -- --test-threads=1`
  - `cargo check -p h7cad-native-facade`
  - `cargo test -p h7cad-native-dwg --test real_samples -- --nocapture --test-threads=1`
- The AC1015 real sample is locally available and currently produces the mission-start baseline of `84` recovered entities.
- PowerShell formatting can be noisy on Windows; validators should trust exit codes and captured output.

## Validation Concurrency

### DWG parser cargo surface
- Max concurrent validators: 1
- Rationale: the machine has ample headroom (32 logical processors, ~63 GB RAM), but this mission explicitly prefers deterministic single-lane validation because `real_samples` evidence is easier to audit sequentially and the dry run showed no need for parallelism.

## Accepted Limitations

- Do not start the H7CAD desktop app.
- Do not treat GUI rendering as required evidence.
- Do not change `acadrust` directly.
- Do not interpret parser progress as runtime DWG rollout.
- Source inspection is an accepted evidence type for the `src/io/mod.rs` compat-boundary assertions.

## Flow Validator Guidance: DWG parser cargo surface
- Operate only through cargo commands and read-only source inspection in `D:/work/plant-code/cad/H7CAD`.
- Do not edit source files or mission metadata while validating.
- Prefer evidence from assertion-named tests, command output, and sample-based regressions.
- For recovery assertions, capture both total/per-family recovery output and any named failure diagnostics.
- For INSERT assertions, require proof on the AC1015 object decode path or a mission-approved AC1015 object-slice fixture; semantic `ENT:INSERT` fixture coverage alone is insufficient.
- For boundary assertions, verify facade fail-closed tests plus `src/io/mod.rs` source evidence.

