# 开发计划：Drawing 元数据附加 4 变量扩充（二十七）

> 起稿：2026-04-22（第二十七轮）
> 前置：HEADER 已覆盖 93 变量（~31%）。上一轮二十六补完显示 & 渲染
> 家族 5 变量。本轮走 plan §9 下一轮候选里最短的"drawing 元数据附加"
> 4 变量路径。
>
> 类型组合首次混搭：1 × `i16` + 2 × `String` + 1 × `bool`，比之前几轮
> 的"整组同类型"更全面地覆盖 3 个 helper（`i16v` / `sv` / `bv`）。

## 动机

二十三轮已落地过一次"drawing 元数据族"（`$FINGERPRINTGUID`、
`$VERSIONGUID`、`$DWGCODEPAGE`、`$CSHADOW`），覆盖了"身份 +
代码页 + 渲染 flag"四项。但 AutoCAD 还有**另外 4 个 drawing 级**
元数据变量属于同一认知维度，二十三轮当时没吞入：

| 变量 | 为什么与身份/代码页同属一类 |
|------|--------------------------|
| `$PROJECTNAME` | 标识该 drawing 所属项目；影响 XREF / raster 路径解析 |
| `$HYPERLINKBASE` | drawing 内相对 hyperlink 的根 URL；路径解析前缀 |
| `$INDEXCTL` | drawing 级 layer / spatial 索引生成控制位 |
| `$OLESTARTUP` | 打开 drawing 时是否加载 OLE 对象 |

4 个都是 drawing 打开时 AutoCAD 立即读取的"环境偏好"，和二十三轮
的 身份 GUID / 代码页 属于同一代工作负载。本轮补齐后，drawing 级
元数据将达到 8 个变量的稳定规模。

## 目标

### 字段

| 字段 | 类型 | `$` 变量 | DXF code | Default | 语义 |
|---|---|---|---|---|---|
| `project_name` | `String` | `$PROJECTNAME` | 1 | `""` | 所属项目名；XREF / raster 路径解析时用来命中 `ProjectFilePath` 子目录 |
| `hyperlink_base` | `String` | `$HYPERLINKBASE` | 1 | `""` | drawing 内所有相对 hyperlink 的根 URL / 路径 |
| `indexctl` | `i16` | `$INDEXCTL` | 70 | `0` | bitfield：0 = 不建索引 / 1 = layer index / 2 = spatial index / 3 = 两者都建 |
| `olestartup` | `bool` | `$OLESTARTUP` | 290 | `false` | 打开 drawing 时是否启动 OLE 对象 |

插入位置：`DocumentHeader` 里紧跟二十三轮的 drawing 身份 / 渲染组
（`$REQUIREDVERSIONS` 之后、`$CHAMFERA` 之前），形成"身份 GUID 对 +
代码页 + shadow + required_versions + project_name + hyperlink_base +
indexctl + olestartup"**8 字段连贯块**。

### 默认值选型

- `project_name = ""` / `hyperlink_base = ""` —— 与 `fingerprint_guid`
  / `version_guid` 默认相同，"空串 = 未设置"，passthrough 到 UI
- `indexctl = 0` —— AutoCAD 出厂默认（不创建索引，最省空间）
- `olestartup = false` —— AutoCAD 出厂默认（不预加载 OLE，最省启动
  时间）

全部对齐 AutoCAD factory default；与既有 `lwdisplay: false` /
`xedit: true` 同一逻辑维度。

## 非目标

- **不**校验 `project_name` / `hyperlink_base` 是合法路径或 URL —— io
  层 passthrough，路径归一化 / 合法性检查是 UI / 命令层职责
- **不**解析 `indexctl` 的各 bit 为独立 `bool`（AutoCAD 文档明确这
  是 bitfield，但 UI 自己拆；io 保持 `i16` 原子存储）
- **不**修改二十三轮已落地的 4 个变量（仅在它们之后追加）
- **不**触碰 DWG 侧 / `real_dwg_samples_baseline_m3b` 红灯

## 关键设计

### 1. Model（`crates/h7cad-native-model/src/lib.rs`）

紧跟 `required_versions` 之后、`chamfera` 之前，追加 4 字段连续声明：

