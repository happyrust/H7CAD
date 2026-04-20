# PID Metadata Read 面完整化计划

> 起稿：2026-04-19  
> 依赖：H7CAD `PIDSETPROP` / `PIDGETPROP` / `PIDSETGENERAL`、`pid-parse` v0.4.1 metadata_helpers（同日早些时候完成）。
>
> **目标**：把 PID metadata 命令族的 **读** 一面补齐到与 **写** 对称：
>
> | 命令 | 已有? | 本计划 |
> |---|---|---|
> | PIDSETDRAWNO / PIDSETPROP / PIDSETGENERAL | ✓ |  |
> | PIDGETPROP（读 Drawing attr） | ✓ |  |
> | **PIDGETGENERAL**（读 General element） |  | **加** |
> | **PIDLISTPROPS**（一次列出 Drawing + General） |  | **加** |
>
> 加上后用户能完成"先看一眼有哪些可改、再选个改"的完整闭环，不再需要 SmartPlant 当作"只读浏览器"对照查表。

---

## 现状盘点

* `pid_parse::writer`：单值读 / 单值写 helper 全部到位
* H7CAD `pid_import`：`edit_pid_drawing_attribute`、`edit_pid_general_element`、`read_pid_drawing_attribute` 已就位
* 缺：(a) 读 General element 的高层 helper + 命令；(b) 一次列出全部 metadata 的列表 helper + 命令

## 用户故事

> 1. `OPEN drawing.pid`
> 2. `PIDLISTPROPS` → 一次看到 Drawing 所有 SP_* 属性 + General 所有 element 当前值
> 3. `PIDGETGENERAL FilePath` → 单看一个 element
> 4. `PIDSETPROP SP_REVISION 2` → 改

## 设计

### Step 1 · pid-parse 微增量：`list_drawing_attributes` / `list_general_elements`

`writer::metadata_helpers` 加：
```rust
/// All `<Tag attr="value" …/>` attribute (name, value) pairs found in
/// the Drawing XML, in document order. Duplicates are kept (the writer
/// rejects edits to ambiguous attrs, but readers should see everything).
/// Pure byte-level scan; consistent with the rest of the module.
pub fn list_drawing_attributes(xml: &str) -> Vec<(String, String)>

/// All `<element>text</element>` (name, text) pairs found in the
/// General XML, in document order. Self-closing tags and elements with
/// only nested children are skipped. Pure byte-level scan.
pub fn list_general_elements(xml: &str) -> Vec<(String, String)>
```
内部用同样的 byte 扫描风格（不引入 quick-xml dependency 切换）：
- `list_drawing_attributes`：遍历 `<…>` 元素，对每个开标签跑 attr scanner（`name="value"` 模式）；过滤掉 XML processing instruction (`<?…?>`) 和 comment (`<!--…-->`) 起始
- `list_general_elements`：遍历 `<openTag>…</openTag>` 对，提取 (tagname, inner text)；只返回 inner text 没有嵌套子元素的（用 inner text 不含 `<` 判定即可）

不 bump 版本（v0.4.1 polish）。新增 4-5 个单元测试覆盖：
- 基本用法
- 空 XML → 空 Vec
- 自闭合 tag 被跳过（list_general 端）
- 兼容 namespace prefix（不强求支持，但确保不 crash）
- 多个 Tag 元素的 attribute 累积

### Step 2 · H7CAD 高层 helper

`src/io/pid_import.rs`：
```rust
pub fn read_pid_general_element(source: &Path, element: &str) -> Option<String>;

pub struct PidPropsListing {
    pub drawing_attributes: Vec<(String, String)>,
    pub general_elements: Vec<(String, String)>,
}
pub fn list_pid_metadata(source: &Path) -> Result<PidPropsListing, String>;
```
- `read_pid_general_element` 与 `read_pid_drawing_attribute` 对称（软返回 `Option`）
- `list_pid_metadata` 走 cached `PidPackage`，返回有错误信息的 `Result`，因为"列不出来"对用户有用（cache 缺失 / stream 缺失 / non-UTF-8 都应明确）

### Step 3 · 命令注册

#### `PIDGETGENERAL <element>` —— 紧邻 `PIDGETPROP`：
- 镜像 `PIDGETPROP` 的实现，调 `read_pid_general_element`
- 输出格式：`PIDGETGENERAL  FilePath = 'C:/…/x.pid'`
- 错误：`PIDGETGENERAL: <element> not found, appears multiple times, is self-closing, or PID stream is unavailable`

