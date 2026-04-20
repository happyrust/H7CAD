# pid-parse 远程 sync 差异分析报告

> 日期：2026-04-19  
> 基线：`origin/main` @ `51e7a28`（v0.3.12，merge）  
> 本地：`codex/pid-workbench` @ `d6ddeb2`（v0.4.1）

---

## 最终结论（第二次审视后更新）

**远程 main 已经吸收了本地 codex/pid-workbench 的全部工作**，**无需任何整合操作**。

验证：
```
$ git merge-base origin/main codex/pid-workbench
4c1cb804
$ git log --oneline 4c1cb80 | head -3
4c1cb80 feat: infer pid symbol hints from jsites
24e47d6 docs: note pid layout model
d6ddeb2 feat: PID writer 层 v0.4.x + 解析层 ergonomic API + 可发现性  ← 本地 commit 在远程祖先链里！

$ git diff 4c1cb80 codex/pid-workbench -- src/model.rs
(empty — model.rs 两端完全相同)
```

远程 main 通过以 `4c1cb80` 为 parent 的 merge commit 已经把我所有工作
（ObjectGraph impl、inspect::unidentified_top_level_streams、metadata_helpers、
pid_writer_validate 等）合并进去。用户本人或另一 agent 完成了这个 merge。

**唯一的微小差距**：`tests/parse_real_files.rs::top_level_unidentified_streams_are_empty_on_sample_file`
测试在远程 main 上没有（但其底层 API 在）。价值不大，可选补一个小 PR。

## 建议行动

| 候选 | 建议 |
|---|---|
| 重开 PR 合并 codex 分支 | **不做** — 远程已是 superset |
| cherry-pick 独有内容 | **不做** — 没有独有内容了 |
| 补那 1 个小测试 | 可选（单独 1 文件 PR，影响小） |
| 删本地 codex/pid-workbench | **不删** — 保留作历史 |
| 继续在 H7CAD 端工作 | ✅ H7CAD 的 14 个 PID 命令族独立，已 push，不受影响 |

---

（以下为初次分析记录，供参考）

## TL;DR (初判，已被后续证伪)

**本地 codex 分支严重落后远程 main**。统计：

```
origin/main..codex/pid-workbench  (本地相对远程)
43 files changed, +953 insertions, −3681 deletions
```

换句话说：远程 main 上有 ~2700 行是本地没有的（Phase 8-9 CI 绿化 + DocVersion2 解码 + 大量文档/测试）。**继续开发前必须做整合决策**。

## 事实清单

### 远程 main 已完成且本地无的工作

1. **Phase 9h CI 绿化**（`ef5e108` / `8da0281` / `c136494` / `8a1777c` / `4910442`）
   - `.github/workflows/ci.yml`（本地无）
   - `parse_real_files.rs` + `unit_parsers::sheet_stream_reuses_cluster_header` 条件降级（**本地也做了，但模式略微不同**）
   - `cargo fmt --all` 清理（26 个文件，含 `examples/*`、`src/crossref.rs` 等）
   - `CONTRIBUTING.md` 新增
2. **Phase 9f DocVersion2 结构化解码**（在 `c677c15` 内或后续）
   - `src/parsers/doc_version2.rs`（本地无）
   - `src/model.rs` 加 `DocVersion2 { magic_u32_le, reserved_all_zero, records }` + `DocVersion2Record { op_type, fixed, separator, version }` + `PidDocument.doc_version2_decoded` 字段
3. **Phase 8-9d docs**
   - `docs/phase8-9h-summary.md`（172 行，9 轮 Writer 周期总结）
   - `docs/writer-clsid-and-timestamps.md`
   - `docs/writer-quickstart.md`
4. **Writer 层演进**（v0.3.2-v0.3.9, `c677c15`）
   - `src/writer/xml_edit.rs`（本地无；和本地的 `metadata_helpers.rs` 是同一用途但不同命名）
   - `src/inspect/diff.rs`（本地无；148 行新增）
   - `src/writer/{mod,plan,metadata_write,sheet_patch,cfb_write}.rs` 内部升级
