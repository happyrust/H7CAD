# PID Metadata Command Family Consolidation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 收敛并完成 H7CAD 当前 PID metadata 命令族的实现与验证闭环，确保“打开 PID -> 读/改 metadata -> SAVE/PIDSAVEAS -> 验证回读”成为一条稳定、可回归的工作流。

**Architecture:** 现有实现已经以 `src/io/pid_package_store.rs` 作为原始 `PidPackage` 进程内缓存层，以 `src/io/pid_import.rs` 作为 metadata 编辑/读取/验证的高层门面，再由 `src/app/commands.rs` 暴露命令行命令。此次工作不再重新设计架构，而是把已落地但仍分散的能力整理为一组可执行任务，重点确认 3 条链路：缓存可用性、metadata helper 的行为一致性、命令层与保存/验证层的最终闭环。

**Tech Stack:** Rust 2021, H7CAD workspace, `pid-parse`, `cfb`, H7CAD command-line dispatch, crate 内单元测试与定向 `cargo test` 验证。

---

## Current Findings

- `src/io/pid_package_store.rs` 已提供 `cache_package / get_package / clear_package`，并带独立单测。
- `src/io/pid_import.rs` 已存在完整的 PID metadata 家族 helper：
  - `edit_pid_drawing_number`
  - `edit_pid_drawing_attribute`
  - `edit_pid_general_element`
  - `read_pid_drawing_attribute`
  - `read_pid_general_element`
  - `list_pid_metadata`
  - `verify_pid_cached`
  - `verify_pid_file`
- `src/app/commands.rs` 已注册并实现以下 PID 命令分支：
  - `PIDSETDRAWNO`
  - `PIDSETPROP`
  - `PIDGETPROP`
  - `PIDSETGENERAL`
  - `PIDSAVEAS`
  - `PIDHELP`
- `src/io/pid_import.rs` 末尾已积累大量测试，覆盖编辑、读取、列举、验证、邻居与路径分析。
- 当前真正缺的不是“从零实现”，而是把这条线整理为一份明确的、可顺序执行的收尾文档，避免后续继续在局部功能上重复补洞。

## Scope

本计划只覆盖 **PID metadata 命令族与保存验证闭环**，不扩展到：

1. DWG / native parser 工作
2. PID 图遍历新能力设计
3. SmartPlant 更深层 writer/schema 扩展
4. GUI properties panel 编辑器

## Success Criteria

完成后应满足以下验收标准：

1. 打开 `.pid` 后，缓存层稳定保存原始 `PidPackage`
2. `PIDSETDRAWNO / PIDSETPROP / PIDSETGENERAL / PIDGETPROP` 行为与帮助文案一致
3. metadata 改动只影响目标 XML 区域，其他字节保持可预期不变
4. `SAVE` / `PIDSAVEAS --verify` 后可通过 round-trip 验证
5. 相关定向测试一键通过

## File Map

### Core files

- Modify: `src/io/pid_package_store.rs`
- Modify: `src/io/pid_import.rs`
- Modify: `src/app/commands.rs`

### Reference files

- Check: `COMMANDS.md`
- Check: `docs/plans/2026-04-19-pid-edit-cli-plan.md`
- Check: `docs/plans/2026-04-19-pid-metadata-cmd-family-plan.md`
- Check: `docs/plans/2026-04-19-pid-read-family-completion-plan.md`
- Check: `docs/plans/2026-04-19-pid-saveas-cmd-plan.md`
- Check: `docs/plans/2026-04-19-pid-writer-validate-edit-mode-plan.md`

### Validation targets

- Test: `src/io/pid_package_store.rs` (inline tests)
- Test: `src/io/pid_import.rs` (inline tests)

---

## Task 1: Freeze the command-family contract

**Files:**
- Modify: `docs/plans/2026-04-21-pid-metadata-family-consolidation-plan.md`
- Check: `src/app/commands.rs:4160-5460`
- Check: `COMMANDS.md`

**Step 1: Enumerate the supported commands from code**

Read and confirm the live behavior of:

- `PIDSETDRAWNO`
- `PIDSETPROP`
- `PIDGETPROP`
- `PIDSETGENERAL`
- `PIDSAVEAS`
- `PIDHELP`

Capture exact usage strings and error wording from `commands.rs`.