```rust
pub required_versions: i64,

/// `$PROJECTNAME` (code 1): project name for this drawing. AutoCAD
/// uses it to pick a `ProjectFilePath` subdir when resolving XREF /
/// raster image paths. Default empty — io layer only passes the value
/// through; path resolution is a command-layer concern.
pub project_name: String,
/// `$HYPERLINKBASE` (code 1): base URL / path for all relative
/// hyperlinks embedded in the drawing. Default empty (no base).
pub hyperlink_base: String,
/// `$INDEXCTL` (code 70): layer / spatial index creation bitfield.
/// bit 0 = layer index, bit 1 = spatial index. Default 0 (no indexes
/// created — the most compact drawing). io stores the raw i16;
/// decoding individual bits is a UI / command-layer concern.
pub indexctl: i16,
/// `$OLESTARTUP` (code 290): on-open behaviour for OLE objects.
/// `false` = don't start OLE application when opening drawing
/// (default, faster); `true` = pre-start. No effect on drawing
/// content itself — purely startup hint.
pub olestartup: bool,

// Interactive geometry command defaults.
/// `$CHAMFERA` (code 40): first chamfer distance. Default 0.0.
pub chamfera: f64,
```

`Default::default()` 同步追加（接在 `required_versions: 0,` 之后）：

```rust
required_versions: 0,

project_name: String::new(),
hyperlink_base: String::new(),
indexctl: 0,
olestartup: false,

chamfera: 0.0,
```

### 2. Reader（`crates/h7cad-native-dxf/src/lib.rs`）

在二十四轮的 `$REQUIREDVERSIONS` arm 之后（`$CHAMFERA` 之前的
"Interactive geometry command defaults" arm 组之前），追加 4 arm：

```rust
"$REQUIREDVERSIONS" => doc.header.required_versions = i64v(160),

"$PROJECTNAME" => doc.header.project_name = sv(1).to_string(),
"$HYPERLINKBASE" => doc.header.hyperlink_base = sv(1).to_string(),
"$INDEXCTL" => doc.header.indexctl = i16v(70),
"$OLESTARTUP" => doc.header.olestartup = bv(290),
```

### 3. Writer（`crates/h7cad-native-dxf/src/writer.rs`）

在 `$REQUIREDVERSIONS` pair 之后、`$CHAMFERA` pair 之前，追加 4 段：

```rust
w.pair_str(9, "$REQUIREDVERSIONS");
w.pair_i64(160, doc.header.required_versions);

w.pair_str(9, "$PROJECTNAME");
w.pair_str(1, &doc.header.project_name);

w.pair_str(9, "$HYPERLINKBASE");
w.pair_str(1, &doc.header.hyperlink_base);

w.pair_str(9, "$INDEXCTL");
w.pair_i16(70, doc.header.indexctl);

w.pair_str(9, "$OLESTARTUP");
w.pair_i16(290, if doc.header.olestartup { 1 } else { 0 });

w.pair_str(9, "$CHAMFERA");
```

### 4. 测试（`crates/h7cad-native-dxf/tests/header_drawing_metadata_addendum.rs`）

4 条，与之前"per-家族"测试风格一致：

1. `header_reads_drawing_metadata_addendum_family` — 4 字段非默认值
   全部精确恢复；额外断言 `project_name != hyperlink_base`（两都是
   code 1 String — arm 串位会导致它们互串，这条 assert 是最直接的
   regression guard）
2. `header_writes_drawing_metadata_addendum_family` — 4 个 `$VAR`
   按 reader arm 顺序出现 + 值精确匹配；验证 `olestartup: true` 的
   bool → "1" 的 i16-as-bool 写入路径
3. `header_roundtrip_preserves_drawing_metadata_addendum_family` —
   read → write → read 4 字段 bit-identical；String 里刻意塞 Unicode
   （中日俄混合）以防任何编码路径 drift
4. `header_legacy_file_without_drawing_metadata_addendum_loads_with_defaults` —
   legacy HEADER 无这 4 变量 → 读出命中默认（空串 / 0 / 0 / false）

### 5. Ground-truth 值选择

两个 String 值刻意**互不相等**且**内容能暴露编码 bug**：

