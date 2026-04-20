# PIDNEIGHBORS --depth N 多跳邻居落地计划

> 起稿：2026-04-19  
> 依赖：上一轮 ObjectGraph 图遍历 API + PIDNEIGHBORS 单跳命令
>
> **目标**：让 PIDNEIGHBORS 能看 N 跳之内的所有可达对象，不只直接邻居。"看这条管段一路连了什么设备" / "查这台仪表上下游 2 跳影响范围"等典型用例。
>
> ```
> PIDNEIGHBORS AAAA           ← 默认 1 跳（与现有行为兼容）
> PIDNEIGHBORS AAAA --depth 2 ← 2 跳之内所有对象（不含起点本身的话）
> PIDNEIGHBORS AAAA --depth 0 ← 只有起点
> ```

---

## 设计

### pid-parse 端：`ObjectGraph::neighbors_within`

`impl ObjectGraph`：
```rust
/// BFS-walk: every object reachable from `drawing_id` within `depth`
/// hops via resolved relationship endpoints. The starting object
/// itself is **not** included in the result. Self-loops and
/// unresolved (`None`) endpoints are silently skipped.
///
/// `depth=0` → empty `Vec` (no hops taken yet).
/// `depth=1` → identical to [`neighbors_of`].
/// `depth=N` → all objects 1..=N hops away, distinct, in BFS visitation
///             order (level-by-level; within a level, by `drawing_id`
///             ascending).
pub fn neighbors_within(&self, drawing_id: &str, depth: usize) -> Vec<&PidObject>;
```

实现思路：
- 用 `BTreeSet<&str>` `seen` 跟踪已访问的 drawing_id（包括起点，让起点不会被算回去）
- 每一层用 `Vec<&str>` `frontier`；下一层用 `Vec<&str>` `next_frontier`
- 每跳：对当前 frontier 的每个 id 用 `neighbors_of`（已 `BTreeSet` 去重 + 排除 None）；新见到的 id 加进 `seen` 与 `next_frontier`
- 输出：每发现一个新对象就 push 到 `out`，自然形成 BFS 顺序

`depth=0` → 直接返回空（不走任何一跳）；`depth=usize::MAX` 等价于"完整连通分量"（可用于将来"找到所有同图对象"）。

### H7CAD 端 helper 升级

`pid_import.rs::list_pid_neighbors` 当前签名：
```rust
pub fn list_pid_neighbors(source, drawing_id_or_prefix)
    -> Result<(PidNeighborInfo, Vec<PidNeighborInfo>), String>
```

升级为：
```rust
pub fn list_pid_neighbors(source, drawing_id_or_prefix, depth: usize)
    -> Result<(PidNeighborInfo, Vec<PidNeighborInfo>), String>
```

行为：
- `depth=1` 等价于现行（保留语义）
- `depth=0` 返回空 neighbors 向量
- `depth>=2` 走 `neighbors_within(resolved_id, depth)`

向后兼容：所有 H7CAD 内部调用方需要明示传 depth。命令分支默认传 1。

### 命令解析

`PIDNEIGHBORS` 分支：把现有"单参数取 .split_once(' ')"升级为 token 遍历：
```rust
let raw = cmd.strip_prefix("PIDNEIGHBORS").unwrap_or("").trim();
let mut id_arg: Option<&str> = None;
let mut depth: usize = 1;
let mut bad: Option<String> = None;
let mut i = 0;
let tokens: Vec<&str> = raw.split_whitespace().collect();
while i < tokens.len() {
    match tokens[i] {
        "--depth" => {
            // require next token, parse usize
            let val = tokens.get(i + 1).ok_or_else(|| "--depth requires N".to_string());
            // ...
            i += 2;
        }
        t if t.starts_with("--") => { bad = Some(t.into()); break; }
        t if id_arg.is_none() => { id_arg = Some(t); i += 1; }
        _ => { i += 1; /* extra token ignored */ }
    }
}
```

`--depth N`：N 必须是非负整数，超过某个 sane 上限（如 1000）→ push_error。

### PIDHELP 更新

```
PIDNEIGHBORS <drawing-id-or-prefix> [--depth N]   list neighbors via ObjectGraph; default depth=1
```

### 测试

#### pid-parse `neighbors_within`（5 个）
- `neighbors_within_zero_returns_empty`
- `neighbors_within_one_equals_neighbors_of`
- `neighbors_within_two_walks_two_hops`
- `neighbors_within_skips_unreachable`
- `neighbors_within_handles_cycle_without_infinite_loop`

#### H7CAD（2 个）
- `list_pid_neighbors_with_depth_two_returns_extended`：fixture 含 A↔B↔C 链；depth=2 返回 [B, C]，depth=1 只返回 [B]
- `list_pid_neighbors_with_depth_zero_returns_only_self`：depth=0 → neighbors=[]

### 落地

- pid-parse `cargo test --lib model::object_graph_impl_tests` 全绿（5 新测）
- H7CAD `cargo test io::pid_import` 全绿（2 新测）
- H7CAD `cargo build` 全绿
- pid-parse CHANGELOG 追加 neighbors_within
- 不 bump pid-parse 版本

## 不做

1. **方向化遍历**（仅沿 source→target 方向走）：当前 relationship 是无向看待
2. **路径返回**（A→B→C 的具体跳序列）：第一版只返回"集合"
3. **加权遍历 / 最短路径**：等真实需求
4. **--include-self flag** 让起点也出现在输出：第一版 explicit 不含

## 工作量预估

- pid-parse `neighbors_within` + 5 测试：30 min
- H7CAD helper 升级 + 命令 token 解析 + 2 测试：30 min
- PIDHELP / 落地：10 min

合计 ~70 min。
