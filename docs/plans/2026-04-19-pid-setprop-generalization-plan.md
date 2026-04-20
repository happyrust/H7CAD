# PID Generic Property Edit 命令落地计划

> 起稿：2026-04-19  
> 依赖：`pid-parse` v0.4.1 `set_drawing_attribute` / `get_drawing_attribute`、H7CAD `PIDSETDRAWNO` / `pid_import::edit_pid_drawing_number`（同日早些时候完成）。
>
> **目标**：把"改 PID 单个 metadata 属性"从只能改 `SP_DRAWINGNUMBER` 一项升级为通用 `PIDSETPROP <attr> <value>`，一次解锁全部 `SP_*` 属性（`SP_PROJECTNUMBER` / `SP_DOC_NUMBER` / `SP_REVISION` / `SP_TITLE` / …），不再为每个属性新增一个命令。

---

## 现状盘点（2026-04-19 24:30）

* `pid-parse` v0.4.1 已发布通用 `set_drawing_attribute(xml, attr, value)` 与配套 `get_drawing_attribute(xml, attr)`
* H7CAD 已有专用 `edit_pid_drawing_number(source, value)` 与 `PIDSETDRAWNO <new>` 命令；二者本质都是 `set_drawing_attribute` + `pid_package_store` cache 替换的薄包装
* 缺：把这层包装通用化为"任意 `attr`"的版本

## 用户故事

> "我想把当前 PID 的项目号、修订号、标题一次性都改了再保存，而不是每改一个就要找一个新命令。"
>
> ```
> OPEN drawing.pid
> PIDSETPROP SP_PROJECTNUMBER PRJ-2026-A
> PIDSETPROP SP_REVISION       2
> PIDSETPROP SP_TITLE          Phase 8 review issue
> SAVEAS drawing-edited.pid
> ```

## 设计

### 命令语法
```
PIDSETPROP <attr> <value...>
```
* `<attr>`：单 token 大写（典型为 `SP_*`）
* `<value...>`：剩余所有 token 拼回（保留中间空格，**不**支持引号转义；这与 H7CAD 现有 `dispatch_command` 风格一致）
* 空 attr 或空 value：`push_error` 给出 usage

例：`PIDSETPROP SP_TITLE  Phase 8 review issue` → attr=`SP_TITLE`, value=`Phase 8 review issue`

### 分层
- `pid-parse`：**无新增**（已具备 `set_drawing_attribute` / `get_drawing_attribute`）
- H7CAD `pid_import`：
  - 新增 `pub struct DrawingAttributeEdit { attr: String, previous: Option<String>, next: String, new_xml_len: usize }`
  - 新增 `pub fn edit_pid_drawing_attribute(source: &Path, attr: &str, value: &str) -> Result<DrawingAttributeEdit, String>` — `edit_pid_drawing_number` 的通用版
  - **重构** `edit_pid_drawing_number` 为 `edit_pid_drawing_attribute(source, "SP_DRAWINGNUMBER", value)` 的薄包装；保留 `DrawingNumberEdit` 类型不变（向后兼容现有 PIDSETDRAWNO 命令）
- H7CAD `commands.rs`：加 `PIDSETPROP` 分支（紧邻 PIDSETDRAWNO），实现参数解析与 helper 调用
- 现有 `PIDSETDRAWNO` 保留：作为常用属性的便捷别名（用户场景里 SP_DRAWINGNUMBER 出现频率最高）

### 错误传透
- attr 名拼错 → `pid-parse` 返回 `MetadataEditError::AttributeNotFound{attr}` → H7CAD 报 `PIDSETPROP: metadata edit failed: attribute 'SP_TYO' not found in XML`（直接透传 thiserror Display）
- attr 在 XML 里出现多次 → `DuplicateAttribute{attr, count}` → 透传，让用户知道范围不唯一
- 其它错误（无 cache、非 UTF-8、缺 Drawing 流）：透传现有 `edit_pid_drawing_number` 的错误信息（共用 helper）

## 实施步骤

### Step 1 · 重构 `edit_pid_drawing_number` 为通用版

`src/io/pid_import.rs`：
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrawingAttributeEdit {
    pub attr: String,
    pub previous: Option<String>,
    pub next: String,
    pub new_xml_len: usize,
}

pub fn edit_pid_drawing_attribute(
    source: &Path,
    attr: &str,
    value: &str,
) -> Result<DrawingAttributeEdit, String> {
    // 1. lookup cache, clone PidPackage
    // 2. read /TaggedTxtData/Drawing as UTF-8
    // 3. previous = pid_parse::writer::get_drawing_attribute(xml, attr)
    // 4. new_xml = pid_parse::writer::set_drawing_attribute(xml, attr, value)?
    // 5. replace_stream + cache_package
    // 6. Ok(DrawingAttributeEdit { attr, previous, next, new_xml_len })
}