5. **examples/ + src/crossref.rs + src/streams/cluster.rs** 等模块 fmt 清理

### 两端内容完全一致（已对齐）

- `src/writer/metadata_helpers.rs`（blob `8654ae1c`）—— 说明**远程 main 上这文件与我写的字节完全相同**
- `src/bin/pid_writer_validate.rs`（blob `c4cc3f9a`）—— 同上

这解释了为什么两端血缘一致：此前某次同步把同一批代码带到了两端。

### 本地 codex 分支独有

1. **`impl ObjectGraph` 图遍历方法**（~220 行 + 测试 ~200 行，在 `src/model.rs`）
   - `object_by_drawing_id` / `relationships_touching` / `neighbors_of` / `neighbors_within`
   - `find_drawing_ids_by_prefix` / `find_objects_by_item_type` / `find_objects_by_extra`
   - `shortest_path` + `EndpointResolutionStats` + `endpoint_resolution_stats`
   - 共 24 个 `object_graph_impl_tests` 单元测试
2. **`inspect::unidentified_top_level_streams` 公共 API** + `KNOWN_TOP_LEVEL_STREAM_NAMES` / `KNOWN_TOP_LEVEL_STORAGE_PREFIXES` 常量（`src/inspect/mod.rs`）
   - 加 4 个新单测
3. **`tests/parse_real_files.rs` 新增**：`top_level_unidentified_streams_are_empty_on_sample_file` 条件性 smoke

### H7CAD 端（另一仓库，与 pid-parse 整合无关）

- 14 个 PID 命令族（已 push 到 `happyrust/H7CAD:codex/pid-workbench`）
- io::pid_import 52/52 全绿
- H7CAD 工作**全部保留**，不受 pid-parse 远程超前影响

## 整合方案

### 方案 A ✨ 推荐 · 最小化 cherry-pick

1. 本地 `codex/pid-workbench` 分支作为工作副本**丢弃**（不删分支，留作历史）
2. 在一个新分支 `codex/object-graph-ergonomic-api` 上，只 cherry-pick 真正独有的内容：
   - `src/model.rs` 的 `impl ObjectGraph` 块 + `EndpointResolutionStats`
   - `src/model.rs::object_graph_impl_tests` 24 个单测
   - `src/inspect/mod.rs` 的 `KNOWN_*` 常量 + `unidentified_top_level_streams` + 4 个单测
   - `tests/parse_real_files.rs::top_level_unidentified_streams_are_empty_on_sample_file`
3. 基于 `origin/main` 开 PR（约 **600-800 行 净新增**，远低于 codex 分支的 5k 行）

**理由**：
- 避免重复劳动的整合风险（Writer 层两端实现不同，硬合并会冲突）
- 只留真正独特的价值
- PR review 友好

### 方案 B · 放弃所有本地工作

只保留 H7CAD 端的 14 个命令（那是基于稳定 pid-parse API 写的），pid-parse 端完全以远程 main 为准。**要做**：确认 H7CAD 的 PID 命令在 `origin/main` pid-parse 的 API 上能继续编译（可能需要调用 xml_edit 而非 metadata_helpers 等重命名）。

### 方案 C · 本地作为 fork PR

把 codex 分支直接发 PR 到 main 让用户 review，用户决定哪些合并哪些丢。**风险**：PR 会有 ~5k 行 diff，review 成本高，冲突多。

## 建议

**方案 A**：低风险、高保值。估算工作量 ~2 小时。

## 我不会做的事（等用户决策）

- 不合并 / 不 rebase / 不 force push
- 不修改已 push 的 `codex/pid-workbench` 分支
- 不删除现有 H7CAD commit

等用户从 A/B/C 中选择后再执行。
