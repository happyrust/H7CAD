# pid-parse `ObjectGraph` 图遍历便利方法落地计划

> 起稿：2026-04-19  
> 依赖：pid-parse v0.4.1 现有 `ObjectGraph` 结构（`objects` / `relationships` / `by_drawing_id` 等）
>
> **目标**：给 `ObjectGraph` 加一组 ergonomic 查询方法，让下游消费方（H7CAD、第三方工具、未来 GUI）不必手动做 BTreeMap lookup + 双层扫描 `relationships` 来找邻居 / 算解析率。
>
> 当前 H7CAD `pid_import.rs::add_relationship_entities` 自己手动做：
>
> ```rust
> let source = relationship.source_drawing_id.as_ref().and_then(|id| positions.get(id));
> let target = relationship.target_drawing_id.as_ref().and_then(|id| positions.get(id));
> ```
>
> 而 "找邻居" / "解析率" 都可以一个方法搞定。

---

## 现状盘点

* `ObjectGraph` 是 plain struct，无 impl 方法
* `by_drawing_id: BTreeMap<String, usize>` 已经是 O(log N) 索引但用户得自己 `graph.by_drawing_id.get(id).map(|i| &graph.objects[*i])` 这样链
* `relationships` 是 `Vec<PidRelationship>`，找"涉及某对象的关系"需要全量扫描（O(N)）；如果有几百条 relationship + 频繁查询，这是 N×N
* 没有"解析率"汇总方法；`tests/parse_real_files.rs::relationship_endpoints_resolve_via_sheet_record` 自己手动 `.filter().count()`

## 用户故事

> 1. 给 H7CAD 提供"查询单对象的所有邻居"
>    ```rust
>    let neighbors = graph.neighbors_of("D8FAB6ED48684E799CDFF0396E213773");
>    // → Vec<&PidObject>
>    ```
> 2. 报告"endpoints 全解析了多少"
>    ```rust
>    let stats = graph.endpoint_resolution_stats();
>    println!("fully resolved: {}/{}", stats.fully_resolved, stats.total);
>    ```

## API 设计

`src/model.rs::ObjectGraph` 之后追加 `impl ObjectGraph { … }` 块：

```rust
impl ObjectGraph {
    /// O(log N) lookup: returns the `PidObject` at `drawing_id`, if any.
    pub fn object_by_drawing_id(&self, drawing_id: &str) -> Option<&PidObject>;

    /// Every relationship whose `source_drawing_id` or
    /// `target_drawing_id` matches `drawing_id`. O(R) — caller-side
    /// callers that need this in a hot loop should build a reverse
    /// index themselves.
    pub fn relationships_touching(&self, drawing_id: &str) -> Vec<&PidRelationship>;

    /// Distinct `&PidObject`s that share at least one relationship with
    /// `drawing_id`. Self-loops and unresolved (None) endpoints are
    /// silently skipped.
    pub fn neighbors_of(&self, drawing_id: &str) -> Vec<&PidObject>;

    /// Aggregate endpoint resolution health: how many relationships
    /// have both / one / zero of their endpoints resolved to a known
    /// `drawing_id`. Useful for diagnostic reports and CI invariants.
    pub fn endpoint_resolution_stats(&self) -> EndpointResolutionStats;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
pub struct EndpointResolutionStats {
    pub total: usize,
    pub fully_resolved: usize,    // both source and target resolved
    pub partially_resolved: usize, // exactly one endpoint resolved
    pub unresolved: usize,         // neither endpoint resolved
}
```

`neighbors_of` 内部：先 `relationships_touching` 拿涉及关系；从每个关系取对端 drawing_id（与查询 id 不同的那一个）；用 `object_by_drawing_id` 解析；用 `BTreeSet` 去重。

不引入 graph 全图加速结构（adjacency list）——第一版 O(R) 即可，N×N 优化等真有性能数据再做。

## 实施步骤

### Step 1 · model.rs 加 impl + 新结构体

在 `ObjectGraph` 定义之后（line ~580）直接加 `impl ObjectGraph` 块和 `EndpointResolutionStats` 结构体。`EndpointResolutionStats` 加 `JsonSchema` derive 让 schema 端能继续序列化（参考既有 `crossref::ClusterCoverage` 等）。

### Step 2 · 单元测试

`src/model.rs::tests` 内嵌 5 个：
- `object_by_drawing_id_returns_none_for_unknown` / `_returns_existing_object`
- `relationships_touching_filters_by_either_endpoint`
- `neighbors_of_dedupes_and_resolves`
- `endpoint_resolution_stats_counts_three_buckets`

构造 helper：手工组装 minimal `ObjectGraph { objects: vec![A, B, C], relationships: vec![A↔B, A↔none, none↔C], by_drawing_id: ... }` 验证。

### Step 3 · 重构现有调用点

`crossref.rs` / `inspect/report.rs` / `tests/parse_real_files.rs::relationship_endpoints_resolve_via_sheet_record` 等如有自己手动算解析率的，改用 `endpoint_resolution_stats`。第一版只做必然的，避免 scope creep。

实际检查后只做 1 处：`tests/parse_real_files.rs::relationship_endpoints_resolve_via_sheet_record` 用新 API 简化，保持原断言语义。

H7CAD 端 `pid_import.rs::add_relationship_entities` **本轮不动** —— 那是 visualization 路径，position lookup 与单纯 neighbors lookup 不完全是同一件事。

### Step 4 · 落地

- `cargo test --lib` 全绿（含 5 个新单测）
- `cargo test --test parse_real_files` 全过（条件降级路径）
- `cargo build` 全绿
- `pid-parse/CHANGELOG.md` 在 0.4.1 段尾追加 ObjectGraph impl 行
- 不 bump 版本（additive lib API，无破坏性）

## 公共 API 增量

### pid-parse
- 新增 `pub struct EndpointResolutionStats { total, fully_resolved, partially_resolved, unresolved }` (Serialize/Deserialize/JsonSchema/Default)
- 新增 4 个方法在 `impl ObjectGraph`：`object_by_drawing_id` / `relationships_touching` / `neighbors_of` / `endpoint_resolution_stats`

### H7CAD
- 无改动；下游想用直接调用即可

## 不做

1. **路径搜索**（A→B 的最短路径）：第一版只单跳邻居；多跳留给真有用例
2. **adjacency list 缓存**：O(R) 扫描在样本规模（几百关系）足够
3. **`reverse_index` 持久化字段**：与 `by_drawing_id` 同位思路，但下游用例不明确
4. **PidDocument 的 graph traversal facade**：先在 ObjectGraph 上加；将来真常用再升级

## 工作量预估

- Step 1 impl + struct：15 min
- Step 2 测试：20 min
- Step 3 重构 1 处：10 min
- Step 4 落地：10 min

合计 ~55 min。
