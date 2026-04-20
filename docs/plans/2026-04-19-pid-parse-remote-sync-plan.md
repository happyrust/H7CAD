# pid-parse 远程 sync + 差异分析计划

> 起稿：2026-04-19  
> 依赖：本会话所有 pid-parse/H7CAD PID 工作；commit `d6ddeb2`（本地 codex 分支）
>
> **目标**：在继续开发前先弄清 pid-parse 远程 main 与本地 codex 分支的**真实差异**——哪些工作远程已独立完成、哪些是我的唯一贡献、两边是否有能直接合并的改动。输出一份决策表格让用户选择整合策略。

---

## 背景

push 时发现：
```
origin/main  f30f5c7 docs: v0.3.12 新增 CONTRIBUTING.md
             4910442 fix(ci): 同步 Cargo.lock 到 v0.3.11
             8a1777c style: v0.3.11 cargo fmt
             039ce79 docs: Phase 8-9h 汇总
             713e509 docs(changelog): v0.3.10
             c136494 fix(tests): sheet_stream conditional skip  ← 与本地重叠
             8da0281 fix(tests): parse_real_files graceful skip ← 与本地重叠
             ef5e108 feat: v0.3.10 CI + README badge
             c677c15 feat: v0.3.2-v0.3.9 Writer 层 + CLI + 逆向消除  ← 可能与本地 v0.4.x 重叠

local codex/pid-workbench
             d6ddeb2 feat: PID writer 层 v0.4.x + 解析层 ergonomic API + 可发现性
```

远程 v0.3.2-v0.3.9 的 "Writer 层 + CLI + 逆向消除最后黑盒" 与我本地 v0.4.0 "Writer 层落地 + metadata_helpers" 是两条独立实现路径，需对齐。

## 实施步骤

### Step 1 · fetch + 列出远程独有 commit 及其改动摘要

```bash
cd pid-parse
git fetch origin main
```

然后收集：
- 远程 `origin/main` 上，本地 `codex/pid-workbench` 未包含的 9 个 commit 的 title + 文件清单（用 `git log codex/pid-workbench..origin/main --stat`）
- 本地 `codex/pid-workbench` 上，远程 `origin/main` 未包含的 commit（只有 `d6ddeb2`）的文件清单

### Step 2 · 按文件对比两端差异

对关键文件分别做：
- `git show origin/main:<path>` vs `git show codex/pid-workbench:<path>`
- 记录：
  - 远程唯一（只远程有 / 本地无）
  - 本地唯一（只本地有 / 远程无）
  - 两端都有但实现不同（冲突热点）
  - 两端完全一致（冗余，可放心丢弃本地）

重点关注文件：
- `src/writer/*` — 两端都做了 Writer 层，比较 API surface
- `src/package.rs` — 两端都可能有
- `src/bin/pid_writer_validate.rs` — 只本地有的 CLI
- `src/inspect/mod.rs` — 我的 `unidentified_top_level_streams`
- `src/model.rs` — ObjectGraph 新方法 (neighbors_within / shortest_path 等)
- `tests/parse_real_files.rs` / `unit_parsers.rs` — 条件降级
- `CHANGELOG.md` — 两端版本号冲突（远程 v0.3.12 vs 本地 v0.4.1）

### Step 3 · 输出决策报告

写到 `docs/plans/2026-04-19-pid-parse-sync-report.md`，格式：

```markdown
## 差异摘要

| 改动 | 远程 | 本地 | 状态 | 建议 |
|---|---|---|---|---|
| Writer 层核心 | v0.3.2-v0.3.9 (c677c15) | v0.4.0 (d6ddeb2) | 都实现，API 可能不同 | 对齐 → 取较完整版本 |
| metadata_helpers | ? | ✓ | 本地独有？ | 若远程无，可直接 cherry-pick 到 main |
| pid_writer_validate CLI | ? | ✓ | 本地独有？ | 同上 |
| unidentified_top_level_streams | ? | ✓ | 本地独有？ | 同上 |
| ObjectGraph 图遍历方法 | ? | ✓ | 本地独有 | 同上 |
| 条件测试降级 | ✓ (8da0281/c136494) | ✓ | 两端一致 | 丢本地，用远程 |
| CI workflow + README badge | ✓ | ✗ | 只远程 | 纳入本地 |
| CONTRIBUTING.md | ✓ | ✗ | 只远程 | 纳入本地 |

## 合并建议

方案 A：本地分支为主，cherry-pick 远程 CI/CONTRIBUTING
方案 B：远程 main 为主，cherry-pick 本地独有的 {A, B, C} 提交
方案 C：两端都 rebase 到共同基，重建线性历史
```

### Step 4 · 不动代码，等用户选择

报告写完即停。用户决定后再执行对应方案。

## 不做（本轮）

1. **实际合并 / rebase**：在用户定方案前不碰代码
2. **force push**：绝不
3. **删除本地 codex 分支**：保留作为备份

## 工作量预估

- Step 1 + 2：30 min（主要是 git show 对比）
- Step 3：15 min（写报告）
- Step 4：0 min（停）

合计 ~45 min。