// 保留旧 API：薄包装
pub fn edit_pid_drawing_number(
    source: &Path,
    new_value: &str,
) -> Result<DrawingNumberEdit, String> {
    let edit = edit_pid_drawing_attribute(source, "SP_DRAWINGNUMBER", new_value)?;
    Ok(DrawingNumberEdit {
        previous: edit.previous,
        next: edit.next,
        new_xml_len: edit.new_xml_len,
    })
}
```

### Step 2 · 命令注册

`src/app/commands.rs`，在 PIDSETDRAWNO 分支之后插入：
```rust
cmd if cmd == "PIDSETPROP" || cmd.starts_with("PIDSETPROP ") => {
    // tokens: ["PIDSETPROP", attr, value...]
    let mut tokens = cmd.splitn(3, ' ');
    tokens.next();  // skip command name
    let attr = tokens.next().map(str::trim).unwrap_or("");
    let value = tokens.next().map(str::trim).unwrap_or("");
    if attr.is_empty() || value.is_empty() {
        self.command_line.push_error(
            "PIDSETPROP: usage: PIDSETPROP <attr> <value>",
        );
        return Task::none();
    }
    // …active tab/.pid checks identical to PIDSETDRAWNO…
    match crate::io::pid_import::edit_pid_drawing_attribute(&source, attr, value) {
        Ok(report) => {
            let prev = report.previous
                .as_deref()
                .map(|s| format!("'{}'", s))
                .unwrap_or_else(|| "(absent)".to_string());
            self.command_line.push_output(&format!(
                "PIDSETPROP  {} {} → '{}' ({} bytes Drawing XML; metadata-only edit)",
                report.attr, prev, report.next, report.new_xml_len
            ));
            self.tabs[i].dirty = true;
        }
        Err(e) => self.command_line.push_error(&format!("PIDSETPROP: {e}")),
    }
}
```

### Step 3 · 测试

H7CAD `src/io/pid_import.rs` `#[cfg(test)] mod tests` 新增 4 个：
- `edit_pid_drawing_attribute_swaps_arbitrary_attribute`：fixture 含 SP_DRAWINGNUMBER；用 PIDSETPROP-style 路径改它，验证替换
- `edit_pid_drawing_attribute_returns_attr_not_found_for_unknown_name`：用 SP_NOSUCH，验证错误信息含 attribute name
- `edit_pid_drawing_attribute_preserves_other_attributes_byte_for_byte`：构造含 SP_DRAWINGNUMBER + SP_OTHER 的 fixture（不能用现成 build_fixture_pid，临时新造），改 SP_DRAWINGNUMBER 后验证 SP_OTHER 行字节级保留
- `edit_pid_drawing_number_still_works_after_refactor`：现有 4 个 `edit_pid_drawing_number*` 测试不动，自然形成 regression 检验

### Step 4 · 落地

- `cargo test io::pid_import io::pid_package_store writer::metadata_helpers` 全绿
- `cargo build` 全绿
- `.memory/2026-04-19.md` 追加段落
- 不 bump pid-parse 版本（无 lib 改动）

## 显式不做（留给下一迭代）

1. **General 流元素文本编辑命令**（`PIDSETGENERAL <element> <value>`）：和 `set_general_file_path` 对应；与本命令机制相似但路径不同（流不同 + 元素而非属性），可后续单独加
2. **PIDGETPROP 读命令**：`get_drawing_attribute` 已具备底层；如果用户需要"看一下当前值再改"，加个读命令很便宜
3. **批量编辑命令**（`PIDSETPROPS attr1=val1 attr2=val2 …`）：第一版先验证单属性路径，再考虑批量

## 风险

- **R1**：用户 attr 拼错（如把 `SP_DRAWINGNUMBER` 写成 `SP_DRAWNUMBER`）→ AttributeNotFound 报错。**降级**：错误信息透传 attr 名，不静默；可后续加"模糊匹配建议"
- **R2**：值含 `<` `>` `&` `"` `'` → metadata_helpers 已自动 XML escape，安全
- **R3**：值为空字符串 → 第一版直接 push_error 拒绝（防止误清空）；如确有"清空属性"需求，后续加 `PIDSETPROP <attr> --empty` 显式 flag

## 工作量预估

- Step 1 重构：15 min
- Step 2 命令分支：10 min
- Step 3 测试：20 min
- Step 4 落地：10 min

合计 ~55 min。
