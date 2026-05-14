# H7CAD Main Divergence Batch A Publish Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to execute this plan step-by-step. Do not force push.

**Goal:** Publish local Batch A commit `b9dcf921 feat(dwg): wire AC1018 native reader sections` safely despite `H7CAD` local `main` being divergent from `origin/main`.

**Architecture:** Keep the current dirty worktree intact and do the publishing work in a clean temporary worktree based on `origin/main`. Cherry-pick only the Batch A commit, verify it there, and push from the clean worktree or prepare a PR branch. This avoids rebasing a worktree that still contains Batch B/C/D/E edits and many local tool/editor directories.

**Tech Stack:** Git worktree, GitHub remote `origin`, Rust workspace, `h7cad-native-dwg`, Cargo package tests, `RUSTFLAGS=-Dwarnings`.

---

## 0. Current Facts

- Current repo: `D:/work/plant-code/cad/H7CAD`.
- Current branch: `main`.
- Local Batch A commit exists:
  - `b9dcf921 feat(dwg): wire AC1018 native reader sections`
- Push failed:
  - `git push origin HEAD` rejected as non-fast-forward.
- Divergence after fetch:
  - `git rev-list --left-right --count HEAD...FETCH_HEAD` returned `72 7`.
  - Local has 72 commits not in fetched `origin/main`.
  - Fetched `origin/main` has 7 commits not in local.
- Current worktree still has many dirty files for later Batch B/C/D/E and local tool/editor directories.
- Native-DWG Batch A verification passed locally before and after commit:
  - `cargo test -p h7cad-native-dwg --test real_samples -- --nocapture`
  - `cargo test -p h7cad-native-dwg`
  - `RUSTFLAGS=-Dwarnings cargo check --locked -p h7cad-native-dwg --all-targets`

---

## 1. Recommended Path

Use a clean worktree:

```text
H7CAD current dirty worktree
  keep as-is; do not rebase or pull here

H7CAD publish worktree
  base = origin/main
  cherry-pick = b9dcf921 only
  verify = native-dwg tests/check
  push = feature branch or main if policy allows
```

Default push target should be a feature branch:

```text
r46-ac1018-batch-a
```

Do not force-push `main`.

---

## 2. Task Plan

### Task 1: Confirm Current Worktree Is Not Suitable For Rebase

**Files:** no edits.

Run in `D:/work/plant-code/cad/H7CAD`:

```bash
git status --short
git rev-list --left-right --count HEAD...FETCH_HEAD
git log --oneline --right-only HEAD...FETCH_HEAD --max-count=10
git log --oneline --left-only HEAD...FETCH_HEAD --max-count=10
```

Expected:

- Dirty files are still present.
- Divergence remains non-zero on both sides.
- Therefore, do not `git pull`, `git merge`, or `git rebase` in this worktree.

### Task 2: Create A Clean Publish Worktree

**Files:** no repo file edits yet.

Use a sibling directory outside the current dirty worktree:

```powershell
cd D:/work/plant-code/cad/H7CAD
git fetch origin main
git worktree add ../H7CAD-r46-batch-a-publish FETCH_HEAD
```

Expected:

- New directory: `D:/work/plant-code/cad/H7CAD-r46-batch-a-publish`.
- It is clean and based on fetched `origin/main`.

Verify:

```powershell
cd D:/work/plant-code/cad/H7CAD-r46-batch-a-publish
git status --short
git log --oneline --max-count=5
```

Expected:

- `git status --short` prints nothing.
- HEAD is fetched `origin/main` tip, currently observed as `272a7e6e docs: add H7CAD documentation baseline`.

### Task 3: Cherry-Pick Batch A Commit

Run inside the clean publish worktree:

```powershell
git switch -c r46-ac1018-batch-a
git cherry-pick b9dcf921
```

Expected:

- Cherry-pick applies cleanly or shows explicit conflicts.

If conflicts happen:

- Resolve only Batch A files.
- Do not bring over current dirty worktree Batch B/C/D/E files.
- After resolving:
  ```powershell
  git status --short
  git diff --name-status
  git cherry-pick --continue
  ```

Stop if:

- Conflict involves unrelated facade/GUI/export files.
- Conflict suggests remote `origin/main` already contains equivalent AC1018 work.

### Task 4: Verify Cherry-Picked Batch A

Run inside the publish worktree:

```powershell
cargo test -p h7cad-native-dwg --test real_samples -- --nocapture
cargo test -p h7cad-native-dwg
$env:RUSTFLAGS='-Dwarnings'
cargo check --locked -p h7cad-native-dwg --all-targets
Remove-Item Env:RUSTFLAGS
```

Expected:

- All commands exit 0.
- If real sample is absent in the clean worktree, tests should soft-skip rather than fail. Record the actual behavior.

Optional broader check:

```powershell
cargo test --locked --workspace --all-targets
$env:RUSTFLAGS='-Dwarnings'
cargo check --locked --workspace --all-targets
Remove-Item Env:RUSTFLAGS
```

Only run the broader check if time allows; if it fails outside Batch A, record the exact failure and keep the Batch A branch intact.

### Task 5: Push A Feature Branch

Run inside the publish worktree:

```powershell
git push -u origin r46-ac1018-batch-a
gh run list --branch r46-ac1018-batch-a --limit 5
```

Expected:

- Push succeeds without force.
- CI run appears if the repository triggers CI on branch pushes.

If the project policy requires direct push to `main`:

```powershell
git push origin HEAD:main
```

Only do direct `main` push after confirming the branch is fast-forwardable:

```powershell
git fetch origin main
git merge-base --is-ancestor origin/main HEAD
```

Expected:

- command exits 0 before direct `main` push.

### Task 6: Report Back And Preserve Current Worktree

Report:

- publish branch name.
- cherry-pick commit hash.
- verification commands and pass/fail result.
- CI run id/status.
- current dirty worktree remains untouched for Batch B/C/D/E.

Do not delete the publish worktree until CI result is known.

---

## 3. Acceptance Criteria

- [ ] Current dirty `H7CAD` worktree is not rebased or force-pushed.
- [ ] Clean publish worktree is based on fetched `origin/main`.
- [ ] Only Batch A commit `b9dcf921` is cherry-picked.
- [ ] Native-DWG verification passes in the publish worktree.
- [ ] Push uses a feature branch by default.
- [ ] No agent/editor/tool directories are committed.
- [ ] CI result is checked or a clear pending CI state is reported.

---

## 4. Risks And Mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| Current dirty worktree is rebased | Batch B/C/D/E edits may conflict or be lost | Use separate clean worktree |
| Feature branch includes 72 local-only commits | PR is too large and includes unrelated history | Base publish worktree on `FETCH_HEAD`, then cherry-pick only `b9dcf921` |
| Remote already has overlapping AC1018 work | Cherry-pick conflicts or duplicates functionality | Inspect conflicts; stop if equivalent work exists |
| Direct push to `main` is not fast-forward | Push rejected or force-push temptation | Prefer feature branch; never force-push |
| Real sample absent in clean worktree | Test behavior differs | Record soft-skip vs hard failure; do not invent fixture data |

---

## 5. After This Plan

Once Batch A is published:

- Return to original dirty `H7CAD` worktree.
- Continue `docs/plans/2026-04-30-h7cad-dirty-worktree-closeout.md`:
  - Batch B: facade / build cleanup.
  - Batch C: IO/export/geometry helpers.
  - Batch D: remaining docs/CHANGELOG.
  - Batch E: local tool ignore policy.
- Handle `ACadSharp/.gitignore` separately if still needed.
