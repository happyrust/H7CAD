# User Testing

## Validation Surface

### DWG parser cargo surface
- Mission scope is parser-only: validate `crates/h7cad-native-dwg` behavior through cargo-driven tests and compile checks.
- Primary validation path is sequential:
  - `cargo check -p h7cad-native-dwg`
  - `cargo test -p h7cad-native-dwg`
  - `cargo check -p h7cad-native-facade`
- For targeted assertion evidence on integration tests, prefer `cargo test -p h7cad-native-dwg --test read_headers -- --test-threads=1` or a single test-name filter; cargo does not support multiple positional TESTNAME filters in one invocation.
- Use synthetic DWG fixtures first. Add selective real DWG samples only at milestone gates where synthetic data cannot cover the target behavior.
- For this semantic mission, prioritize paired fixtures that vary section order, payload packing, embedded-zero placement, layout/block ownership, and invalid owner/block/layout references while keeping decoded meaning explicit.
- When validating resolved behavior, distinguish parser-emitted records from resolver-seeded scaffold records explicitly in the test evidence.
- The current desktop app DWG path in `src/io` remains on `acadrust`; validators should not treat UI opening of DWG files as part of this mission's done criteria.

## Validation Readiness

- Dry run confirmed the cargo-based parser validation path is executable in the current environment.
- Existing parser skeleton and current test baseline run without requiring new services, ports, credentials, or desktop automation setup.
- Resource demand was reported as low-to-moderate during the dry run, but the user explicitly selected sequential validation for this mission.
- On this Windows host, PowerShell `Tee-Object` output can show noisy `RemoteException` formatting even when cargo commands succeed; validators should trust command exit codes plus saved evidence logs over console formatting alone.

## Validation Concurrency

### DWG parser cargo surface
- Max concurrent validators: 1
- Rationale: user-directed sequential validation strategy, parser-only scope, and shared fixture/test state make deterministic single-lane execution preferable to parallel cargo jobs for this mission.

## Accepted Limitations

- Do not validate or change the desktop DWG open path in `src/io` during this mission.
- `h7cad-native-facade` is only a compile-surface check unless a later milestone explicitly expands scope.
- Real DWG fixtures are selective milestone-gate evidence, not a requirement for every feature.
- Current uncommitted DWG parser skeleton is baseline mission context and should be extended rather than restarted.

## Flow Validator Guidance: DWG parser cargo surface
- Operate only through cargo commands in the shared repository at D:/work/plant-code/cad/H7CAD.
- Do not edit source files or mission metadata while validating.
- Use sequential cargo execution only; do not start concurrent cargo jobs or background services.
- Evidence should come from command output and, when useful, captured logs saved under the assigned evidence directory; treat exit codes and saved logs as authoritative if PowerShell formatting is noisy.
- Stay within parser-only scope: validate h7cad-native-dwg and h7cad-native-facade compile surface only.
- Prefer assertion evidence that references `PendingDocument` projections, parser provenance tuples, and outward-facing resolved projections instead of helper-only/internal summaries.