**Step 2: Compare code behavior with command inventory**

Check whether `COMMANDS.md` and the actual command dispatch cover the same command family.  
Expected outcome:

- inventory says these commands are implemented
- dispatch code actually contains production branches

**Step 3: Record the contract in this plan**

Document, in this plan, the stable command contract:

- input shape
- active tab constraints
- `.pid` source path requirement
- dirty flag expectation
- metadata-only limitation

**Step 4: Verification**

No code changes yet. This task is complete when the engineer can answer:

- what each command expects
- what success output looks like
- what failure output looks like

**Step 5: Commit**

Do not commit yet. Bundle with first code change.

---

## Task 2: Validate and, if needed, harden the PID package cache surface

**Files:**
- Modify: `src/io/pid_package_store.rs`
- Check: `src/io/pid_import.rs: open_pid / load_pid_native_with_package / save_pid_native callers`
- Test: `src/io/pid_package_store.rs`

**Step 1: Write/adjust the failing test if any cache gap is found**

If review shows a missing cache behavior, add or tighten a focused test near the existing block:

```rust
#[test]
fn cache_then_get_returns_same_bytes() {
    let path = unique_path("cache-then-get");
    cache_package(&path, fixture_pkg("hello"));
    let pkg = get_package(&path).expect("should find cached package");
    assert_eq!(pkg.streams["/Marker"].data, b"hello");
}
```

Potential additions only if needed:

- canonical path aliasing
- overwrite semantics
- clear-after-overwrite semantics

**Step 2: Run the focused cache test**

Run:

```bash
cargo test pid_package_store -- --nocapture
```

Expected: all cache tests pass; if not, capture the exact failing case.

**Step 3: Write the minimal implementation**

If a gap exists, keep implementation minimal inside:

- `key_for`
- `cache_package`
- `get_package`
- `clear_package`

Do not redesign the store or introduce new concurrency primitives.

**Step 4: Run the focused test again**

Run:

```bash
cargo test pid_package_store -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/io/pid_package_store.rs
git commit -m "fix: harden pid package cache behavior"
```

Only commit if there were actual code changes.

---

## Task 3: Consolidate Drawing metadata edit helpers

**Files:**
- Modify: `src/io/pid_import.rs:240-320`
- Test: `src/io/pid_import.rs`

**Step 1: Start from the existing drawing-helper tests**

Relevant tests already present:

- `edit_pid_drawing_number_swaps_attribute_in_cached_package`
- `edit_pid_drawing_number_without_cached_package_errors`
- `edit_pid_drawing_number_rejects_non_utf8_drawing_xml`
- `edit_pid_drawing_attribute_swaps_arbitrary_attribute`
- `edit_pid_drawing_attribute_returns_attr_not_found_for_unknown_name`
- `edit_pid_drawing_attribute_preserves_other_attributes_byte_for_byte`

If behavior is incomplete or inconsistent, first add/adjust a failing test adjacent to these.

Example shape:

```rust
#[test]
fn edit_pid_drawing_attribute_preserves_other_attributes_byte_for_byte() {
    let src = unique_pid_path("attr-preserve");
    build_fixture_pid_with_multi_attrs(&src);
    load_pid_native_with_package(&src).expect("load fixture");

    edit_pid_drawing_attribute(&src, "SP_DRAWINGNUMBER", "NEW-9999")
        .expect("edit drawing number");

    let new_xml = std::str::from_utf8(
        &pid_package_store::get_package(&src).unwrap().streams[FIXTURE_DRAWING].data
    ).unwrap();
    assert!(new_xml.contains("SP_DRAWINGNUMBER=\"NEW-9999\""));
}
```

**Step 2: Run just the drawing-edit tests**

Run:

```bash
cargo test edit_pid_drawing -- --nocapture
```

Expected: any contract mismatch fails here first.

**Step 3: Write the minimal implementation**

Keep logic inside:

- `edit_pid_drawing_attribute`
- `edit_pid_drawing_number`

Implementation rules:

- require cached package
- require `/TaggedTxtData/Drawing`
- require UTF-8
- use `pid_parse::writer::*` helper for read/write
- re-cache the modified package
- return typed report struct

Do not duplicate XML parsing logic in H7CAD.

**Step 4: Re-run the test**

Run:

```bash
cargo test edit_pid_drawing -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/io/pid_import.rs
git commit -m "feat: consolidate pid drawing metadata edits"
```

---

## Task 4: Consolidate General metadata edit/read helpers

**Files:**
- Modify: `src/io/pid_import.rs:317-432`
- Test: `src/io/pid_import.rs`

**Step 1: Use the existing General-stream tests as the contract**

Existing tests:

- `edit_pid_general_element_replaces_file_path`
- `edit_pid_general_element_returns_not_found_for_unknown_element`
- `edit_pid_general_element_preserves_other_elements_byte_for_byte`
- `read_pid_general_element_returns_value_via_helper`
- `read_pid_general_element_returns_none_when_no_cache`

If one expected case is missing, add the smallest failing test first.

**Step 2: Run only the General helper tests**

Run:

```bash
cargo test pid_general -- --nocapture
```

Expected: only General-related failures remain visible.

**Step 3: Write the minimal implementation**

Restrict changes to:

- `edit_pid_general_element`
- `read_pid_general_element`

Implementation rules:

- same cache/UTF-8 discipline as Drawing stream
- call `pid_parse::writer::get_general_element_text`
- call `pid_parse::writer::set_element_text`
- re-cache only the modified package

**Step 4: Re-run the test**

Run:

```bash
cargo test pid_general -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/io/pid_import.rs
git commit -m "feat: consolidate pid general metadata edits"
```

---

## Task 5: Complete the read/list surface for metadata inspection

**Files:**
- Modify: `src/io/pid_import.rs:376-432`
- Modify: `src/app/commands.rs` (only if command-side behavior is inconsistent)
- Test: `src/io/pid_import.rs`

**Step 1: Confirm the inspection API contract**

Helpers to validate:

- `read_pid_drawing_attribute`
- `read_pid_general_element`
- `list_pid_metadata`

Existing tests already cover:

- no-cache returns `None` for soft reads
- `list_pid_metadata` returns typed error without cache
- listing preserves source order

If needed, add one focused failing test before changing code.

**Step 2: Run the inspection tests**

Run:

```bash
cargo test list_pid_metadata -- --nocapture
cargo test read_pid_ -- --nocapture
```

Expected: inspection contract is green in isolation.

**Step 3: Write minimal implementation**

Only adjust:

- error wording
- stream selection
- UTF-8 handling
- source-order preservation

Do not add new command names in this task.

**Step 4: Re-run tests**

Run:

```bash
cargo test list_pid_metadata -- --nocapture
cargo test read_pid_ -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/io/pid_import.rs src/app/commands.rs
git commit -m "feat: complete pid metadata inspection helpers"
```

Commit `commands.rs` only if it changed.

---

## Task 6: Verify command dispatch matches helper behavior

**Files:**
- Modify: `src/app/commands.rs:4160-4362`
- Test: existing app-level tests if present, otherwise rely on helper coverage + compile

**Step 1: Identify mismatches between command strings and helper semantics**

Check for:

- usage text mismatches
- active-tab guard mismatches
- `.pid` extension guard mismatches
- missing dirty-flag set after edits
- inconsistent “metadata-only edit” suffix

Relevant branches:

- `PIDSETDRAWNO`
- `PIDSETPROP`
- `PIDGETPROP`
- `PIDSETGENERAL`

**Step 2: If a mismatch exists, add the smallest regression test**

If there are no command-dispatch tests in this file, prefer:

- adding a narrow unit/integration test if infrastructure exists
- otherwise using compile + helper tests and keeping changes minimal

**Step 3: Write the minimal implementation**

Expected command behavior:

- edit commands mark tab dirty
- read command does not mark dirty
- edit success output includes byte count
- edit failure output prefixes command name
- all commands require an opened `.pid` source tab

**Step 4: Run compile / targeted validation**

Run:

```bash
cargo test pid_import -- --nocapture
cargo test pid_package_store -- --nocapture
cargo check
```

Expected: compile cleanly with command dispatch changes.

**Step 5: Commit**

```bash
git add src/app/commands.rs src/io/pid_import.rs
git commit -m "fix: align pid commands with metadata helpers"
```

---

## Task 7: Close the SAVE / PIDSAVEAS / VERIFY round-trip loop

**Files:**
- Modify: `src/io/pid_import.rs`
- Modify: `src/app/commands.rs:5201-5460`
- Test: `src/io/pid_import.rs`

