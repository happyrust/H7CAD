# 开发计划：DWG/DXF 保存对话框版本标签诚实化（Milestone C）

> 起稿：2026-04-21（第十八轮，同日 DWG 解析审核 Milestone C）  
> 前置：同日 Milestone A（`OpenError` 类型化）、Milestone B（同步解析移出 iced 主循环）已落地。  
> **目标**：把 `pick_save_path` 中 8 个 DWG 版本标签 + 8 个 DXF 版本标签简化为单一标签，消除"我选了 2018 但实际按别的版本存出"的用户误导。

---

## 动机

执行中发现**比审核报告预设更严重的问题**：`native_bridge` 的两个方向都**忽略 version 字段**：

- `acadrust_doc_to_native`（line 55-106）：用 `nm::CadDocument::new()` 构造，默认 version = `R2000`，从不读取 `doc.version`。
- `native_doc_to_acadrust`（line 24-53）：用 `acadrust::CadDocument::new()` 构造，默认 version = `AC1032`，从不读取 `native.header.version`。

所以**任意读→存往返都会把文件版本重置为 `AC1032`**。打开一个真实 R2000 (AC1015) 的 DWG、另存，输出永远是 R2018 格式——这是真正的数据丢失 bug，不仅仅是审核报告里说的 "对话框标签是装饰"。

由此本 plan 的范围扩大：**一并修好双向 version 保真**。

当前 `pick_save_path` 给用户展示了 **16 个版本筛选选项**：

```text
DWG Files (2018), (2013), (2010), (2007), (2004), (2000), (R14), (R13)
DXF Files (2018), (2013), (2010), (2007), (2004), (2000), (R14), (R13)
PID Files (Smart P&ID)
```

审核发现这些标签是**装饰**：

1. `save_dwg(doc: &NativeCadDocument, path: &Path)` 调用 `DwgWriter::write_to_file(path, &acad_doc)`，`acadrust` 的写入器通过 `acad_doc.version` 字段挑选格式（AC15 / AC18 / AC21 三种 writer）。
2. `acad_doc.version` 由 `native_doc_to_acadrust` 从 native 文档桥接而来；**和 rfd 对话框里选中的筛选器无任何关联**。
3. `rfd::AsyncFileDialog::save_file()` 只返回 `Option<FileHandle>`（即 `PathBuf`），API 不暴露"用户选了哪个 filter 标签"（查阅 `vendor_tmp/rfd/src/file_dialog.rs:284`）。

所以用户在"另存为"里看到 "DWG Files (2018)" 和 "DWG Files (R14)" 8 个选项，但实际存出的版本**永远是 `doc.version` 所描述的版本**（默认新文档 AC1032；打开 R14 再另存会保持 R14）。这是典型的"菜单承诺超过实现"。

## 目标

1. 把 `pick_save_path` 的 DWG/DXF 筛选各自收敛为单项：`("DWG File", &["dwg"])`、`("DXF File", &["dxf"])`、`("PID File", &["pid"])`。
2. 在 `save_dwg` / `save_dxf` 添加内联文档注释，说明"版本来自 `doc.header.version`"的实际行为。
3. **新增**：在 `native_bridge` 两个方向都桥接 version 字段：
   - `acadrust_doc_to_native(&acad_doc)` → 从 `acad_doc.version` 映射到 `native.header.version`
   - `native_doc_to_acadrust(&native)` → 从 `native.header.version` 映射到 `doc.version`
   - 引入 `nm::DxfVersion` ↔ `acadrust::types::DxfVersion` 的双向 helper（两个 enum variant 一一对应）
4. 加一条**版本保真往返**测试：构造 acadrust 文档设 `version = AC1014`，`native_doc_to_acadrust` ↔ `acadrust_doc_to_native` 往返，断言 version 字段保留。
5. `pick_cui_save_path`、`pick_image_file`、`pick_and_open` 的 filter 策略不在本轮范围内（这些不涉及版本歧义）。

## 非目标

- **不**新增版本选择 UI（下拉 / radio）——那是 Roadmap 上的独立 feature。
- **不**改变 DWG writer 默认写入的版本（仍然是 `doc.version`）。
- **不**强制用户把 AC1032 存为 AC1015 等（没有自动降级）。
- **不**动 `load_file_native_blocking` 的 DWG 读取路径（version sniff 已经由 `acadrust` reader 正确填 `doc.version`）。

## 关键设计

### 改动 1：`src/io/mod.rs::pick_save_path`

**Before**：

```rust
pub async fn pick_save_path() -> Option<PathBuf> {
    let dwg_filters: &[(&str, &[&str])] = &[
        ("DWG Files (2018)", &["dwg"]),
        ("DWG Files (2013)", &["dwg"]),
        // ... 6 more ...
    ];
    let dxf_filters: &[(&str, &[&str])] = &[
        ("DXF Files (2018)", &["dxf"]),
        // ... 7 more ...
    ];
    let pid_filters: &[(&str, &[&str])] = &[("PID Files (Smart P&ID)", &["pid"])];
    // ...
}
```

**After**：

