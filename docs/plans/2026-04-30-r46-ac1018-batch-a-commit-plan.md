# R46 AC1018 Batch A Commit Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Safely close out and commit the R46 Batch A native AC1018 DWG reader work without mixing it with facade, GUI, export, CLI, or local agent/tool changes.

**Architecture:** Batch A is the native-DWG-only slice: AC1018 encrypted metadata, LZ77, page map, section map, section payload reassembly, top-level `read_dwg` wiring, and real-sample tests. It must remain independent from later facade/default-backend/export work so failures can be bisected cleanly.

**Tech Stack:** Rust workspace, `h7cad-native-dwg`, `h7cad-native-model`, Cargo tests, `RUSTFLAGS=-Dwarnings`, real DWG sample soft-skip tests.

---

## 0. Current Checkpoint

Already observed:

- `H7CAD` is on branch `main`.
- Batch A candidate files are present but unstaged.
- Native-DWG verification has passed locally:
  - `cargo test -p h7cad-native-dwg --test real_samples -- --nocapture`
  - `cargo test -p h7cad-native-dwg`
  - `RUSTFLAGS=-Dwarnings cargo check --locked -p h7cad-native-dwg --all-targets`
- `Cargo.lock` currently has only one observed diff: `pid-parse` package version `0.11.6` -> `0.11.7`.

Important boundary:

- Do not commit on `main` unless the user explicitly authorizes the commit.
- Do not include `ACadSharp/.gitignore`.
- Do not include H7CAD agent/editor/tool directories.
- Do not include facade / GUI / export / CLI work in this Batch A commit.

---

## 1. Candidate File Boundary

### Include in Batch A

Code:

- `crates/h7cad-native-dwg/src/entity_common.rs`
- `crates/h7cad-native-dwg/src/error.rs`
- `crates/h7cad-native-dwg/src/known_section.rs`
- `crates/h7cad-native-dwg/src/lib.rs`
- `crates/h7cad-native-dwg/src/file_header_ac1018.rs`
- `crates/h7cad-native-dwg/src/lz77_ac18.rs`
- `crates/h7cad-native-dwg/src/page_map_ac1018.rs`
- `crates/h7cad-native-dwg/src/section_data_ac1018.rs`
- `crates/h7cad-native-dwg/src/section_map_ac1018.rs`
- `crates/h7cad-native-dwg/tests/real_samples.rs`

Plan docs:

- `docs/plans/2026-04-28-r46-dwg-ac1018-bring-up-plan.md`
- `docs/plans/2026-04-28-r46-a-encrypted-metadata-plan.md`
- `docs/plans/2026-04-28-r46-b-lz77-decompressor-plan.md`
- `docs/plans/2026-04-28-r46-c-page-map-plan.md`
- `docs/plans/2026-04-29-r46-d-section-descriptors-plan.md`
- `docs/plans/2026-04-29-r46-e1-section-data-plan.md`
- `docs/plans/2026-04-29-r46-e2-read-dwg-ac1018-plan.md`
- `docs/plans/2026-04-30-h7cad-dirty-worktree-closeout.md`
- `docs/plans/2026-04-30-r46-ac1018-batch-a-commit-plan.md`

Conditional:

- `Cargo.lock` only if the `pid-parse 0.11.7` lock drift is intentional for this workspace state. If not intentional, leave it out and handle separately.

### Exclude from Batch A

- `crates/h7cad-native-facade/src/lib.rs`
- `crates/h7cad-native-model/src/geom_ocs.rs`
- `crates/h7cad-native-model/src/lib.rs`
- `src/app/*`
- `src/io/*`
- `src/scene/*`
- `docs/cli.md`
- `docs/pdf_export.md`
- `docs/plans/2026-04-25-*`
- `docs/plans/2026-04-26-*` unless directly referenced by R46 Batch A.
- local tool/config directories: `.agents/`, `.augment/`, `.claude/`, `.cursor/`, `.factory/`, `.junie/`, `.kiro/`, `.pi/`, `.vscode/`, `.windsurf/`, `vendor_tmp/`, `clippy_full.log`, `skills-lock.json`.

---

## 2. Task Plan

### Task 1: Confirm Batch A Diff Shape

**Files:** read-only review of all Batch A candidate files.

Run:

```bash
git status --short -- Cargo.lock crates/h7cad-native-dwg docs/plans/2026-04-28-r46-dwg-ac1018-bring-up-plan.md docs/plans/2026-04-28-r46-a-encrypted-metadata-plan.md docs/plans/2026-04-28-r46-b-lz77-decompressor-plan.md docs/plans/2026-04-28-r46-c-page-map-plan.md docs/plans/2026-04-29-r46-d-section-descriptors-plan.md docs/plans/2026-04-29-r46-e1-section-data-plan.md docs/plans/2026-04-29-r46-e2-read-dwg-ac1018-plan.md
git diff --stat -- Cargo.lock crates/h7cad-native-dwg
git diff -- Cargo.lock
```

Expected:

- Batch A code diff is limited to `crates/h7cad-native-dwg`.
- AC1018 support is backed by new modules and `real_samples.rs` tests.
- `Cargo.lock` diff is reviewed and either explicitly included or deferred.

Stop if:

- Batch A requires files from facade / GUI / export.
- `Cargo.lock` contains unrelated dependency graph churn.

### Task 2: Re-run Batch A Verification

Run:

```powershell
cargo test -p h7cad-native-dwg --test real_samples -- --nocapture
cargo test -p h7cad-native-dwg
$env:RUSTFLAGS='-Dwarnings'
cargo check --locked -p h7cad-native-dwg --all-targets
Remove-Item Env:RUSTFLAGS
```

Expected:

- `real_samples` passes. Last observed: 38 passed.
- `h7cad-native-dwg` unit/integration/doc tests pass. Last observed: 53 unit + 38 real-sample tests passed.
- `-Dwarnings` package check passes.

Stop if:

- Any test fails after the previous passing checkpoint.
- A failure points to missing sample data rather than code behavior; document whether it soft-skipped or truly failed.

### Task 3: Stage Batch A Only

Run explicit staging only:

```bash
git add \
  crates/h7cad-native-dwg/src/entity_common.rs \
  crates/h7cad-native-dwg/src/error.rs \
  crates/h7cad-native-dwg/src/known_section.rs \
  crates/h7cad-native-dwg/src/lib.rs \
  crates/h7cad-native-dwg/src/file_header_ac1018.rs \
  crates/h7cad-native-dwg/src/lz77_ac18.rs \
  crates/h7cad-native-dwg/src/page_map_ac1018.rs \
  crates/h7cad-native-dwg/src/section_data_ac1018.rs \
  crates/h7cad-native-dwg/src/section_map_ac1018.rs \
  crates/h7cad-native-dwg/tests/real_samples.rs \
  docs/plans/2026-04-28-r46-dwg-ac1018-bring-up-plan.md \
  docs/plans/2026-04-28-r46-a-encrypted-metadata-plan.md \
  docs/plans/2026-04-28-r46-b-lz77-decompressor-plan.md \
  docs/plans/2026-04-28-r46-c-page-map-plan.md \
  docs/plans/2026-04-29-r46-d-section-descriptors-plan.md \
  docs/plans/2026-04-29-r46-e1-section-data-plan.md \
  docs/plans/2026-04-29-r46-e2-read-dwg-ac1018-plan.md \
  docs/plans/2026-04-30-h7cad-dirty-worktree-closeout.md \
  docs/plans/2026-04-30-r46-ac1018-batch-a-commit-plan.md
```

If including the lockfile:

```bash
git add Cargo.lock
```

Do not run `git add .`.

### Task 4: Verify Staged Diff

Run:

```bash
git diff --cached --name-status
git diff --cached --stat
git diff --cached --check
```

Expected staged files:

- only Batch A code files.
- only R46 / closeout plan docs.
- optionally `Cargo.lock`.

Must not include:

- `crates/h7cad-native-facade/src/lib.rs`
- `crates/h7cad-native-model/*`
- `src/app/*`
- `src/io/*`
- `src/scene/*`
- local agent/tool/editor directories.

If wrong:

```bash
git restore --staged .
```

Then repeat Task 3.

### Task 5: Commit Batch A

Only after explicit user authorization to commit on `main`:

```bash
git commit -m "$(cat <<'EOF'
feat(dwg): wire AC1018 native reader sections

EOF
)"
```

If PowerShell heredoc quoting is unreliable, use:

```powershell
$commitMessage = @'
feat(dwg): wire AC1018 native reader sections

'@
git commit -m $commitMessage
```

After commit:

```bash
git status --short
git log --oneline --max-count=5
```

Expected:

- Batch A commit exists.
- remaining dirty files belong to Batch B/C/D/E from `docs/plans/2026-04-30-h7cad-dirty-worktree-closeout.md`.

### Task 6: Post-Commit Verification

Run at least:

```bash
cargo test -p h7cad-native-dwg --test real_samples -- --nocapture
cargo test -p h7cad-native-dwg
RUSTFLAGS=-Dwarnings cargo check --locked -p h7cad-native-dwg --all-targets
```

If time allows before push:

```bash
cargo test --locked --workspace --all-targets
RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets
```

Expected:

- Native DWG package remains green.
- If workspace-level tests fail due unrelated pre-existing GUI/property issues, record exact failing test and do not patch it inside Batch A.

### Task 7: Push and CI

Only after commit verification:

```bash
git push origin HEAD
gh run list --branch main --limit 5
```

Wait for the new CI run if it starts.

Report:

- commit hash.
- exact verification commands and result.
- CI run id and status.
- remaining dirty files by next batch.

---

## 3. Acceptance Criteria

- [ ] Batch A staged diff contains only native-DWG AC1018 code, R46 plan docs, and optionally reviewed `Cargo.lock`.
- [ ] Native-DWG verification passes.
- [ ] Commit message is `feat(dwg): wire AC1018 native reader sections`.
- [ ] No local tool/editor directories are committed.
- [ ] Remaining dirty worktree is explicitly left for Batch B/C/D/E.
- [ ] If pushed, CI is monitored and any failure is attributed before further fixes.

---

## 4. Risks And Mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| `Cargo.lock` drift is unrelated | Commit contains hidden dependency/version churn | Inspect and include only if intentional; otherwise leave for separate lock cleanup |
| R46 plan docs include broader roadmap than Batch A | Commit review becomes noisy | Include only R46 AC1018 plan docs, leave unrelated CLI/export plans for later |
| Workspace tests fail outside native-DWG | Batch A gets polluted by unrelated GUI/IO fix | Record failure and move it to later batch |
| Committing directly on `main` | Hard to recover from bad commit | Require explicit user authorization, staged diff review, and post-commit verification |
| Agent/editor directories accidentally staged | Repo pollution | Never use `git add .`; verify staged name-status |

---

## 5. Next Batch Handoff

After Batch A is committed, continue with:

- Batch B: `crates/h7cad-native-facade/src/lib.rs`, `src/app/commands.rs`, `src/io/mod.rs`, `src/scene/mod.rs`.
- Batch C: IO/export/geometry helpers (`src/io/color_policy.rs`, `src/scene/hatch_geom.rs`, OCS model changes).
- Batch D: remaining docs/CHANGELOG.
- Batch E: `.gitignore` policy for local agent/editor/tool directories.