- `project_name = "my-proj/sub-dir 项目 α"` — 含空格、斜杠、中文、希腊字母
- `hyperlink_base = "https://example.com/docs/日本語/"` — 含协议、路径、日文
- `indexctl = 3`（≠ default 0） — 两 bit 同时置位
- `olestartup = true`（≠ default false）

String 里包含 Unicode 字符即可顺便验证 DXF reader 的 UTF-8 / ANSI
codepage 分叉在 HEADER 文本字段上没有 drift。

## 实施步骤

| 步骤 | 工作内容 | 预估 |
|---|---|---|
| M1 | `DocumentHeader` 扩 4 字段 + Default | 3 min |
| M2 | reader 4 arm | 2 min |
| M3 | writer 4 对 pair | 2 min |
| M4 | 新测试文件 4 条（含 Unicode ground-truth） | 10 min |
| M5 | `cargo test -p h7cad-native-dxf` 全绿 | 1 min |
| M6 | `cargo check -p H7CAD`、`-p h7cad-native-facade` | 2 min |
| M7 | `ReadLints` + CHANGELOG "二十七" 条目 | 5 min |

## 验收

- `cargo test -p h7cad-native-dxf` **157 → 161**（+4 header_drawing_metadata_addendum）
- `cargo test --bin H7CAD io::native_bridge` 25 / 25 不受影响
- `cargo check -p H7CAD` 零新 warning
- `cargo check -p h7cad-native-facade` 零新 warning
- `ReadLints` 改动的 4 个文件零 lint
- CHANGELOG 存在 "2026-04-22（二十七）" 条目
- HEADER 覆盖：93 → **97**
- 本 plan §状态 写 "落地完成"

## 风险

| 风险 | 缓解 |
|---|---|
| `$PROJECTNAME` / `$HYPERLINKBASE` 都用 code 1，reader arm 串位时会把 hyperlink URL 错塞进 project_name | 测试用例里两 String 非空且字面不同，arm 串位 assert 立刻挂 |
| String 里的 Unicode 经 DXF ANSI codepage 路径可能 drift | 测试用 `read_dxf(&str)` 入口而非 `read_dxf_bytes(&[u8])`，跳过 codepage 侦测；专门的 encoding roundtrip 测试不在本 plan 范围 |
| `$OLESTARTUP` writer 用 `pair_i16(290, …)` 写入 AutoCAD 文档规定的 code 290 bool，与 `lwdisplay / xedit` 同模式；历史选型 OK | 参照既有 `$LWDISPLAY / $XEDIT` 同款 pattern，不新发明 |

## 执行顺序

M1 → M2 → M3 → M4 → M5 → M6 → M7 → commit（严格串行）

## 下一轮候选（二十八）

本轮用完"元数据附加" 4 变量后，plan §9 原先列的 3 候选还剩两个：

1. `$LOFTANG1 / $LOFTANG2 / $LOFTMAG1 / $LOFTMAG2 / $LOFTNORMALS /
   $LOFTPARAM` — 6 变量，Loft 3D 默认（混 f64 / i16）
2. `$DIMALT / $DIMALTD / $DIMALTF / $DIMALTRND / $DIMALTTD / $DIMALTTZ
   / $DIMALTU / $DIMALTZ / $DIMAPOST` — 9 变量，DIM 替代单位家族
   （中等规模，需引入 `dim_alt_*` 子组）

推荐顺序：**1 → 2**（按规模；Loft 6 变量是自然下一跳，DIMALT 9 变量
是中期目标）。

## 状态

- [x] 计划定稿（本文件）
- [x] M1 DocumentHeader 4 字段 + Default
- [x] M2 reader 4 arm
- [x] M3 writer 4 对 pair
- [x] M4 新测试文件 4 条（含 Unicode ground-truth）
- [x] M5 `cargo test -p h7cad-native-dxf` 157 → 161 全绿
- [x] M6 `cargo check -p H7CAD` / `-p h7cad-native-facade` 零新 warning；`cargo test --bin H7CAD io::native_bridge` 25 / 25
- [x] M7 CHANGELOG "2026-04-22（二十七）" 落地 + ReadLints 零 lint
