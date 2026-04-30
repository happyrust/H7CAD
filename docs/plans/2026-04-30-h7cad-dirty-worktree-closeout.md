# H7CAD Dirty Worktree Closeout Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to execute this plan batch-by-batch. Do not commit unknown tool/config directories unless the user explicitly asks.

**Date:** 2026-04-30
**Goal:** 把当前 `H7CAD` 大范围 dirty worktree 拆成可验证、可审查、可回滚的提交批次，避免把 native DWG 代码、GUI/IO 清理、计划文档和 agent/tool 目录混成一个不可审查的大 commit。
**Current state:** `pid-parse` 已 clean + CI green；`ACadSharp` 只有 `.gitignore` 单行 `.ace-tool/`；`H7CAD` 仍有大量 tracked / untracked 改动。
**Estimate:** 2-4 h for review + verification + staged commits, depending on test failures.

---

## 0. Safety Rules

- 不直接 `git add .`。
- 不提交 `.agents/`、`.augment/`、`.claude/`、`.cursor/`、`.factory/`、
  `.junie/`、`.kiro/`、`.pi/`、`.vscode/`、`.windsurf/`、`vendor_tmp/`、
  `clippy_full.log` 等 agent / editor / scratch 目录，除非用户明确要求。
- 所有 commit 都先看 staged diff：
  ```bash
  git diff --cached --name-status
  git diff --cached --stat
  ```
- 每个 commit 只包含一个主题；若验证失败，先定位根因，不把修复混入不相关 commit。

---

## 1. Observed Dirty Inventory

### Tracked code/documentation themes

当前 tracked diff 主要集中在：

- `crates/h7cad-native-dwg/`
  - AC1018 reader wiring / section map / known section / error handling。
  - `tests/real_samples.rs` 大幅增加 AC1018 real sample coverage。
- `crates/h7cad-native-facade/`
  - facade DWG load 接 native reader。
- `crates/h7cad-native-model/`
  - OCS / geometry model support。
- `src/app/commands.rs`、`src/app/properties.rs`
  - GUI command / properties path cleanup。
- `src/io/*`、`src/scene/*`
  - IO / SVG / scene cleanup 或 geometry helpers。
- `Cargo.lock`
  - dependency lock drift; 必须确认来源。

### Untracked code files

Likely code-bearing files:

- `crates/h7cad-native-dwg/src/file_header_ac1018.rs`
- `crates/h7cad-native-dwg/src/lz77_ac18.rs`
- `crates/h7cad-native-dwg/src/page_map_ac1018.rs`
- `crates/h7cad-native-dwg/src/section_data_ac1018.rs`
- `crates/h7cad-native-dwg/src/section_map_ac1018.rs`
- `src/io/color_policy.rs`
- `src/scene/hatch_geom.rs`
- `tests/fixtures/sample.html`

### Untracked planning / docs

Plan/docs files are numerous and should be grouped separately from code:

- `docs/cli.md`
- `docs/pdf_export.md`
- `docs/plans/2026-04-25-*`
- `docs/plans/2026-04-26-*`
- `docs/plans/2026-04-28-*`
- `docs/plans/2026-04-29-*`
- this file: `docs/plans/2026-04-30-h7cad-dirty-worktree-closeout.md`

### Untracked tool/config noise

Treat as **do not commit by default**:

- `.agents/`, `.augment/`, `.claude/`, `.cursor/`, `.factory/`, `.junie/`,
  `.kiro/`, `.memory/`, `.pi/`, `.vscode/`, `.windsurf/`
- `skills-lock.json`
- `vendor_tmp/`
- `clippy_full.log`

These may need `.gitignore` cleanup, but that should be a separate commit after
confirming the repo's policy.

---

## 2. Proposed Commit Batches

### Batch A — R46 AC1018 native DWG reader

Purpose: land AC1018 file/header/page/section decode and top-level reader wiring.

Candidate files:

- `Cargo.lock`
- `crates/h7cad-native-dwg/src/error.rs`
- `crates/h7cad-native-dwg/src/known_section.rs`
- `crates/h7cad-native-dwg/src/lib.rs`
- `crates/h7cad-native-dwg/src/entity_common.rs`
- `crates/h7cad-native-dwg/src/file_header_ac1018.rs`
- `crates/h7cad-native-dwg/src/lz77_ac18.rs`
- `crates/h7cad-native-dwg/src/page_map_ac1018.rs`
- `crates/h7cad-native-dwg/src/section_data_ac1018.rs`
- `crates/h7cad-native-dwg/src/section_map_ac1018.rs`
- `crates/h7cad-native-dwg/tests/real_samples.rs`
- relevant R46 plan docs:
  - `docs/plans/2026-04-28-r46-*.md`
  - `docs/plans/2026-04-29-r46-*.md`

Verification:

```bash
cargo test -p h7cad-native-dwg --test real_samples -- --nocapture
cargo test -p h7cad-native-dwg
RUSTFLAGS=-Dwarnings cargo check --locked -p h7cad-native-dwg --all-targets
```

Commit message:

```text
feat(dwg): wire AC1018 native reader sections
```

### Batch B — R48 facade / build cleanup

Purpose: land facade DWG load wiring and warning cleanup independent of AC1018.

Candidate files:

- `crates/h7cad-native-facade/src/lib.rs`
- `src/app/commands.rs`
- `src/io/mod.rs`
- `src/scene/mod.rs`
- `src/app/properties.rs` only if the diff is part of the documented warning / viewport sync fix.
- relevant R48 / R49 plan docs:
  - `docs/plans/2026-04-28-r48-facade-and-build-cleanup-plan.md`
  - `docs/plans/2026-04-28-r49-viewport-paper-space-sync-plan.md`

Verification:

```bash
cargo test -p h7cad-native-facade
RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets
```

Commit message:

```text
fix(native): connect DWG facade load path
```

### Batch C — IO / export / geometry helpers

Purpose: keep CLI / PDF / SVG / color / hatch work separate from native DWG reader work.

Candidate files:

- `src/io/color_policy.rs`
- `src/io/pid_import.rs`
- `src/io/svg_export.rs`
- `src/scene/acad_to_truck.rs`
- `src/scene/hatch_geom.rs`
- `crates/h7cad-native-model/src/geom_ocs.rs`
- `crates/h7cad-native-model/src/lib.rs`
- `docs/cli.md`
- `docs/pdf_export.md`
- relevant CLI/PDF/SVG plans under `docs/plans/2026-04-25-*`

Verification:

```bash
cargo test --locked --workspace --all-targets
RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets
```

Commit message:

```text
feat(export): add shared color and hatch helpers
```

### Batch D — plan/documentation archive

Purpose: land planning docs that are not tied to the code batches above.

Candidate files:

- remaining `docs/plans/*.md`
- `CHANGELOG.md`

Verification:

```bash
git diff --check
```

Commit message:

```text
docs(plan): add native DWG and CLI execution plans
```

### Batch E — local tool ignore cleanup

Purpose: decide what to do with agent/editor scratch directories.

Default action:

- Do not commit tool directories.
- If the repo wants to ignore them, add targeted `.gitignore` entries in a
  separate commit.

Candidate commit message:

```text
chore(gitignore): ignore local agent work directories
```

---

## 3. Execution Order

1. Run read-only inspection for each batch:
   ```bash
   git diff --stat -- <candidate files>
   git diff -- <candidate files>
   ```
2. Start with Batch A if AC1018 is the main product goal.
3. Run Batch A verification; fix only Batch A failures.
4. Commit Batch A.
5. Repeat for Batch B / C.
6. Commit docs-only Batch D after code commits are known-good.
7. Handle Batch E last, only if needed.
8. Push after all chosen batches are committed, then wait for CI.

---

## 4. Stop Conditions

Stop and ask before committing if:

- A batch mixes native DWG reader code with CLI/export/tooling changes.
- `Cargo.lock` diff is not explained by a committed `Cargo.toml` change.
- Verification reveals a pre-existing failure unrelated to the batch.
- Any staged diff includes tool/scratch directories.
- `H7CAD` branch is `main`/`master` and user has not explicitly authorized commits.

---

## 5. Immediate Next Step

Run Batch A read-only review:

```bash
git diff --stat -- Cargo.lock crates/h7cad-native-dwg docs/plans/2026-04-28-r46-dwg-ac1018-bring-up-plan.md docs/plans/2026-04-29-r46-e2-read-dwg-ac1018-plan.md
git diff -- Cargo.lock crates/h7cad-native-dwg
```

Then decide whether Batch A is internally coherent enough to verify and commit.
