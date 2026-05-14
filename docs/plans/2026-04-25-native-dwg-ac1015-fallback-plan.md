# Native DWG AC1015 Fallback Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Let H7CAD open AC1015 DWG files with `h7cad-native-dwg` when `acadrust` rejects the file, without replacing the existing DWG backend.

**Architecture:** `acadrust` remains the primary DWG reader and the only DWG writer. The native fallback is attempted only after the primary reader fails and only through `h7cad-native-dwg::read_dwg`; successful fallback returns a `NativeCadDocument` plus a warning notice that records the primary failure.

**Tech Stack:** Rust 2021, `acadrust`, `h7cad-native-dwg`, `h7cad-native-model`, `cargo test`.

---

## 1. Current State

The previous plan (`2026-04-25-native-dwg-advisory-plan.md`) added runtime advisory notices. The open path still fails immediately when `acadrust::DwgReader` rejects a DWG.

`h7cad-native-dwg` can parse synthetic AC1015 fixtures and a partial set of real AC1015 entities. That is enough to offer a conservative fallback for AC1015 files while clearly warning users that the primary reader failed.

## 2. Proposed Behavior

For `.dwg` open:

1. Read file bytes once at the start so fallback has the same input.
2. Try the existing `acadrust` path.
3. If `acadrust` succeeds:
   - Convert to native as today.
   - Append native advisory notices as today.
4. If `acadrust` fails:
   - Attempt `h7cad-native-dwg::read_dwg(&bytes)`.
   - If native succeeds, return the native document with one `Warning` notice:
     `native DWG fallback opened file after acadrust failed: ...`
   - If native also fails, return the original classified `acadrust` error.

This makes fallback strictly additive: it never changes successful `acadrust` opens and never masks a double failure with a weaker native error.

## 3. Out Of Scope

- Native DWG writer.
- Native fallback for AC1018+.
- Replacing compat `CadDocument` with native-only UI state.
- Fallback after non-DWG extensions.
- Expanding entity coverage inside `h7cad-native-dwg`.

## 4. Task Plan

### Task 1: RED Test

**Files:**
- Modify: `src/io/mod.rs`

Add a unit test with a synthetic AC1015 fixture copied from the native DWG crate's public on-disk layout expectations:

```rust
#[test]
fn native_dwg_fallback_opens_ac1015_when_acadrust_rejects_fixture() {
    let path = write_temp_dwg("native-fallback", &synthetic_ac1015_fixture());
    let result = load_file_native_blocking(&path);
    let _ = std::fs::remove_file(&path);

    let (doc, notices) = result.expect("native fallback should open synthetic AC1015");
    assert_eq!(doc.header.version, h7cad_native_model::DxfVersion::R2000);
    assert!(notices.iter().any(|n| n.message.contains("native DWG fallback")));
}
```

Run:

```bash
cargo test --bin H7CAD native_dwg_fallback -- --nocapture
```

Expected before implementation: failure because the current path returns the primary `acadrust` error.

### Task 2: Minimal Fallback

**Files:**
- Modify: `src/io/mod.rs`

Extract the existing primary reader into a helper:

```rust
fn load_dwg_with_acadrust(path: &Path) -> Result<(NativeCadDocument, Vec<OpenNotice>), OpenError>
```

Then add:

```rust
fn load_dwg_native_blocking(path: &Path) -> Result<(NativeCadDocument, Vec<OpenNotice>), OpenError>
```

The `.dwg` branch of `load_file_native_blocking` delegates to that helper.

### Task 3: Verify

Run:

```bash
cargo test --bin H7CAD native_dwg_fallback -- --nocapture
cargo test --bin H7CAD native_dwg_advisory -- --nocapture
cargo test --bin H7CAD io:: -- --nocapture
RUSTFLAGS=-Dwarnings cargo check -p H7CAD
```

Expected: all pass.

## 5. Risk Controls

- Return the original primary error when fallback also fails.
- Emit an explicit warning when fallback succeeds.
- Keep save path unchanged.
- Keep the fallback private to `src/io/mod.rs` until real AC1015 coverage is broader.

## 6. Completion Criteria

- Synthetic AC1015 fallback test passes.
- Advisory tests still pass.
- `io::` tests pass.
- `RUSTFLAGS=-Dwarnings cargo check -p H7CAD` passes.
