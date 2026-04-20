# H7CAD PIDNEIGHBORS 命令落地计划

> 起稿：2026-04-19  
> 依赖：pid-parse `ObjectGraph::neighbors_of` / `endpoint_resolution_stats`（上一轮完成）
>
> **目标**：把上一轮新加的 `ObjectGraph::neighbors_of` 接到 H7CAD 命令行，作为该 API 的第一个真实消费者验证设计。同时顺手加 `PIDSTATS` 命令调 `endpoint_resolution_stats` 一行报告解析率。
>
> ```
> PIDNEIGHBORS D8FAB6ED48684E799CDFF0396E213773
>     PIDNEIGHBORS  3 neighbor(s) of D8FAB6ED…3773 (Equipment)
>         OBJ_AAAA1111  Instrument        FIT-001
>         OBJ_BBBB2222  PipeRun
>         OBJ_CCCC3333  Nozzle
>
> PIDSTATS
>     PIDSTATS  cluster <path>: 4 objects, 3 relationships
>         endpoint resolution: 1 fully / 2 partially / 0 unresolved
> ```

---

## 用户故事

> 1. 选一个对象，看它连接了哪些其它对象（拓扑邻居）
> 2. 一行查看当前 PID 的整体连接质量

## 设计

### 命令语法

```
PIDNEIGHBORS <drawing-id>      ← 32-hex 完整 drawing_id；命令行输出邻居列表 + 自身 item_type
PIDSTATS                       ← 当前 active tab 的 ObjectGraph 一行汇总
```

第一版要求精确 drawing_id（不做前缀匹配）；用户可结合 `PIDLISTPROPS` / `pid_inspect` 工具找出 id。

### H7CAD 端 API 增量

`src/io/pid_import.rs`：
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PidNeighborInfo {
    pub drawing_id: String,
    pub item_type: String,
    pub model_id: Option<String>,
    pub tag_label: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PidGraphStats {
    pub object_count: usize,
    pub relationship_count: usize,
    pub fully_resolved: usize,
    pub partially_resolved: usize,
    pub unresolved: usize,
}

/// Look up `drawing_id` in the cached `PidPackage`'s ObjectGraph and
/// return neighbors + the queried object's `(item_type, model_id, tag)`.
pub fn list_pid_neighbors(
    source: &Path,
    drawing_id: &str,
) -> Result<(PidNeighborInfo, Vec<PidNeighborInfo>), String>;

/// Aggregate count of objects, relationships, and endpoint-resolution
/// distribution for the cached `PidPackage`'s ObjectGraph.
pub fn pid_graph_stats(source: &Path) -> Result<PidGraphStats, String>;
```

`PidNeighborInfo` 内部从 `PidObject` 提取：tag_label 取 `extra.get("Tag").or(extra.get("ItemTag"))` 一行回显（前几轮 add_object_entities 也是这种取法）。

### 命令注册

紧邻 `PIDLISTPROPS`：
```rust
cmd if cmd == "PIDNEIGHBORS" || cmd.starts_with("PIDNEIGHBORS ") => {
    let did = cmd.split_once(' ').map(|(_, r)| r.trim()).unwrap_or("");
    if did.is_empty() { usage error }
    let i = active_tab; let source = ...; pid checks;
    match list_pid_neighbors(&source, did) {
        Ok((self_info, neighbors)) => {
            push_output("PIDNEIGHBORS  N neighbor(s) of ID (item_type)");
            for n in &neighbors { push_info row }
        }
        Err(e) => push_error
    }
}

cmd if cmd == "PIDSTATS" => {
    let i = active_tab; let source = ...; pid checks;
    match pid_graph_stats(&source) {
        Ok(s) => {
            push_output("PIDSTATS  N objects, M relationships in <target>");
            push_info("    endpoint resolution: F fully / P partially / U unresolved");
        }
        Err(e) => push_error
    }
}
```

### PIDHELP 更新

新增两行：
```
PIDNEIGHBORS <drawing-id>           list neighbors via ObjectGraph relationships
PIDSTATS                            object graph & endpoint resolution one-liner
```

命令族扩到 12（PIDHELP + 11 实际命令）。

### 测试

H7CAD `#[cfg(test)] mod tests` 新增 4 个：
1. `list_pid_neighbors_returns_self_and_neighbors`：build fixture + ObjectGraph 含 A↔B → load → query A → 自身=A + neighbors=[B]
2. `list_pid_neighbors_returns_error_for_unknown_drawing_id`：query "Z" → error 含 "not found"
3. `pid_graph_stats_returns_aggregate_counts`：fixture 有 N obj + M rel → 验证 stats 字段
4. `pid_graph_stats_without_object_graph_errors`：构造一个没有 object_graph 的 fixture（其实自然 fixture parse 出来 object_graph 是 Some 但 objects 是空的；这点要确认）

第一版 fixture 构造：用现有 `build_fixture_pid_with_general` 即可（解析后 object_graph 是 Some，objects 应该是空 — 因为 fixture 没有 P&IDAttributes 内容）。但这意味着 stats 全是 0 — 无法验证非 0 case。

简化方案：单元测试**不依赖端到端 fixture**；直接构造 `PidPackage` + 手工填一个 `PidDocument { object_graph: Some(...) }`，cache 进 store，调 helper 验证。

### 落地

- `cargo test io::pid_import` 全绿（含 4 新测）
- `cargo build` 全绿
- `.memory/2026-04-19.md` 追加段落

## 不做

1. **前缀匹配 drawing_id**：第一版只精确；前缀有歧义需要交互
2. **多跳邻居**（`PIDNEIGHBORS <id> --depth N`）：等真有用例
3. **PIDSTATS 输出 cross_reference 段**：与 ObjectGraph 解耦，第一版只 graph-only
4. **JSON 模式**（`--json`）：H7CAD 命令行没有 JSON 输出协议

## 工作量预估

- helpers：15 min
- 命令分支 ×2：15 min
- 测试 4 个：20 min
- 落地：10 min

合计 ~60 min。
