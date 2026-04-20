# PID Edit CLI 落地计划

> 起稿：2026-04-19  
> 依赖：`pid-parse` v0.4.1（`writer::metadata_helpers` 已发布）+ H7CAD PID round-trip 通路（同日早些时候完成）。
>
> **目标**：在 H7CAD 命令行（`command_line`）暴露第一个 PID 编辑命令 `PIDSETDRAWNO <new>`，让用户能：
>
> ```
> 1. 打开 .pid 文件
> 2. 输入 PIDSETDRAWNO MY-NEW-001
> 3. SAVE 或 SAVEAS *.pid
> 4. 打开输出文件验证 SP_DRAWINGNUMBER 已更新，其它流字节级不变
> ```

---

## 现状盘点（2026-04-19）

* `pid-parse` v0.4.1 已发布 `writer::set_drawing_number(xml, value) -> Result<String, MetadataEditError>`：byte-level splice，保留所有未改字节
* H7CAD `pid_package_store::{cache, get, clear}_package` + `pid_import::save_pid_native` 已让"打开 → 保存"形成 round-trip 闭环
* 但当前 `PidPackage` 是只读：用户对 `NativeCadDocument` 的修改不会回流；要改 metadata 必须**绕过 NativeCadDocument**，直接动 cached `PidPackage` 的原始 stream 字节
* `dispatch_command` 的 main match 在 `commands.rs::dispatch_command`，模式参考 `ATTMAN` (line 3499) / `ALIASEDIT` (line 4166)

## 用户故事

> "我想在 H7CAD 里改一下当前 .pid 文件的图号，然后另存为新文件交给 SmartPlant 验证。"
>
> 1. `OPEN drawing.pid`  
> 2. `PIDSETDRAWNO DWG-NEW-001`  
>    → 命令行回显 `PIDSETDRAWNO  OLD-001 → DWG-NEW-001 (94 bytes Drawing XML)`  
> 3. `SAVEAS drawing-edited.pid`  
> 4. SmartPlant 打开 `drawing-edited.pid` → 图号已更新；其它流字节级保真

## 实施步骤

### Step 1 · pid-parse 微增量：`get_drawing_attribute` 读取器

`writer::metadata_helpers` 加一个 read-only 配套函数，让上层能在编辑前/后读出值用作日志：

```rust
pub fn get_drawing_attribute(xml: &str, attr: &str) -> Option<String>
```

- 复用现有 `find_attribute_value_ranges` 内部函数
- 两个/多个匹配：返回 `None`（与 set 端拒绝重复一致）
- 单元测试 ≥3 个：基本读取 / 不存在返回 None / 重复返回 None

不 bump pid-parse 版本（v0.4.1 范围内的 polish）。

### Step 2 · H7CAD 高层 helper：`pid_import::edit_pid_drawing_number`

`src/io/pid_import.rs` 加：

```rust
pub struct DrawingNumberEdit {
    pub previous: Option<String>,
    pub next: String,
    pub new_xml_len: usize,
}

pub fn edit_pid_drawing_number(
    source: &Path,
    new_value: &str,
) -> Result<DrawingNumberEdit, String>
```

行为：
1. `pid_package_store::get_package(source)` → 拿到 `Arc<PidPackage>`
2. clone 出可变 `PidPackage`
3. 取 `/TaggedTxtData/Drawing` 流字节 → `std::str::from_utf8`（失败时报错"Drawing XML is not UTF-8（BOM/UTF-16 暂不支持）"）
4. 用 `pid_parse::writer::get_drawing_attribute` 抓旧值
5. `pid_parse::writer::set_drawing_number` 拼新 XML
6. `package.replace_stream(...)` 写回 cloned 包
7. `pid_package_store::cache_package(source, package)` 替换缓存

错误：source 无缓存 / 不存在 Drawing 流 / XML 非 UTF-8 / metadata helper 报错（重复属性等）。

### Step 3 · H7CAD 命令注册：`PIDSETDRAWNO`

`src/app/commands.rs::dispatch_command` 主 match 添加分支（位置参考 ALIASEDIT 附近，与其它 manage 类命令同段）：

```rust
cmd if cmd == "PIDSETDRAWNO" || cmd.starts_with("PIDSETDRAWNO ") => {
    // 1. 解析参数：cmd.split_once(' ') 取 trim 后的 new_value
    // 2. 校验 active tab 的 current_path 后缀为 .pid
    // 3. 调用 pid_import::edit_pid_drawing_number(&source, new_value)
    // 4. 命令行 push_output / push_error 回显 + 标记 tab.dirty = true
    Task::none()
}
```

