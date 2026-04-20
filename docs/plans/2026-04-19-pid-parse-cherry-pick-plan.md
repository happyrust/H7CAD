# pid-parse 真正独有内容 cherry-pick 到 main 计划

> 日期：2026-04-19  
> 基线：sync report（上一份）选定方案 A  
> 前提：远程 `origin/main` @ `51e7a28`（v0.3.12），本地 `codex/pid-workbench` @ `d6ddeb2`

---

## 目标

把本地 codex 分支上**远程 main 没有的真正独有内容**迁移到一个基于远程 main 的新分支 `codex/object-graph-ergonomic-api`，开一个干净的小 PR（~700 行净新增），避免 ~5k 行重复 diff 的整合风险。

## 要迁移的内容

三块（全部是 pid-parse additive API）：

### A. `impl ObjectGraph` 图遍历方法组（`src/model.rs`）

- 新结构 `EndpointResolutionStats { total, fully_resolved, partially_resolved, unresolved }`（含 `Serialize/Deserialize/JsonSchema/Default` 等全套 derive）
- `impl ObjectGraph` 内 9 个方法：
  - `object_by_drawing_id(&str) -> Option<&PidObject>`
  - `relationships_touching(&str) -> Vec<&PidRelationship>`
  - `neighbors_of(&str) -> Vec<&PidObject>`
  - `neighbors_within(&str, depth: usize) -> Vec<&PidObject>`
  - `find_drawing_ids_by_prefix(&str) -> Vec<&str>`
  - `find_objects_by_item_type(&str) -> Vec<&PidObject>`
  - `find_objects_by_extra(key, value) -> Vec<&PidObject>`
  - `shortest_path<'a>(&'a self, from, to) -> Option<Vec<&'a str>>`
  - `endpoint_resolution_stats() -> EndpointResolutionStats`
- `mod object_graph_impl_tests` 24 个单测

### B. `inspect` 可发现性 API（`src/inspect/mod.rs`）

- `pub const KNOWN_TOP_LEVEL_STREAM_NAMES: &[&str]` 13 个已识别顶层流名
- `pub const KNOWN_TOP_LEVEL_STORAGE_PREFIXES: &[&str]` 3 个前缀
- `pub fn unidentified_top_level_streams(&PidDocument) -> Vec<&StreamEntry>` 返回"pid-parse 尚未识别的顶层流"
- 4 个单测：empty_for_default / filters_all_known / keeps_unknown / strips_leading_slash
- **兼容远程**：远程 `src/inspect/mod.rs` 已有 `pub mod diff;`，新增内容不冲突
- **本地 inspect/report.rs 也要同步**：改用 `inspect::unidentified_top_level_streams` 替代内联 filter（这是 blob 差异中 `report.rs` 的一部分；要小心不 overwrite 远程其它 report 改动）

### C. 条件性真实文件测试（`tests/parse_real_files.rs`）

- 只加一个测试：`top_level_unidentified_streams_are_empty_on_sample_file`
- **远程 parse_real_files 已有自己的条件降级版本**；不替换，只追加这个测试

## 实施步骤

### Step 1 · 创建新工作区

```
cd pid-parse
git fetch origin
git switch -c codex/object-graph-ergonomic-api origin/main
```

### Step 2 · 迁移 A（model.rs）

从 `codex/pid-workbench` 拷贝：
- `EndpointResolutionStats` 结构定义
- `impl ObjectGraph { … }` 整块 9 个方法
- `mod object_graph_impl_tests` 整块 24 测试

**冲突点**：远程 `PidDocument` 结构加了 `doc_version2_decoded` 字段。因为我们只加 `impl ObjectGraph`（不动 PidDocument 结构体），**无冲突**。`ObjectGraph` 结构体字段两端一致。

### Step 3 · 迁移 B（inspect/mod.rs）

远程已有 `pub mod diff; pub mod mermaid; pub mod report;`。在末尾追加：
- `use crate::model::{PidDocument, StreamEntry};`
- `KNOWN_TOP_LEVEL_STREAM_NAMES` / `KNOWN_TOP_LEVEL_STORAGE_PREFIXES` 常量
- `unidentified_top_level_streams` 函数
- `#[cfg(test)] mod tests { … }` 4 个单测

**同时**检查远程 `src/inspect/report.rs` 是否内联"top-level unidentified"过滤逻辑：
- 如果有 → 替换为调用新 API（保持人类输出一致）
- 如果无 → 不动

### Step 4 · 迁移 C（tests/parse_real_files.rs）

远程 parse_real_files.rs 已有自己的条件降级 pattern（Option-based `parse_test_file`）。在文件末尾追加：
```rust
#[test]
fn top_level_unidentified_streams_are_empty_on_sample_file() {
    let Some(doc) = parse_test_file("DWG-0201GP06-01.pid") else { return };
    let leftover = pid_parse::inspect::unidentified_top_level_streams(&doc);
    assert!(leftover.is_empty(), "...");
}
```

### Step 5 · 编译 + 测试 + 提交 + push

```
cargo build
cargo test --lib
cargo test --test parse_real_files  (fixture 缺失时条件降级)
```

Commit message（中文）：
```
feat: ObjectGraph ergonomic API + inspect 可发现性 API

为 ObjectGraph 增加图遍历便利方法，为 inspect 模块增加
"还没解码的顶层流"待办清单 API。下游消费者不再需要手写
BTreeMap lookup / 双段 filter / 内联过滤白名单。

impl ObjectGraph:
- object_by_drawing_id, relationships_touching
- neighbors_of, neighbors_within (BFS 多跳)
- find_drawing_ids_by_prefix, find_objects_by_item_type,
  find_objects_by_extra
- shortest_path (BFS + predecessor map)
- endpoint_resolution_stats + EndpointResolutionStats

inspect:
- KNOWN_TOP_LEVEL_STREAM_NAMES / KNOWN_TOP_LEVEL_STORAGE_PREFIXES
  公开识别白名单
- unidentified_top_level_streams 返回待办清单
- report.rs 内部改用新 API，人类输出不变

测试：model::object_graph_impl_tests 24 + inspect tests 4 +
tests/parse_real_files.rs 新增条件性 top_level_unidentified
smoke。全部 additive 不改 PidDocument 字段结构。
```

Push 到 `origin/codex/object-graph-ergonomic-api`，给 PR URL。

### Step 6 · 不动 codex/pid-workbench

保留作历史备份。

## 不做

1. 不 rebase / force push / 删 codex/pid-workbench
2. 不合并远程 Writer 层差异（远程已做，不重做）
3. 不改 H7CAD 端（那仓库独立，已 push）

## 工作量预估

- Step 1-2：15 min
- Step 3：15 min（含查远程 report.rs 是否有内联 filter）
- Step 4：5 min
- Step 5：15 min（build + test + commit + push）

合计 ~50 min。
