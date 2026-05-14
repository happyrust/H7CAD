# Native DWG Fallback Notice Hardening Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make AC1015 native fallback diagnostics actionable while preserving the primary `acadrust` error when fallback cannot recover.

**Architecture:** Keep the existing primary/fallback split in `src/io/mod.rs`. Improve only the private fallback helper and its tests: fallback success reports recovered entity count; fallback failure returns the original primary `OpenError`.

**Tech Stack:** Rust 2021, `h7cad-native-dwg`, `h7cad-native-model`, `cargo test`.

---

## 1. Scope

Included:

- Add a regression test for fallback notice content.
- Add a regression test for native fallback failure returning the original primary error.
- Include recovered entity count in the fallback warning.

Not included:

- Any parser coverage increase.
- Any DWG writer work.
- Any UI changes beyond existing notice surfacing.

## 2. Tasks

### Task 1: Tests

**Files:**
- Modify: `src/io/mod.rs`

Add assertions to the existing fallback test:

```rust
assert!(notice.message.contains("0 entities"));
```

Add a second test:

```rust
#[test]
fn native_dwg_fallback_returns_primary_error_when_native_rejects_bytes() {
    let primary_error = OpenError::Corrupt { format: "DWG", reason: "primary".into() };
    let err = native_dwg_fallback_from_bytes(b"ZZ9999", primary_error).unwrap_err();
    assert!(matches!(err, OpenError::Corrupt { reason, .. } if reason == "primary"));
}
```

### Task 2: Implementation

**Files:**
- Modify: `src/io/mod.rs`

Update fallback success notice to include:

- primary error message
- recovered entity count

### Task 3: Verify

Run:

```bash
cargo test --bin H7CAD native_dwg_fallback -- --nocapture
RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets
```

Expected: pass.
