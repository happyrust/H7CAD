# drawing_id 前缀匹配落地计划

> 起稿：2026-04-19  
> 依赖：上一轮的 `ObjectGraph::object_by_drawing_id` / `neighbors_of` + H7CAD `PIDNEIGHBORS` 命令
>
> **目标**：让 H7CAD 用户输入 drawing_id 时不必凑齐 32-hex；接受任何唯一前缀（≥2 字符）自动展开。命令行体验从"硬记 32-hex"升级为"输 8-12 字符即可"。
>
> ```
> PIDNEIGHBORS D8FAB6ED                ← 8 字符前缀
>     PIDNEIGHBORS  3 neighbor(s) of D8FAB6ED…3773 (Equipment)  ← 自动展开
>
> PIDNEIGHBORS DD                      ← 2 字符匹配多个
>     PIDNEIGHBORS: prefix 'DD' is ambiguous (matches 7): DD11AA…, DD22BB…, …
> ```

---

## 用户故事

> 1. 用 `PIDLISTPROPS` 看到对象 id 开头 `D8FAB6ED48684E79…`
> 2. 想看它的邻居，懒得拷贝整段 → `PIDNEIGHBORS D8FAB6ED`
> 3. 自动匹配，输出邻居

## 设计

### pid-parse 端：`ObjectGraph::find_drawing_ids_by_prefix`

`src/model.rs::impl ObjectGraph`：
```rust
/// All `drawing_id`s that start with `prefix`, in sorted order.
/// Empty `prefix` returns every id (≡ all object drawing_ids).
/// Case-sensitive — drawing_ids are uppercase 32-hex by convention.
pub fn find_drawing_ids_by_prefix(&self, prefix: &str) -> Vec<&str>;
```

实现：用 `BTreeMap::range(prefix..)` 拿到从 prefix 起的有序段，然后 take_while `key.starts_with(prefix)`。O(log N + K)。

### H7CAD 端：list_pid_neighbors 接受前缀

`pid_import.rs::list_pid_neighbors` 当前签名 `(source, drawing_id) -> Result<(Self, Vec), String>`，行为是精确查找。

**变化**：先做前缀展开。
```rust
pub fn list_pid_neighbors(source: &Path, drawing_id_or_prefix: &str)
    -> Result<(PidNeighborInfo, Vec<PidNeighborInfo>), String>
{
    let arc = ...;
    let graph = arc.parsed.object_graph.as_ref()...;
    
    let resolved_id = match graph.object_by_drawing_id(drawing_id_or_prefix) {
        Some(_) => drawing_id_or_prefix.to_string(),  // exact hit
        None => {
            let matches = graph.find_drawing_ids_by_prefix(drawing_id_or_prefix);
            match matches.len() {
                0 => return Err("no drawing_id matches '<X>'"),
                1 => matches[0].to_string(),
                n => return Err(format!(
                    "prefix '<X>' is ambiguous (matches {n}): {first 3 ids}, ..."
                )),
            }
        }
    };
    // 用 resolved_id 走原逻辑
}
```

注意：如果 `drawing_id_or_prefix` 长度恰好是 32-hex 但不在 graph 里，第一版也走前缀路径（matches=0 → 错误），与"长 prefix 没匹配"一致。

### 命令行不需要改

用户输入直接当作 prefix 传入 helper；helper 内部决定。

### PIDHELP 文字更新

```
PIDNEIGHBORS <drawing-id-or-prefix>   list neighbors via ObjectGraph; prefix accepted (≥2 chars unique)
```

### 测试

#### pid-parse `find_drawing_ids_by_prefix`（4 个）
- `find_by_prefix_returns_sorted_matches`
- `find_by_empty_prefix_returns_all_ids`
- `find_by_prefix_returns_empty_when_no_match`
- `find_by_long_prefix_acts_as_exact_match`

#### H7CAD（3 个）
- `list_pid_neighbors_accepts_unique_prefix`：fixture 含 AAAA/BBBB/CCCC，调 `list_pid_neighbors(src, "AA")` → 解析为 AAAA → 返回邻居
- `list_pid_neighbors_returns_ambiguous_prefix_error`：fixture 含 AA01/AA02，调 `list_pid_neighbors(src, "AA")` → 错误含 "ambiguous" + matches 数
- `list_pid_neighbors_returns_no_match_error`：调 `list_pid_neighbors(src, "ZZ")` → 错误含 "no drawing_id matches"

### 落地

- `cargo test --lib model::object_graph_impl_tests` 全绿（含 4 新测）
- `cargo test io::pid_import` 全绿（含 3 新测）
- H7CAD `cargo build` 全绿
- pid-parse `CHANGELOG.md` 在 0.4.1 段尾追加 `find_drawing_ids_by_prefix` 行
- 不 bump pid-parse 版本（additive）

## 公共 API 增量

### pid-parse
- 新增 `pub fn ObjectGraph::find_drawing_ids_by_prefix(&self, &str) -> Vec<&str>`

### H7CAD
- `list_pid_neighbors` 行为变化（接受前缀），签名不变。所有现有调用方继续 work（精确 32-hex 仍然是合法输入）

## 不做

1. **大小写不敏感前缀匹配**：drawing_id 按 SmartPlant 约定是大写 32-hex；用户应该按规范输
2. **PIDGETPROP / PIDSETPROP 也接前缀**：与 attr 名前缀有歧义风险，第一版只对 drawing_id 启用
3. **跨 object_id 全局前缀索引**：用 BTreeMap::range 即可，O(log N + K) 性能足够

## 工作量预估

- pid-parse 函数 + 4 测试：15 min
- H7CAD helper 改 + 3 测试：25 min
- PIDHELP / 落地：10 min

合计 ~50 min。
