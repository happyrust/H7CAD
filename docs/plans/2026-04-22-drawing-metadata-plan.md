# 开发计划：DXF HEADER Tier 3 表头元数据扩充（二十三）

> 起稿：2026-04-22（第二十三轮）  
> 前置：HEADER 已覆盖 77 变量（~26%）。本轮补 **4 个表头元数据**：
> drawing 标识 GUID 对 + 代码页 + 当前实体 shadow flag。
> 覆盖推到 81。

## 动机

AutoCAD HEADER 里有一组“表头元数据”变量，不描述任何几何或样式，而是
携带图纸的身份/版本/渲染属性。H7CAD 当前 reader/writer 全部忽略，读
AutoCAD .dxf 再 roundtrip 回写后这些信息会静默归零：

1. **`$FINGERPRINTGUID`**（code 2 string）：图纸的**永久 GUID**，在图纸
   首次创建时生成，此后任意 save / copy / rename 都**不变**。用于跨
   AutoCAD 会话追踪“这是同一份图纸”。
2. **`$VERSIONGUID`**（code 2 string）：**每次 save 都更新**的 GUID，
   用于区分“同一份图纸的不同版本”。两张 fingerprint 相同但 version
   不同的图纸 = 同一份图的两个历史版本。
3. **`$DWGCODEPAGE`**（code 3 string）：图纸字符编码代码页。R2000–R2006
   时期重要（用于解读 ANSI / Shift-JIS 等），R2007+ 文件已迁 UTF-8 但
   AutoCAD 仍继续写出此字段兼容旧 reader，典型值 `"ANSI_1252"`。
4. **`$CSHADOW`**（code 280 i16）：**当前实体**默认的阴影投射 / 接收
   模式，范围 0–3：
   - 0 = 既投射也接收阴影（AutoCAD 默认）
   - 1 = 仅投射阴影
   - 2 = 仅接收阴影
   - 3 = 忽略阴影

## 目标字段

| 字段 | 类型 | `$` 变量 | DXF code | Default | 语义 |
|---|---|---|---|---|---|
| `fingerprint_guid` | `String` | `$FINGERPRINTGUID` | 2 | `""` | 永久 GUID |
| `version_guid` | `String` | `$VERSIONGUID` | 2 | `""` | 版本 GUID |
| `dwg_codepage` | `String` | `$DWGCODEPAGE` | 3 | `""` | 字符编码代码页 |
| `cshadow` | `i16` | `$CSHADOW` | 280 | `0` | 当前实体 shadow 模式 |

GUID 字段 Default 选 `""`（而非随机新建）：io 层**纯透传**，不承担
身份生成职责；上层 UI / 命令层在“真正新建图纸”时才写入合法 GUID，
否则保持空串让下游自己决定。

## 非目标

- **不**在 io 层自动生成 GUID（身份创建是命令层责任）
- **不**解析 `$DWGCODEPAGE` 的值做字符集转换（reader / writer 当前已经
  按 UTF-8 处理全文；codepage 仅是元数据透传）
- **不**实现 `$REQUIREDVERSIONS`（code 160 i64）——reader / writer 缺
  `pair_i64` helper，下一轮先加 helper 再一并吞入
- **不**承担 shadow 渲染逻辑（只透传 `$CSHADOW` flag）

## 关键设计

### 1. Model（`crates/h7cad-native-model/src/lib.rs`）

在 `DocumentHeader` 的 miscellany 组（`xedit` 之后、`chamfera` 之前）
插入 4 字段：

```rust
/// `$XEDIT` (code 290): allow external edits to this drawing when
/// referenced as XREF. Default true.
pub xedit: bool,

// Drawing identity and render metadata.
/// `$FINGERPRINTGUID` (code 2): permanent drawing GUID stamped at
/// creation time; unchanged across save / copy / rename. Default
/// empty — io layer only passes the value through; generating a
/// fresh GUID for brand-new drawings is a command-layer concern.
pub fingerprint_guid: String,
/// `$VERSIONGUID` (code 2): per-save GUID — updated every time the
/// drawing is written. Default empty (same passthrough policy).
pub version_guid: String,
/// `$DWGCODEPAGE` (code 3): drawing character code page. Legacy
/// field from R2000–R2006 (ANSI_* families); AutoCAD R2007+ writes
/// UTF-8 on disk but still emits this for backward compatibility
/// (commonly `"ANSI_1252"`). Default empty.
pub dwg_codepage: String,
/// `$CSHADOW` (code 280): current-entity shadow mode.
/// 0 = casts and receives shadows (default);
/// 1 = casts only; 2 = receives only; 3 = ignores shadows.
pub cshadow: i16,

// Interactive geometry command defaults.
pub chamfera: f64,
```

