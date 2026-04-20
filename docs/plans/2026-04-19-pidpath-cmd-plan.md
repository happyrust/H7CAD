# H7CAD PIDPATH 命令落地计划

> 起稿：2026-04-19  
> 依赖：上一轮 ObjectGraph BFS 遍历能力 + drawing_id 前缀匹配
>
> **目标**：让用户能在命令行查询 P&ID 对象图中任意两点之间的最短路径。和 PIDNEIGHBORS（看周围）+ PIDFIND（找对象）形成完整图导航三件套。
>
> ```
> PIDPATH AAAA BBBB
>     PIDPATH  3 hop(s) from AAAA… (Equipment) to BBBB… (PipeRun)
>         AAAA…  Equipment
>         CCCC…  Nozzle
>         DDDD…  PipeRun
>         BBBB…  PipeRun
>
> PIDPATH AAAA ZZZZ
>     PIDPATH: no path from AAAA to ZZZZ (within 1000 hops)
> ```

---

## 设计

### pid-parse 端：`ObjectGraph::shortest_path`

`impl ObjectGraph`：
```rust
/// Shortest-path BFS in the relationship graph from `from_id` to
/// `to_id`. Returns `Some(path)` where `path[0] == from_id`,
/// `path.last() == to_id`, and adjacent entries are connected by at
/// least one resolved relationship endpoint. Returns `None` if no
/// such path exists or either endpoint is unknown.
///
/// `from_id == to_id` returns `Some(vec![from_id])` (zero-length path).
/// Cycles are safe; each `drawing_id` is enqueued at most once.
/// 
/// Time: O(V + E). Space: O(V).
pub fn shortest_path(&self, from_id: &str, to_id: &str) -> Option<Vec<&str>>;
```

实现：标准 BFS + predecessor map (`BTreeMap<&str, &str>`)，到 `to_id` 后反推。Predecessor map 不存 `from_id`，标识终止条件。

### H7CAD 端

`pid_import.rs`：
```rust
/// Resolve `from_or_prefix` and `to_or_prefix` via the same
/// prefix-match rules as [`list_pid_neighbors`], then return the
/// shortest path through the cached PidPackage's ObjectGraph.
/// Returns `(from_info, to_info, path_objects)` — `path_objects`
/// includes both endpoints in order.
pub fn list_pid_path(
    source: &Path,
    from_or_prefix: &str,
    to_or_prefix: &str,
) -> Result<(PidNeighborInfo, PidNeighborInfo, Vec<PidNeighborInfo>), String>;
```

resolve_id 逻辑抽成内部 fn 以避免在 list_pid_neighbors / list_pid_path 之间复制粘贴。

### 命令注册

紧邻 `PIDNEIGHBORS`：
```rust
cmd if cmd == "PIDPATH" || cmd.starts_with("PIDPATH ") => {
    let raw = cmd.strip_prefix("PIDPATH").unwrap_or("").trim();
    let parts: Vec<&str> = raw.split_whitespace().collect();
    if parts.len() != 2 {
        push_error("usage: PIDPATH <from-id-or-prefix> <to-id-or-prefix>");
        return;
    }
    // ...active tab/.pid checks...
    match list_pid_path(&source, parts[0], parts[1]) {
        Ok((from_info, to_info, path)) => {
            let hops = path.len().saturating_sub(1);
            push_output("PIDPATH  N hop(s) from X (type) to Y (type)");
            for n in &path { push_info row }
        }
        Err(e) => push_error
    }
}
```

### PIDHELP 更新

新增行：
```
PIDPATH <from-id-or-prefix> <to-id-or-prefix>   shortest path through ObjectGraph
```

命令族扩到 14。

### 测试

#### pid-parse `shortest_path`（5 个）
- `shortest_path_returns_zero_length_for_same_endpoint`：A → A 返回 `Some(["A"])`
- `shortest_path_finds_direct_neighbor`：A↔B → A → B 返回 `["A", "B"]`
- `shortest_path_finds_multi_hop`：A↔B↔C → A → C 返回 `["A", "B", "C"]`
- `shortest_path_returns_none_when_unreachable`：D 孤立 → A → D 返回 None
- `shortest_path_returns_none_for_unknown_endpoint`：A → "ZZZZ"（不存在）→ None

#### H7CAD（3 个）
- `list_pid_path_returns_path_through_chain`
- `list_pid_path_returns_error_when_no_path`：D 孤立 → 错误信息含 "no path"
- `list_pid_path_accepts_unique_prefix_for_both_endpoints`

### 落地

- pid-parse `cargo test --lib model::object_graph_impl_tests` 全绿（5 新测）
- H7CAD `cargo test io::pid_import` 全绿（3 新测）
- H7CAD `cargo build` 全绿
- pid-parse CHANGELOG 追加 shortest_path
- 不 bump pid-parse 版本

## 不做

1. **k 条最短路径** / **所有路径**：第一版只一条
2. **加权边**（按 relationship 类型加权）：所有边视为等权
3. **方向化最短路径**：与 neighbors_of 一致，无向看待
4. **路径上的 relationship guid**：只输出对象序列；如需边信息，下游 grep object 对在 relationships 里查

## 工作量预估

- pid-parse `shortest_path` + 5 测试：30 min
- H7CAD helper + 命令分支 + 3 测试：30 min
- PIDHELP / 落地：10 min

合计 ~70 min。
