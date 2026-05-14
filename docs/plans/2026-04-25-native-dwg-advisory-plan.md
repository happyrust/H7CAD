# Native DWG Advisory Runtime Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Surface `h7cad-native-dwg` runtime diagnostics while keeping `acadrust` as the DWG open/save backend.

**Architecture:** The current DWG production path remains `acadrust::DwgReader` / `DwgWriter`. `h7cad-native-dwg` is added as a non-blocking advisory pass during DWG open so its version support and AC1015 parser state can be shown as `OpenNotice`s without changing the loaded document.

**Tech Stack:** Rust 2021, `acadrust`, `h7cad-native-dwg`, `h7cad-native-model`, `cargo test`.

---

## 1. Current State

`H7CAD` already opens and saves DWG through `src/io/mod.rs`:

- Read: `.dwg` -> `DwgReader::from_file(path)` -> `reader.read()` -> `native_bridge::acadrust_doc_to_native`.
- Save: `save_native` -> `save_dwg` -> `native_bridge::native_doc_to_acadrust` -> `DwgWriter::write_to_file`.
- Diagnostics: `acadrust` notifications are converted by `diagnostics::from_acadrust_notifications`.

The workspace also contains `crates/h7cad-native-dwg`, but the root package does not depend on it. `crates/h7cad-native-facade` still returns `native DWG reader not implemented yet` for DWG load/save, so the native crate is not part of the GUI/CLI runtime.

## 2. Proposed Next Step

Add a small advisory bridge:

1. Root `H7CAD` depends on `h7cad-native-dwg`.
2. DWG open keeps using `acadrust` for the actual document.
3. After a successful `acadrust` read, read the DWG bytes and ask `h7cad-native-dwg` for an advisory result.
4. Convert advisory outcomes into `OpenNotice`s:
   - AC1015 parse success: no user warning by default.
   - AC1015 parse failure: `Warning`, because the main `acadrust` open already succeeded.
   - Non-AC1015 known DWG version: `NotSupported`, because the native advisory parser is not ready for that version.
   - Invalid/truncated advisory bytes: `Warning`, but never fail the main open.
5. Keep DWG save untouched; no native DWG writer is introduced.

This closes the previously documented "native-dwg advisory into runtime" gap without risking a backend swap.

## 3. Out Of Scope

- Replacing `acadrust::DwgReader`.
- Adding a native DWG writer.
- AC1018+ native parsing.
- Changing save-version selection.
- Changing the `h7cad-native-facade` contract.

## 4. Task Plan

### Task 1: Add Advisory Tests

**Files:**
- Modify: `src/io/mod.rs`

**Step 1: Write failing tests**

Add unit tests for a private helper with the intended shape:

```rust
#[test]
fn native_dwg_advisory_reports_non_ac1015_as_not_supported() {
    let notices = native_dwg_advisory_notices(b"AC1021rest");
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].severity, NoticeSeverity::NotSupported);
    assert!(notices[0].message.contains("AC1021"));
}

#[test]
fn native_dwg_advisory_reports_ac1015_structural_failure_as_warning() {
    let notices = native_dwg_advisory_notices(b"AC1015");
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].severity, NoticeSeverity::Warning);
    assert!(notices[0].message.contains("native DWG advisory"));
}
```

**Step 2: Verify RED**

Run:

```bash
cargo test --bin H7CAD native_dwg_advisory -- --nocapture
```

Expected: compile failure because `native_dwg_advisory_notices` does not exist yet.

### Task 2: Add Minimal Advisory Implementation

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/io/mod.rs`

**Step 1: Add dependency**

```toml
h7cad-native-dwg = { path = "crates/h7cad-native-dwg" }
```

**Step 2: Implement helper**

Add a private function near the DWG load path:

```rust
fn native_dwg_advisory_notices(bytes: &[u8]) -> Vec<OpenNotice> {
    // Use h7cad-native-dwg only as advisory. Never fail the main acadrust open.
}
```

Behavior:

- `sniff_version(bytes)` returns AC1015 -> call `read_dwg(bytes)`.
- AC1015 `read_dwg` success -> `Vec::new()`.
- AC1015 `read_dwg` error -> one `Warning`.
- Any other known version -> one `NotSupported`.
- Invalid/truncated magic -> one `Warning`.

**Step 3: Wire into DWG open**

In the `.dwg` branch of `load_file_native_blocking`, after `reader.read()`:

```rust
let mut notices = diagnostics::from_acadrust_notifications(&acad_doc.notifications);
if let Ok(bytes) = std::fs::read(path) {
    notices.extend(native_dwg_advisory_notices(&bytes));
}
```

Do not make filesystem advisory read failures fatal.

**Step 4: Verify GREEN**

Run:

```bash
cargo test --bin H7CAD native_dwg_advisory -- --nocapture
```

Expected: both advisory tests pass.

### Task 3: Focused Regression

**Files:**
- Modify only if tests expose a real issue: `src/io/mod.rs`, `src/io/diagnostics.rs`

Run:

```bash
cargo test --bin H7CAD io:: -- --nocapture
cargo check -p H7CAD
```

Expected: pass with no new warnings.

## 5. Risks

- Advisory notices may add noise for every non-AC1015 DWG. If that is too chatty, gate `NotSupported` notices behind AC1015 only and keep unsupported versions silent.
- Reading the file twice adds overhead on large drawings. The advisory pass can later be made optional or reused from a byte-buffered DWG open path.
- Native advisory failures must never replace `acadrust` errors; they are secondary context only.

## 6. Completion Criteria

- Plan file exists.
- Root package can reference `h7cad-native-dwg`.
- DWG advisory helper is covered by RED/GREEN tests.
- Existing DWG open/save behavior remains routed through `acadrust`.
- Targeted tests pass.