设计要点：
- 命令行命令（无 dialog），与 ALIASEDIT/ATTMAN 一致
- 参数缺失 / 空白 → push_error "usage: PIDSETDRAWNO <new-drawing-number>"
- 不是 PID tab → push_error "active tab is not a PID file"
- 不在历史栈里推 undo snapshot（PID 编辑暂时不参与 undo；下一迭代再考虑）
- 标记 `tab.dirty = true` 让 SAVE 走存盘路径

### Step 4 · 单元测试

#### pid-parse `get_drawing_attribute`（3 个）
- `get_returns_value_when_single_match`
- `get_returns_none_when_missing`
- `get_returns_none_when_duplicate`

#### H7CAD `edit_pid_drawing_number`（4 个，借现有 `tests` 模块的 `build_fixture_pid` helper）
- `edit_swaps_drawing_number_in_cached_package`：fixture → load_pid_native_with_package → edit → 重新 get_package 后 streams[Drawing] 含新值 + previous 是 fixture 的 "FX-001"
- `edit_without_cached_package_errors`
- `edit_when_drawing_xml_is_invalid_utf8_errors`：手动 cache 一个含 0xFF 0xFE 0x00 BOM 的 fixture，验证错误信息包含 "UTF-8" 关键字
- `edit_then_save_round_trips_new_drawing_number_through_disk`：edit → save_pid_native → re-parse_package → 验证 dst 的 Drawing 流含新值，其它流字节级不变

### Step 5 · 落地报告 + 验证

- `cargo test io::pid_package_store io::pid_import writer::metadata_helpers` 一键全绿
- H7CAD `cargo build` 不引入新 warning（容忍既有 PidBrowser 中间态 warning）
- `.memory/2026-04-19.md` 追加 "PID Edit CLI 第一个命令" 段落
- `pid-parse/CHANGELOG.md` 在 0.4.1 段尾追加 `get_drawing_attribute` 配套读取器行

## 公共 API 增量

### pid-parse `writer::metadata_helpers`
- 新增 `pub fn get_drawing_attribute(xml: &str, attr: &str) -> Option<String>`
- `MetadataEditError` 不变；`set_*` 系列不变

### H7CAD `io::pid_import`
- 新增 `pub struct DrawingNumberEdit { previous, next, new_xml_len }`
- 新增 `pub fn edit_pid_drawing_number(source, new_value) -> Result<DrawingNumberEdit, String>`

### H7CAD `dispatch_command`
- 新增分支 `"PIDSETDRAWNO" / "PIDSETDRAWNO …"`
- `Message` enum 不变（命令直接走 dispatch_command 字符串路径，不需要新 variant）

## 显式不做（留给下一迭代）

1. **PID undo/redo**：第一版编辑后立刻可见但不可撤销；PidPackage 的 in-memory 历史栈是单独工作量
2. **Properties panel UI 编辑器**：先用命令行验证编辑桥逻辑无问题，再做 UI（A 候选）
3. **`SP_*` 其它属性的命令**（PIDSETPROJECT 等）：待 PIDSETDRAWNO 验证模式后逐个加
4. **BOM / UTF-16 自动嗅探**：`pid-parse` 0.4.x risk note 已声明；等真实 fixture 出现再做
5. **写出后自动重 parse 校验**：当前 SAVE 不会自动 round-trip 校验；可后续加 `--verify` flag

## 风险与降级路径

- **风险 R1**：用户 OPEN PID → 修改 NativeCadDocument 视图 → PIDSETDRAWNO → SAVE，期望 native 编辑也回流 → 不会，第一版只回流 metadata helper 修改的字段。**降级**：明确在命令行回显里加 `(metadata-only edit; native scene changes are not flushed)` 提示
- **风险 R2**：`Drawing` XML 是 UTF-16 / 含 BOM → `from_utf8` 失败。**降级**：返回明确错误信息，不静默吞掉
- **风险 R3**：`SP_DRAWINGNUMBER` 在 XML 里出现多次（多 sheet 模板）→ `set_drawing_number` 返回 `DuplicateAttribute{count}`。**降级**：错误信息透传 count，让用户知道范围太宽

## 工作量预估

- Step 1：15 min（一个函数 + 3 测试）
- Step 2：30 min（高层 helper + 4 测试 fixture 复用）
- Step 3：15 min（命令分支）
- Step 4：随 Step 1-3
- Step 5：10 min

合计 ~70 min。
