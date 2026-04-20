# H7CAD PIDFIND 命令落地计划

> 起稿：2026-04-19  
> 依赖：pid-parse `ObjectGraph` 现有结构 + 上一轮新加的便利方法
>
> **目标**：让用户能在命令行按 item_type 或 extra 字段值快速找出对象。和 PIDLISTPROPS（列属性）+ PIDNEIGHBORS（看邻居）形成完整查询三件套。
>
> ```
> PIDFIND PipeRun
>     PIDFIND  3 object(s) of type 'PipeRun' in <target>
>         AAAA1111  PipeRun
>         BBBB2222  PipeRun  Run-002
>         CCCC3333  PipeRun
>
> PIDFIND Tag=FIT-001
>     PIDFIND  1 object(s) where Tag='FIT-001' in <target>
>         DDDD4444  Instrument  FIT-001
> ```

---

## 设计

### CLI 语法

```
PIDFIND <item-type>     ← 单 token 当 item_type 精确匹配（如 PipeRun, Instrument）
PIDFIND <key>=<value>   ← extra 字段精确匹配
```

第一版精确匹配（不做正则/通配）。空 token / 空 key / 空 value → push_error usage。

### pid-parse 端：两个新查询方法

`impl ObjectGraph`：
```rust
/// Linear scan: every object whose `item_type` exactly equals the
/// argument. Returned in source order.
pub fn find_objects_by_item_type(&self, item_type: &str) -> Vec<&PidObject>;

/// Linear scan: every object whose `extra[key]` exists and equals
/// `value`. Returned in source order.
pub fn find_objects_by_extra(&self, key: &str, value: &str) -> Vec<&PidObject>;
```

不引入索引（O(N) 在样本规模足够；将来真常用再加）。

### H7CAD 端

`pid_import.rs`：
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PidFindCriterion {
    ItemType(String),
    ExtraEquals { key: String, value: String },
}

pub fn list_pid_objects_matching(
    source: &Path,
    criterion: &PidFindCriterion,
) -> Result<Vec<PidNeighborInfo>, String>;
```

底层调对应 ObjectGraph 方法，结果 map 成 `PidNeighborInfo`（已有的投影）。

### 命令注册

紧邻 `PIDSTATS`：
```rust
cmd if cmd == "PIDFIND" || cmd.starts_with("PIDFIND ") => {
    let arg = cmd.split_once(' ').map(|(_, r)| r.trim()).unwrap_or("");
    if arg.is_empty() { usage error }
    let criterion = if let Some((k, v)) = arg.split_once('=') {
        PidFindCriterion::ExtraEquals { key: k.trim().to_string(), value: v.to_string() }
    } else {
        PidFindCriterion::ItemType(arg.to_string())
    };
    // ...PID checks...
    match list_pid_objects_matching(&source, &criterion) {
        Ok(matches) => {
            push_output("PIDFIND  N object(s) of type/where ...");
            for m in &matches { push_info row }
        }
        Err(e) => push_error
    }
}
```

### PIDHELP 更新

新增行：
```
PIDFIND <item-type>                 search by item_type (PipeRun / Instrument / ...)
PIDFIND <key>=<value>               search by extra field exact match
```

命令族扩到 13。

### 测试

#### pid-parse `find_objects_by_item_type` / `find_objects_by_extra`（4 个）
- `find_by_item_type_returns_matches_in_source_order`
- `find_by_item_type_returns_empty_for_unknown`
- `find_by_extra_returns_matching_value`
- `find_by_extra_returns_empty_when_key_missing`

#### H7CAD（3 个）
- `list_pid_objects_matching_filters_by_item_type`：synthetic graph 含 2 PipeRun + 1 Instrument，find ItemType("PipeRun") → 2
- `list_pid_objects_matching_filters_by_extra`：synthetic graph 含 Tag="FIT-001" + Tag="FIT-002"，find ExtraEquals(Tag, FIT-001) → 1
- `list_pid_objects_matching_returns_empty_when_no_match`

### 落地

- `cargo test --lib model::object_graph_impl_tests` 全绿（4 新测）
- `cargo test io::pid_import` 全绿（3 新测）
- H7CAD `cargo build` 全绿
- pid-parse CHANGELOG 追加两个新方法
- 不 bump 版本

## 不做

1. **正则 / 通配匹配**：第一版精确，灵活性留给未来
2. **多条件 AND/OR**：等真实用例
3. **跨字段全文搜索**（"FIT" 任意字段命中）：用 pid_inspect 报告 + grep 即可
4. **结果分页 / 截断**：典型 P&ID 对象数 < 1000，全显示

## 工作量预估

- pid-parse 2 方法 + 4 测试：20 min
- H7CAD enum + helper + 3 测试：25 min
- 命令分支 + PIDHELP：15 min
- 落地：10 min

合计 ~70 min。
