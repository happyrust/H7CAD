# PID Metadata 命令族完整化计划

> 起稿：2026-04-19  
> 依赖：`pid-parse` v0.4.1 + H7CAD `PIDSETDRAWNO` / `PIDSETPROP`（同日早些时候完成）
>
> **目标**：把 PID metadata 命令族从"只能写 Drawing 流的 SP_* 属性"扩展到"读+写 + Drawing/General 双流"，同一轮内交付：
>
> | 命令 | 流 | 操作 | 状态 |
> |---|---|---|---|
> | `PIDSETDRAWNO <new>` | Drawing | 写 SP_DRAWINGNUMBER | 已落地 |
> | `PIDSETPROP <attr> <value...>` | Drawing | 写任意 attr | 已落地 |
> | `PIDGETPROP <attr>` | Drawing | **读** attr 当前值 | **本计划** |
> | `PIDSETGENERAL <element> <value...>` | General | **写**元素文本 | **本计划** |

`PIDGETGENERAL` 留到下一迭代（先看本轮 Drawing/General 双流编辑桥跑通后再补对偶读命令）。

---

## 现状盘点

* `pid_parse::writer`：`get_drawing_attribute` / `set_drawing_attribute` / `set_drawing_number` / `set_element_text` / `set_general_file_path` 已就位
* H7CAD `pid_import`：`edit_pid_drawing_attribute` (通用 Drawing 写)、`save_pid_native` (整包写出) 已就位
* 缺：(a) 读 Drawing attr 的命令包装；(b) 写 General element 的高层 helper + 命令包装

## 用户故事

> 1. `OPEN drawing.pid`
> 2. `PIDGETPROP SP_REVISION`  
>    → `PIDGETPROP  SP_REVISION = '1'`
> 3. `PIDSETPROP SP_REVISION 2`  
>    → 改了
> 4. `PIDSETGENERAL FilePath D:/issued/drawing-rev2.pid`  
>    → General 流的 `<FilePath>…</FilePath>` 文本被替换
> 5. `SAVEAS drawing-rev2.pid` → 验证

## 设计

### Step 1 · pid-parse 微增量：`get_general_element_text`

`writer::metadata_helpers` 加：
```rust
pub fn get_general_element_text(xml: &str, element: &str) -> Option<String>
```
对偶 `set_element_text`：唯一匹配返回 inner text；自闭合 / 未找到 / 重复 → `None`。3 个单元测试。

不 bump 版本（v0.4.1 polish 范围内）。

### Step 2 · H7CAD 高层 helper：General 流编辑

`src/io/pid_import.rs`：
```rust
const GENERAL_STREAM_PATH: &str = "/TaggedTxtData/General";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneralElementEdit {
    pub element: String,
    pub previous: Option<String>,
    pub next: String,
    pub new_xml_len: usize,
}

pub fn edit_pid_general_element(
    source: &Path,
    element: &str,
    value: &str,
) -> Result<GeneralElementEdit, String>
```
逻辑与 `edit_pid_drawing_attribute` 完全镜像，只是流路径和 helper 函数不同：
- 流：`/TaggedTxtData/General` 而非 `Drawing`
- 读：`pid_parse::writer::get_general_element_text`
- 写：`pid_parse::writer::set_element_text`

不抽公共函数（两个流的 metadata schema 在结构上不同，硬抽会增加间接性而无收益）。

### Step 3 · 命令注册

#### `PIDGETPROP <attr>` 在 `commands.rs::dispatch_command`，放在 `PIDSETPROP` 之后：
```rust
cmd if cmd == "PIDGETPROP" || cmd.starts_with("PIDGETPROP ") => {
    let attr = cmd.split_once(' ').map(|(_,r)| r.trim()).unwrap_or("");
    if attr.is_empty() {
        push_error usage; return;
    }
    let i = active_tab; let source = current_path 校验 .pid;
    // 读 cached PidPackage 的 Drawing 流 → utf8 → get_drawing_attribute
    // exactly-one match → push_output "PIDGETPROP  SP_X = 'value'"
    // None → push_error "attribute SP_X not found or appears multiple times"
}
```
读路径不需要 clone PidPackage（只读 → 用 `&*arc` 即可）。

#### `PIDSETGENERAL <element> <value...>` 紧接其后：
- 与 `PIDSETPROP` 几乎同形，只是调 `edit_pid_general_element`
- 命令行回显：`PIDSETGENERAL  FilePath '{prev}' → '{next}' ({n} bytes General XML; metadata-only edit)`

### Step 4 · 测试

#### pid-parse `get_general_element_text`（3 个）
- `returns_text_for_single_match`
- `returns_none_when_missing`
- `returns_none_when_self_closing`（self-closing tag 没有可读文本，与 set 端拒绝一致）

#### H7CAD（5 个）
1. `edit_pid_general_element_replaces_file_path`：build 一个含 `<General><FilePath>OLD</FilePath></General>` 的 fixture（新 helper：`build_fixture_pid_with_general`），改 → 校验
2. `edit_pid_general_element_returns_not_found_for_unknown_element`
3. `edit_pid_general_element_preserves_other_elements_byte_for_byte`
4. `read_pid_drawing_attribute_via_helper_returns_value`：直接调 `pid_parse::writer::get_drawing_attribute`（端到端读取链路）
5. 不为 PIDGETPROP 命令本身写测试（命令在 `dispatch_command` super-long 函数里，参考其它命令测试覆盖率为零，统一不测；helper 已被 #1-4 覆盖）

### Step 5 · 落地

- `cargo test io::pid_import` + `cargo test --lib writer::metadata_helpers` 全绿
- H7CAD `cargo build` 全绿
- `.memory/2026-04-19.md` 追加段落
- `pid-parse/CHANGELOG.md` 在 0.4.1 段尾追加 `get_general_element_text` 配套读取器行
- 不 bump pid-parse 版本

## 公共 API 增量

### pid-parse `writer::metadata_helpers`
- 新增 `pub fn get_general_element_text(xml: &str, element: &str) -> Option<String>`
- `set_*` 系列、`get_drawing_attribute`、`MetadataEditError` 不变

### H7CAD `io::pid_import`
- 新增 `pub struct GeneralElementEdit { element, previous, next, new_xml_len }`
- 新增 `pub fn edit_pid_general_element(source, element, value) -> Result<GeneralElementEdit, String>`

### H7CAD `dispatch_command`
- 新增分支 `"PIDGETPROP" / "PIDGETPROP …"`
- 新增分支 `"PIDSETGENERAL" / "PIDSETGENERAL …"`

## 显式不做（留给下一迭代）

1. **PIDGETGENERAL** 读 General element：等本轮跑通后单独加
2. **PIDLISTPROPS** 列出所有 SP_* 属性：需要新的 lib helper（遍历所有属性）
3. **PID undo snapshot**：依旧独立工作量
4. **写后验证模式**：`PIDSETPROP --verify` flag 写后立即 round-trip 比对

## 工作量预估

- Step 1：10 min（一个函数 + 3 测试）
- Step 2：15 min（镜像现有 helper）
- Step 3：15 min（两个命令分支）
- Step 4：20 min（5 个测试）
- Step 5：10 min

合计 ~70 min。