`Default::default` 追加：`fingerprint_guid: String::new(),
version_guid: String::new(), dwg_codepage: String::new(), cshadow: 0`。

### 2. Reader（`crates/h7cad-native-dxf/src/lib.rs`）

在 `"$XEDIT" => ...` 之后、`"$CHAMFERA" => ...` 之前追加：

```rust
// Drawing identity and render metadata.
"$FINGERPRINTGUID" => doc.header.fingerprint_guid = sv(2).to_string(),
"$VERSIONGUID" => doc.header.version_guid = sv(2).to_string(),
"$DWGCODEPAGE" => doc.header.dwg_codepage = sv(3).to_string(),
"$CSHADOW" => doc.header.cshadow = i16v(280),
```

### 3. Writer（`crates/h7cad-native-dxf/src/writer.rs`）

在 `$XEDIT` pair 之后追加：

```rust
w.pair_str(9, "$XEDIT");
w.pair_i16(290, if doc.header.xedit { 1 } else { 0 });

// ── Drawing identity and render metadata ──────────────────────────────
w.pair_str(9, "$FINGERPRINTGUID");
w.pair_str(2, &doc.header.fingerprint_guid);

w.pair_str(9, "$VERSIONGUID");
w.pair_str(2, &doc.header.version_guid);

w.pair_str(9, "$DWGCODEPAGE");
w.pair_str(3, &doc.header.dwg_codepage);

w.pair_str(9, "$CSHADOW");
w.pair_i16(280, doc.header.cshadow);

// ── Interactive geometry command defaults ─────────────────────────────
```

### 4. 测试（`crates/h7cad-native-dxf/tests/header_drawing_metadata.rs`）

4 条：

- `header_reads_all_4_drawing_metadata_vars`：GUID 字符串 + codepage +
  cshadow 精确读入
- `header_writes_all_4_drawing_metadata_vars`：构造 → write → 4 个
  `$VAR` 字符串全在
- `header_roundtrip_preserves_all_4_drawing_metadata_vars`：read →
  write → read 全字段完全保持（字符串精确相等）
- `header_legacy_file_without_drawing_metadata_loads_with_defaults`：
  缺省 → GUID / codepage 空串，cshadow = 0

## 实施步骤

| 步骤 | 工作内容 | 预估 |
|---|---|---|
| M1 | `DocumentHeader` 扩 4 字段 + Default | 5 min |
| M2 | reader 4 个 arm | 2 min |
| M3 | writer 4 对 pair | 3 min |
| M4 | 新测试文件 4 条 | 10 min |
| M5 | test + check + ReadLints + CHANGELOG | 8 min |

## 验收

- `cargo test -p h7cad-native-dxf` 141 → **145** (+4)
- `cargo test --bin H7CAD io::native_bridge` 25 / 25 不受影响
- `cargo check -p H7CAD` 零新 warning
- `ReadLints` 4 个文件零 lint
- CHANGELOG "2026-04-22（二十三）" 条目存在
- HEADER 覆盖：77 → **81**

## 风险

| 风险 | 缓解 |
|---|---|
| `$FINGERPRINTGUID` / `$VERSIONGUID` 用同一 code 2，读 header 时需按 `$VAR` 名字区分 | reader 是按 `var_name` 分支 match，code 2 helper (`sv(2)`) 在同一 arm 内各取各的，天然隔离 |
| `$DWGCODEPAGE` 含大小写敏感字符（`ANSI_1252`） | `sv(3)` 返回 trim 后字符串字面量，不做 case fold，与 AutoCAD 保持一致 |
| `$CSHADOW` 未来 AutoCAD 可能增加 flag bit | i16 存储容量 >> 4 个值，向前兼容 |

## 执行顺序

M1 → M2 → M3 → M4 → M5 → commit（严格串行）
