# H7CAD Remaining Closeout After Stacked PRs Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to execute this plan task-by-task. Keep using stacked branches from the clean publish worktree; do not force-push `main`.

**Goal:** Finish the remaining H7CAD dirty-worktree cleanup after publishing the R46/R48/R50 stacked PR chain.

**Architecture:** Keep the original dirty worktree as the source inventory, but publish each remaining logical slice from `D:/work/plant-code/cad/H7CAD-r46-batch-a-publish` as stacked branches. Each slice must be small enough to verify independently and must avoid local agent/editor/tool directories unless the slice is explicitly the ignore-policy cleanup.

**Tech Stack:** Git worktrees, GitHub PRs against `happyrust/H7CAD`, Rust workspace checks, Markdown plan docs.

---

## 0. Current Checkpoint

Already completed:

- PR #1: `https://github.com/happyrust/H7CAD/pull/1`
  - Base: `main`
  - Head: `r46-ac1018-batch-a`
  - Scope: native AC1018 reader sections.
- PR #2: `https://github.com/happyrust/H7CAD/pull/2`
  - Base: `r46-ac1018-batch-a`
  - Head: `r48-facade-viewport-sync`
  - Scope: facade tests, `TP` alias, R48/R49 docs.
- PR #3: `https://github.com/happyrust/H7CAD/pull/3`
  - Base: `r48-facade-viewport-sync`
  - Head: `r50-model-owner-fallback`
  - Scope: model-space fallback for recovered DWG entities with unresolved owner handles.

Verified locally:

- Batch A native-DWG tests/check passed in the publish worktree.
- Batch B facade/property tests and `-Dwarnings` workspace check passed.
- R50 baseline test passed with AC1015 recovery at `238` document entities and diagnostics recovered total `177`.

Current original dirty worktree still shows:

- Real text diffs:
  - `src/io/pid_import.rs`
  - `src/io/svg_export.rs`
  - `CHANGELOG.md`
- Content-hash-clean but status-dirty tracked files, likely line-ending/index noise:
  - `crates/h7cad-native-model/src/geom_ocs.rs`
  - `src/entities/{arc,circle,lwpolyline,polyline,ray,shape,spline,traits}.rs`
  - `src/scene/acad_to_truck.rs`
- Untracked docs/plans:
  - `docs/cli.md`
  - `docs/pdf_export.md`
  - `docs/plans/2026-04-25-*`
  - `docs/plans/2026-04-26-*`
  - `docs/plans/2026-04-28-native-dwg-line-body-decode-plan.md`
  - `docs/plans/2026-04-28-r51-diagnostics-fallback-dedup-plan.md`
  - `docs/plans/2026-04-30-h7cad-main-divergence-batch-a-publish-plan.md`
  - this file.
- Local tool/editor/scratch directories:
  - `.agents/`, `.augment/`, `.claude/`, `.cursor/`, `.factory/`, `.junie/`, `.kiro/`, `.memory/`, `.pi/`, `.vscode/`, `.windsurf/`, `vendor_tmp/`, `clippy_full.log`, `skills-lock.json`.

---

## 1. Publish Branch Strategy

Continue the stack from PR #3:

```text
main
  -> r46-ac1018-batch-a          (#1)
    -> r48-facade-viewport-sync  (#2)
      -> r50-model-owner-fallback (#3)
        -> next branches
```

Default next branch names:

- `r51-fixture-schema-cleanup`
- `docs-native-dwg-cli-plan-archive`
- `chore-ignore-local-agent-dirs`

Each branch should use:

```powershell
cd D:/work/plant-code/cad/H7CAD-r46-batch-a-publish
git switch <base-branch>
git pull --ff-only
git switch -c <new-branch>
```

If `pull --ff-only` fails, stop and inspect; do not force.

---

## 2. Task Plan

### Task 1: Fixture / Schema Cleanup

**Purpose:** Commit the remaining Rust test fixture/schema cleanup separately from documentation archives.

**Files:**

- Modify: `src/io/pid_import.rs`
- Modify: `src/io/svg_export.rs`

**Known changes from original worktree:**

- `SPPID_SOFTWARE_VERSION` changes from `0.1.3` to `0.1.7`.
- `endpoint_decode_error: None` is added to a `CrossReferenceGraph` fixture.
- `aabb: WireModel::UNBOUNDED_AABB` is added to an SVG export test fixture.

**Steps:**

1. In the original dirty worktree, confirm the exact diff:
   ```bash
   git diff -- src/io/pid_import.rs src/io/svg_export.rs
   ```
2. Apply only these two file diffs to a new publish branch based on `r50-model-owner-fallback`.
3. Verify:
   ```powershell
   cargo test -p H7CAD --bin H7CAD sppid_software_version_tracks_cargo_pkg_version
   cargo test -p H7CAD --bin H7CAD svg_export
   $env:RUSTFLAGS='-Dwarnings'
   cargo check --locked --workspace --all-targets
   Remove-Item Env:RUSTFLAGS
   ```
4. Commit:
   ```text
   fix(test): update fixture schema defaults
   ```
5. Push branch `r51-fixture-schema-cleanup`.
6. Create stacked PR with base `r50-model-owner-fallback`.

Stop if:

- `pid_import.rs` carries behavioral parser changes beyond version/schema fixture updates.
- `svg_export.rs` carries real export behavior changes beyond fixture construction.

Execution finding (2026-04-30):