```rust
pub async fn pick_save_path() -> Option<PathBuf> {
    // The output version is determined by the document's in-memory
    // `version` field (sniffed from the source file on open, or the
    // acadrust default `AC1032` for fresh drawings). The save dialog
    // does NOT currently let the user override that — see the
    // `save_dwg` doc comment for the full rationale. Keep the filter
    // labels honest to avoid the "I selected 2018 but it wrote R14"
    // trap.
    rfd::AsyncFileDialog::new()
        .set_title("Save As")
        .set_file_name("drawing.dwg")
        .add_filter("DWG File", &["dwg"])
        .add_filter("DXF File", &["dxf"])
        .add_filter("PID File", &["pid"])
        .add_filter("All Files", &["*"])
        .save_file()
        .await
        .map(|h| h.path().to_path_buf())
}
```

### 改动 2：`save_dwg` / `save_dxf` 文档注释

在两个函数头加 doc comment：

```rust
/// Write the document to a DWG file.
///
/// The target DWG version is taken from `doc.header` (preserved when
/// the document was opened from an existing file, otherwise
/// `acadrust`'s default — currently `AC1032` / DXF 2018). The save
/// dialog does NOT expose a version picker today; users who need a
/// specific output version should open a template in that version,
/// then "Save As" — the in-memory version field carries through.
pub fn save_dwg(...) -> Result<(), String> { ... }
```

### 改动 3：测试

新增 `tests/dwg_save_preserves_version.rs`（集成测试）或复用 `src/io/mod.rs::tests`：

```rust
#[test]
fn save_dwg_writes_version_that_matches_document_header() {
    // Build a tiny acadrust document, force version = AC1014,
    // write to temp path, re-read, verify DwgReader's sniff
    // reports AC1014.
    use acadrust::types::DxfVersion;

    let mut doc = acadrust::CadDocument::new();
    doc.version = DxfVersion::AC1014;
    // ... minimal valid content so writer doesn't bail on empty doc ...

    let temp = std::env::temp_dir().join("h7cad-save-version-test.dwg");
    let _ = std::fs::remove_file(&temp);

    let native = crate::io::native_bridge::acadrust_doc_to_native(&doc);
    crate::io::save_dwg(&native, &temp).expect("write dwg");

    let bytes = std::fs::read(&temp).expect("read back");
    let magic = std::str::from_utf8(&bytes[..6]).expect("magic utf8");
    assert_eq!(magic, "AC1014", "dwg magic should match doc.version");

    let _ = std::fs::remove_file(&temp);
}
```

*若 acadrust 写 AC1014 最小空文档有困难（需特定 required tables），降级测试为：*

1. 跳过实际 write，只验证 `native_doc_to_acadrust` 后 `acad_doc.version` 是 `AC1032` 默认（当 native 没有 version 字段时）。
2. 或者只写一个含一条 LINE 的文档，断言 magic 正确。

本轮**以能走通的最小门**为准——如果 AC1014 空文档写入 panic / error，降级到 AC1032 默认 + magic 断言。

## 实施步骤

| 步骤 | 工作内容 | 预估 |
|---|---|---|
| M1 | 把 `pick_save_path` filter 改为三个单标签 | 5 min |
| M2 | 给 `save_dwg` / `save_dxf` 加文档注释 | 5 min |
| M3 | 写 "save_dwg 保留版本" 测试；若 AC1014 写入失败降级到 AC1032 | 15-30 min |
| M4 | `cargo test` + `cargo check --tests` + `ReadLints` | 5 min |
| M5 | CHANGELOG 二十八 | 10 min |

## 验收门

- `cargo test --bin H7CAD io::` 125+ 绿（新增 1 条）
- `cargo check --bin H7CAD --tests` 零新 warning
- `ReadLints` 改动文件零 lint
- CHANGELOG 条目存在

## 风险与退路

- **风险**：`acadrust::DwgWriter::write_to_file` 对空 / 最小文档不接受，测试写不出来。
  **退路**：测试改为只验证 `native_doc_to_acadrust(&doc).version == AC1014`（内存往返，不触文件 I/O），仍能证明桥接正确传递 version；文件 I/O 往返测试推迟到 Milestone F。
- **风险**：某些平台的 rfd 默认就显示 "All Files"，filter 没人看。
  **退路**：无影响，改动依然消除了最糟糕的"版本欺骗"case。
- **风险**：用户习惯了之前的 8 个标签，以为版本功能消失了。
  **退路**：CHANGELOG 里明确说"版本选择功能后续以下拉菜单重新引入"，把期望管好。

## 与现有 Roadmap 的关系

此 plan 是同日 DWG 审核报告的 **P0-③** 的兑现，与以下计划相容：

- `2026-04-17-acadrust-removal-plan.md`：运行时收口到 native 模型；本 plan 不动 writer，继续用 acadrust，方向一致。
- `2026-04-09-dwg-native-port-plan.md` / `2026-04-10-h7cad-runtime-native-migration.md`：native DWG 写入器就绪后，本 plan 的 filter 简化是前置友好（只留一个标签更容易替换底层实现而不产生 UI 跳动）。

后续 Milestone：

- **D** — native-dwg advisory 接入（P2-⑦）
- **E** — 显式版本选择 UI（dropdown + `save_dwg` 签名加 `Option<DxfVersion>` 参数）
- **F** — xref 同步瀑布优化