**Step 1: Use the existing round-trip and verify tests as baseline**

Relevant tests already present in `pid_import.rs` include:

- `verify_pid_cached_passes_for_unmodified_fixture`
- `verify_pid_cached_passes_after_metadata_edit`
- round-trip save tests around `save_pid_native`
- verification tests around `verify_pid_file`

If any end-to-end behavior is still missing, add a failing test first.

Representative test shape:

```rust
#[test]
fn verify_pid_cached_passes_after_metadata_edit() {
    let src = unique_pid_path("verify-after-edit");
    build_fixture_pid(&src);
    load_pid_native_with_package(&src).expect("load fixture");

    edit_pid_drawing_attribute(&src, "SP_DRAWINGNUMBER", "EDITED-XYZ")
        .expect("edit drawing number");

    let report = verify_pid_cached(&src).expect("verify after edit");
    assert!(report.ok());
}
```

**Step 2: Run the round-trip-focused tests**

Run:

```bash
cargo test verify_pid -- --nocapture
cargo test save_pid -- --nocapture
```

Expected: failures point directly at save/verify contract drift.

**Step 3: Write the minimal implementation**

Possible edit surface:

- `save_pid_native`
- `verify_pid_cached`
- `verify_pid_file`
- `PIDSAVEAS` command branch

Rules:

- do not change file format strategy
- preserve `--verify`, `--force`, `--dry-run`
- keep dry-run non-destructive
- clear only the minimum necessary state

**Step 4: Re-run round-trip tests**

Run:

```bash
cargo test verify_pid -- --nocapture
cargo test save_pid -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/io/pid_import.rs src/app/commands.rs
git commit -m "fix: close pid save and verify loop"
```

---

## Task 8: Run the final validator set

**Files:**
- No code changes required unless validators fail

**Step 1: Run the targeted PID test slice**

Run:

```bash
cargo test pid_package_store -- --nocapture
cargo test pid_import -- --nocapture
```

Expected: all PID cache/import tests pass.

**Step 2: Run a broader workspace compile check**

Run:

```bash
cargo check
```

Expected: workspace compiles successfully.

**Step 3: If command dispatch changed materially, run a broader test pass**

Run:

```bash
cargo test
```

Use this only if earlier changes touched wider application behavior or if targeted tests are not enough to prove safety.

**Step 4: Fix failures before declaring done**

If any validator fails:

1. isolate the failing file or command
2. write the smallest regression test if one is missing
3. apply the minimum fix
4. re-run the failing validator

**Step 5: Commit**

```bash
git add -A
git commit -m "test: verify pid metadata command family"
```

Only if there are remaining uncommitted changes from fixes.

---

## Final Deliverable Checklist

- [ ] `pid_package_store` cache behavior verified
- [ ] Drawing metadata edit helpers verified
- [ ] General metadata edit helpers verified
- [ ] metadata read/list helpers verified
- [ ] command dispatch aligned with helper contract
- [ ] `PIDSAVEAS` round-trip verify flow verified
- [ ] targeted PID validators pass
- [ ] `cargo check` passes

## Recommended Commit Strategy

Keep commits small and topical:

1. `fix: harden pid package cache behavior`
2. `feat: consolidate pid drawing metadata edits`
3. `feat: consolidate pid general metadata edits`
4. `feat: complete pid metadata inspection helpers`
5. `fix: align pid commands with metadata helpers`
6. `fix: close pid save and verify loop`
7. `test: verify pid metadata command family`

If the code is already mostly correct and only tiny edits are required, squash into 2 commits instead:

1. `feat: complete pid metadata command family`
2. `test: verify pid metadata workflows`

## Out of Scope

Do not add these in this plan:

1. PID GUI property editor
2. Undo/redo for PID metadata edits
3. UTF-16/BOM auto-detection and transcoding
4. New graph search commands
5. New SmartPlant export schema features
6. DWG-related parser work

Plan complete and saved to `docs/plans/2026-04-21-pid-metadata-family-consolidation-plan.md`. Two execution options:

**1. Subagent-Driven (this session)** - I dispatch fresh subagent per task, review between tasks, fast iteration

**2. Parallel Session (separate)** - Open new session with executing-plans, batch execution with checkpoints

Which approach?