#### `PIDLISTPROPS` —— 紧邻 `PIDGETGENERAL`：
- 无参数（第一版）
- 调 `list_pid_metadata`
- 输出多行：
  ```
  PIDLISTPROPS  Drawing /TaggedTxtData/Drawing  (3 attributes)
      SP_DRAWINGNUMBER     = FX-001
      SP_PROJECTNUMBER     = PRJ-OLD
      SP_REVISION          = 1
  PIDLISTPROPS  General /TaggedTxtData/General (3 elements)
      FilePath             = C:/old/path.pid
      FileSize             = 2048
      Author               = OLD-AUTHOR
  ```
- 一行命令行 `push_output` + N 行 `push_info`（参考 ALIASEDIT LIST 的输出风格）
- attr name 列宽用 `max(20, 全部 name 的 max len)`，对齐美观

### Step 4 · 测试

#### pid-parse `list_*`（5 个）
- `list_drawing_attributes_returns_pairs_in_document_order`：含 `<?xml ?>` + 含 `<!--comment-->` + 多个 `<Tag>`
- `list_drawing_attributes_returns_empty_for_empty_xml`
- `list_general_elements_returns_pairs_for_simple_xml`
- `list_general_elements_skips_self_closing`
- `list_general_elements_skips_elements_with_nested_children`：`<E><Sub/></E>` 不算，因为 inner 不是纯 text

#### H7CAD（5 个）
- `read_pid_general_element_returns_value`
- `read_pid_general_element_returns_none_when_no_cache`
- `list_pid_metadata_returns_drawing_and_general_pairs`：用 multi-attr fixture + general fixture 合并的版本
- `list_pid_metadata_returns_error_without_cache`
- `list_pid_metadata_returns_error_when_general_stream_missing`

### Step 5 · 落地

- `cargo test` 关键集合全绿
- `cargo build` 全绿
- `pid-parse/CHANGELOG.md` 在 0.4.1 段尾追加 `list_*` 行
- `.memory/2026-04-19.md` 追加段落

## 公共 API 增量

### pid-parse `writer::metadata_helpers`
- 新增 `pub fn list_drawing_attributes(xml: &str) -> Vec<(String, String)>`
- 新增 `pub fn list_general_elements(xml: &str) -> Vec<(String, String)>`

### H7CAD `io::pid_import`
- 新增 `pub fn read_pid_general_element(source, element) -> Option<String>`
- 新增 `pub struct PidPropsListing { drawing_attributes, general_elements }`
- 新增 `pub fn list_pid_metadata(source) -> Result<PidPropsListing, String>`

### H7CAD `dispatch_command`
- 新增 `"PIDGETGENERAL" / "PIDGETGENERAL …"`
- 新增 `"PIDLISTPROPS"`（无参数版本）

## 显式不做（留给下一迭代）

1. **PIDLISTPROPS 过滤参数**（`PIDLISTPROPS DRAWING` / `GENERAL` / `<prefix>`）：第一版只无参全量
2. **PID undo snapshot**：依旧独立工作量，待 metadata 命令族稳定一段时间后再做
3. **写后 verify 模式**：可作为 SAVE 的修饰
4. **XML namespace 处理**：list_* 的 byte 扫描不深入 namespace；本批次确保不 crash 即可

## 风险

- **R1**：`list_general_elements` 跳过含嵌套的元素 → 用户期望"全部都列出"。**降级**：第一版只列 leaf-text element；嵌套作为 Phase 2 增强
- **R2**：CDATA section 在 inner text 里 → byte 扫描可能错误把 `<![CDATA[…]]>` 内的 `<` 识别为子元素。**降级**：CDATA 暂不支持，文档明示
- **R3**：`<Tag>` 命名容易与其它名字冲突 → list_drawing_attributes 不限定开标签为 `Tag`，按"任何开标签的属性"返回。这与 set_drawing_attribute 不限定 `Tag` 一致

## 工作量预估

- Step 1：25 min（两个新函数 + 5 测试，byte 扫描状态机比 set 略复杂）
- Step 2：15 min
- Step 3：15 min（两个命令分支，PIDLISTPROPS 输出格式化稍多）
- Step 4：20 min（5+5 测试）
- Step 5：10 min

合计 ~85 min。
