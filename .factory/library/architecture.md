# Architecture

High-level architecture for the PID real-sample display and screenshot mission.

This document is worker-facing guidance for how the approved PID sample moves through the system, where this mission is allowed to change behavior, and which seams are the main truth surfaces. It is intentionally high-level; use the mission validation contract for the exact observable definition of done.

## Mission Scope

This mission is intentionally bounded to the PID runtime lane for:

- opening the approved sample PID file
- building a credible preview document
- fitting and presenting that preview inside the H7CAD app
- exporting a deterministic PNG screenshot
- validating the result through targeted tests and regression checks

Primary implementation surfaces:

- `src/io/pid_import.rs`
- `src/io/pid_screenshot.rs`
- `src/io/mod.rs`
- `src/app/update.rs`
- `src/app/commands.rs`
- `src/scene/*` only where directly needed for PID fit/display correctness

Out of scope:

- unrelated DWG parser work
- unrelated DXF writer/header work
- broad generic rendering redesign
- generic screenshot/export framework beyond what PID needs
- GUI editor work for PID metadata

## Current System Shape

There are four relevant layers for this mission:

1. **PID package ingest layer**  
   `src/io/pid_import.rs::open_pid(path)` parses the `.pid`, merges publish sidecars when present, derives layout, builds a `PidOpenBundle`, and caches the raw `PidPackage` for later PID command usage.

2. **Preview construction layer**  
   `pid_document_to_bundle(...)` / related PID preview helpers turn parsed PID/layout truth into:
   - `pid_doc`
   - `native_preview: h7cad_native_model::CadDocument`
   - preview index and summary data

3. **Runtime tab/display layer**  
   `src/app/update.rs::Message::FileOpened` receives `OpenedDocument::Pid(bundle)`, installs PID tab state, sets the compat/native preview into the scene, and fits the view. This is the user-visible open path.

4. **Deterministic screenshot layer**  
   `src/io/pid_screenshot.rs` rasterises the PID preview document to a PNG without GPU readback so it remains stable in tests and headless environments. `src/app/commands.rs` exposes this through `PIDSHOT`.

## Canonical PID Flow

### Open flow
`pick/open path -> io::open_path -> pid_import::open_pid -> PidOpenBundle -> Message::FileOpened -> PID tab + scene preview + fit`

This is the core user-visible path that must remain intact and become more reliable for the approved sample.

### Preview truth flow
`PidPackage -> parsed PidDocument -> derive_layout -> preview construction -> native_preview entities/layers`

This is where display fidelity lives. If the approved sample looks sparse, visually wrong, or dominated by decorative panels, the root cause is usually here.

### Scene-fit flow
`native_preview + compat projection -> scene.set_native_doc(...) -> fit primary PID layers first -> fallback fit_all only when needed`

This mission must preserve the notion that the main drawing should dominate the viewport over distant decorative layers.

### Screenshot/export flow
`active PID tab -> PIDSHOT command -> deterministic PNG export -> regression checks`

This is the canonical non-GPU confirmation path for the mission.

## Mission Focus Areas

### 1. Approved sample fidelity
The mission is anchored to the real sample:

- `D:\\work\\plant-code\\cad\\pid-parse\\test-file\\工艺管道及仪表流程-1.pid`

Completion is not generic “PID support exists”; it is specifically that this sample:

- opens
- produces a credible preview
- is visually centered around the main drawing
- remains exportable and regressible

### 2. Main drawing visual priority
The current PID preview can include decorative layers such as metadata, fallback, cross-ref, unresolved, streams, clusters, and symbols. These are useful diagnostics but must not overpower the main drawing in the user-visible viewport.

### 3. Deterministic screenshot evidence
This mission prefers a deterministic export pipeline over GPU/window capture as the primary truth source. The PNG output must be stable enough for regression testing and useful enough for human confirmation.

### 4. UI confirmation as a second layer
After deterministic export is solid, the mission should attempt a second-layer confirmation path closer to real user behavior. If the environment lacks a reliable automation harness, workers must surface a concrete blocker rather than silently skipping it.

## Canonical Observable Surfaces

Workers should treat these as the authoritative observable surfaces:

- **Open/parse truth:** `src/io/pid_import.rs`
- **Runtime open/tab truth:** `src/app/update.rs`
- **PID command truth:** `src/app/commands.rs`
- **Deterministic screenshot truth:** `src/io/pid_screenshot.rs`
- **Scene fit/display truth:** `src/scene/*`

If a change cannot be observed through these surfaces plus targeted tests, it is not complete.

## Invariants To Preserve

- The approved sample path remains the primary acceptance target.
- PID open must continue through the normal `open_path` / `Message::FileOpened` workflow.
- Decorative PID layers must not silently displace the main drawing from the user-visible fit.
- `PIDSHOT` remains deterministic enough for regression use.
- Screenshot/export work must stay PID-scoped; no broad screenshot platform redesign.
- Unrelated dirty-tree work must not be reverted or reformatted.

## Risk Concentration

### `src/io/pid_import.rs`
Highest-risk seam for:

- sample parsing and sidecar merge behavior
- layout derivation
- preview density / layer balance
- target-sample regression tests

### `src/app/update.rs`
Highest-risk seam for:

- PID tab activation
- scene state installation
- main-layer-first fit behavior

### `src/io/pid_screenshot.rs`
Highest-risk seam for:

- deterministic PNG generation
- baseline stability
- helper-level screenshot regression signal quality

### `src/app/commands.rs`
Highest-risk seam for:

- `PIDSHOT` command UX
- active-tab validation
- argument and output-path error handling

## Validation Surface Mapping

### Target-sample open/display regression
Use target-sample tests in `src/io/pid_import.rs` and app/open-path tests to verify:

- sample opens
- preview remains non-trivial
- main layers remain visually dominant
- scene fit remains usable

### Screenshot regression
Use `src/io/pid_screenshot.rs` helper tests to verify:

- PNG export succeeds
- exported image dimensions remain fixed
- image remains non-empty and within baseline/tolerance bounds

### Command-path checks
Use command-focused tests or command-path assertions to verify:

- `PIDSHOT` success from active PID tab
- clear errors for invalid context/arguments

### Optional UI confirmation
If a reliable automation surface exists, use it to prove the app can open the approved sample and trigger screenshot export through a real user-style flow.

## Relationship Summary

The architecture relationship for workers is:

`approved PID sample -> pid_import/open path -> PID preview document -> PID tab scene fit -> PIDSHOT deterministic PNG -> regression evidence`

Everything in this mission should strengthen or verify that chain.