- `endpoint_decode_error: None` is already present on the publish stack.
- `SPPID_SOFTWARE_VERSION = "0.1.7"` fails on the publish stack because that branch's
  `Cargo.toml` still reports package version `0.1.3`; this change must wait for the
  version-bump branch or be dropped.
- `svg_export.rs` adding `aabb: WireModel::UNBOUNDED_AABB` fails because the publish
  stack does not yet contain the `WireModel::aabb` schema change; this belongs with
  the geometry/schema branch, not a standalone fixture cleanup PR.
- Result: do not create `r51-fixture-schema-cleanup` as originally scoped. Split the
  remaining two edits into their dependency branches.

### Task 2: Resolve Status-Dirty / No-Diff Files

**Purpose:** Avoid polluting commits with line-ending or index-only modifications.

**Files currently suspicious:**

- `crates/h7cad-native-model/src/geom_ocs.rs`
- `src/entities/arc.rs`
- `src/entities/circle.rs`
- `src/entities/lwpolyline.rs`
- `src/entities/polyline.rs`
- `src/entities/ray.rs`
- `src/entities/shape.rs`
- `src/entities/spline.rs`
- `src/entities/traits.rs`
- `src/scene/acad_to_truck.rs`

**Steps:**

1. Confirm no text diff:
   ```bash
   git diff --name-status -- <files>
   git diff --numstat -- <files>
   git status --porcelain=v2 -- <files>
   ```
2. If `git diff` is empty and only working-tree line-ending status remains, do not stage them.
3. If needed, normalize separately in a dedicated line-ending cleanup branch.

Stop if:

- Any file shows real Rust source diff. Move it into its own reviewed code batch.

### Task 3: Documentation Archive

**Purpose:** Land plan/docs that document already-executed work or CLI/PDF/SVG plans, separate from code.

**Candidate files:**

- `docs/cli.md`
- `docs/pdf_export.md`
- `docs/plans/2026-04-25-*.md`
- `docs/plans/2026-04-26-*.md`
- `docs/plans/2026-04-28-native-dwg-line-body-decode-plan.md`
- `docs/plans/2026-04-28-r51-diagnostics-fallback-dedup-plan.md`
- `docs/plans/2026-04-30-h7cad-main-divergence-batch-a-publish-plan.md`
- `docs/plans/2026-04-30-h7cad-remaining-closeout-after-stacked-prs.md`

**Steps:**

1. Run:
   ```bash
   git diff --check -- docs
   ```
2. Fix trailing whitespace in Markdown files.
3. Group docs into one of two branches:
   - `docs-native-dwg-cli-plan-archive` for stable plan archive.
   - a smaller branch if the docs naturally split into CLI/PDF vs native-DWG.
4. Commit:
   ```text
   docs(plan): archive native DWG and CLI execution plans
   ```
5. Push and open a stacked PR after Task 1.

Stop if:

- A doc describes work not yet represented by any code branch and reads like a release note rather than a plan.

### Task 4: CHANGELOG Closeout

**Purpose:** Update `CHANGELOG.md` only after the code PR stack is settled.

**Files:**

- `CHANGELOG.md`

**Steps:**

1. Review the current `CHANGELOG.md` diff:
   ```bash
   git diff -- CHANGELOG.md
   ```
2. Decide whether it describes PR #1/#2/#3 only, or also remaining docs/fixture cleanup.
3. If it is releasable, commit it after code/docs branches:
   ```text
   docs(changelog): record native DWG stacked closeout
   ```

Stop if:

- Changelog includes unmerged branches as if already shipped.

### Task 5: Local Tool Ignore Cleanup

**Purpose:** Decide how to handle untracked local agent/editor/tool directories.

**Default action:**

- Do not commit directory contents.
- Prefer targeted `.gitignore` entries only if the repository wants local AI/editor scratch directories ignored.

**Candidate ignored paths:**

- `.agents/`
- `.augment/`
- `.claude/`
- `.cursor/`
- `.factory/`
- `.junie/`
- `.kiro/`
- `.memory/`
- `.pi/`
- `.vscode/`
- `.windsurf/`
- `vendor_tmp/`
- `clippy_full.log`
- `skills-lock.json`

**Steps:**

1. Inspect existing `.gitignore`.
2. Add only targeted ignore rules.
3. Commit:
   ```text
   chore(gitignore): ignore local agent work directories
   ```

Stop if:

- Any directory contains project policy files that should be intentionally tracked.

### Task 6: ACadSharp Single-Line Cleanup

**Purpose:** Decide whether to commit the sibling repo `.gitignore` change.

**Repo:** `D:/work/plant-code/cad/ACadSharp`

**Known change:**

- `.gitignore` adds `.ace-tool/`.

**Steps:**

1. Run:
   ```bash
   git status --short
   git diff -- .gitignore
   ```
2. If still desired, commit in `ACadSharp` only:
   ```text
   chore(gitignore): ignore ace tool directory
   ```

Do not mix this with H7CAD commits.

---

## 3. Acceptance Criteria

- [ ] PR #1/#2/#3 remain untouched unless new conflicts require updates.
- [ ] Fixture/schema cleanup is a small code PR with focused verification.
- [ ] Documentation archive is separate from code behavior.
- [ ] `CHANGELOG.md` is committed only after its wording matches the actual PR stack state.
- [ ] No local agent/editor/tool directory contents are committed.
- [ ] Original dirty worktree has no unexplained real diffs after closeout.

---

## 4. Reporting Template

For each next branch/PR, report:

- branch name and PR URL.
- base branch.
- exact files included.
- verification commands and results.
- any remaining dirty files in the original worktree.
