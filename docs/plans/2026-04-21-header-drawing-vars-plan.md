# 开发计划：DXF HEADER 绘图环境变量扩充（15 → 30）

> 起稿：2026-04-21（第三轮）  
> 背景：H7CAD DXF 解析进度盘点中 HEADER 段只覆盖 15 个系统变量（几何范围 + 单位 + handseed），读真实 AutoCAD DXF 时绘图环境配置全部丢失，写回的 DXF 对这些变量用默认值，导致 UI/工具链里 "grid / snap / ortho / 当前图层 / 当前颜色" 等状态无法 round-trip。本轮补齐 **15 个最常用的绘图环境变量**，覆盖 8 成日常场景。

## 动机

DXF HEADER 段存 280+ 系统变量，AutoCAD 写出的 DXF 几乎都包含：

- 绘图模式：`$ORTHOMODE` / `$GRIDMODE` / `$SNAPMODE` / `$FILLMODE` / `$MIRRTEXT` / `$ATTMODE`
- 当前属性：`$CLAYER` / `$CECOLOR` / `$CELTYPE` / `$CELWEIGHT` / `$CELTSCALE` / `$CETRANSPARENCY`
- 角度规范：`$ANGBASE` / `$ANGDIR`
- 线型空间：`$PSLTSCALE`

当前 H7CAD 读 DXF 时这些变量被 `read_header_section` 的 `match var_name.as_str() { ... _ => {} }` 默认分支静默丢弃。写回时走默认值（`orthomode=0, gridmode=0, ...`）。结果：

- 用户在 AutoCAD 里打开 "grid on" 的 DXF，在 H7CAD 里保存后再打开 → grid 恢复 off
- 当前图层 `$CLAYER` 丢失 → 工具栏显示 `"0"` 而不是实际值
- 当前颜色 / 线型 / 线宽 默认值覆盖实际值

## 目标

扩 `DocumentHeader` 15 个字段 + reader 识别 + writer 输出 + 测试覆盖，覆盖以下变量：

| 变量 | 字段 | 类型 | 字段名 | DXF code | 默认值 |
|---|---|---|---|---|---|
| `$ORTHOMODE` | 正交模式 | bool (i16 0/1) | `orthomode` | 70 | false |
| `$GRIDMODE` | 网格显示 | bool | `gridmode` | 70 | false |
| `$SNAPMODE` | 捕捉模式 | bool | `snapmode` | 70 | false |
| `$FILLMODE` | 填充 | bool | `fillmode` | 70 | true |
| `$MIRRTEXT` | 镜像文字 | bool | `mirrtext` | 70 | false |
| `$ATTMODE` | 属性可见性 | i16 (0/1/2) | `attmode` | 70 | 1 |
| `$CLAYER` | 当前图层 | String | `clayer` | 8 | `"0"` |
| `$CECOLOR` | 当前颜色 | i16 (ACI) | `cecolor` | 62 | 256 (BYLAYER) |
| `$CELTYPE` | 当前线型 | String | `celtype` | 6 | `"ByLayer"` |
| `$CELWEIGHT` | 当前线宽 (1/100mm) | i16 | `celweight` | 370 | -1 (ByLayer) |
| `$CELTSCALE` | 当前线型比例 | f64 | `celtscale` | 40 | 1.0 |
| `$CETRANSPARENCY` | 当前透明度 | i32 | `cetransparency` | 440 | 0 |
| `$ANGBASE` | 角度基准（rad）| f64 | `angbase` | 50 | 0.0 |
| `$ANGDIR` | 角度方向 (0=逆/1=顺) | bool | `angdir` | 70 | false |
| `$PSLTSCALE` | 图纸空间线型比例 | bool | `psltscale` | 70 | true |

注：`$CECOLOR = 256` 表示 BYLAYER；`$CELWEIGHT = -1` 表示 ByLayer；`$ATTMODE` 是三态（0=off, 1=normal, 2=on），所以用 i16 不用 bool。

## 非目标

- 不扩 DIMxxx 家族（`$DIMADEC` 等 100+ 尺寸标注变量） — 独立大工作
- 不扩 `$TDCREATE / $TDUPDATE / $TDINDWG / $TDUSRTIMER` 时间戳（Julian 日期，需要 chrono 依赖或自写编解码）
- 不扩 `$UCSBASE / $UCSORG / $UCSXDIR / $UCSYDIR` UCS 家族 — 与 TABLES.UCS 联动
- 不扩渲染 / 3D / 光照相关变量（`$SHADEDGE / $LIGHTGLYPHDISPLAY` 等）
- 不扩视口几何变量（`$VIEWCTR / $VIEWSIZE / $VIEWDIR` —— 与 VPORT 表联动，独立工作）
- 不扩 `$USRTIMER / $CMLSTYLE / $CMLJUST / $CMLSCALE` 多线专用变量
- 不改变 HEADER section 的读写入口结构（沿用当前 `match var_name` 派发）

## 关键设计

### 1. Model 扩字段

`crates/h7cad-native-model/src/lib.rs::DocumentHeader` + `impl Default` 对应 15 个新字段。保持 field ordering：先几何/范围，再单位/精度，再**绘图模式**，再**当前属性**，再**角度**，再**线型空间**，handseed 放最后。

### 2. Reader 扩派发

`crates/h7cad-native-dxf/src/lib.rs::read_header_section` 的 `match var_name.as_str()` 加 15 个新 arm。每个 arm 按对应的 DXF code 提取值：

```rust
"$ORTHOMODE" => doc.header.orthomode = i16v(70) != 0,
"$CLAYER" => doc.header.clayer = sv(2).to_string(),
"$CECOLOR" => doc.header.cecolor = i16v(62),
"$CELWEIGHT" => doc.header.celweight = i16v(370),
"$CELTSCALE" => doc.header.celtscale = f(40),
"$CETRANSPARENCY" => doc.header.cetransparency = codes.iter()
    .find(|(c, _)| *c == 440).and_then(|(_, v)| v.parse().ok()).unwrap_or(0),
"$ANGBASE" => doc.header.angbase = f(50),
// ...
```

注：当前 reader 的辅助 helper `sv(c)` / `f(c)` / `i16v(c)` 已存在（见 `read_header_section` line 205-225）。对于 `$CLAYER`，DXF code 是 8（不是 2）？让我再核对 — 实际 `$CLAYER` 用 **code 8**（layer name），不是 2。需要查 AutoCAD DXF Reference 精确。

**DXF code 规范**（核对 AutoCAD DXF Reference 2018）：
- `$ORTHOMODE / $GRIDMODE / $SNAPMODE / $FILLMODE / $MIRRTEXT / $ATTMODE / $ANGDIR / $PSLTSCALE` → 所有这些都写 **code 70**
- `$CLAYER` → code **8**（layer name string）
- `$CECOLOR` → code **62**（ACI integer）
- `$CELTYPE` → code **6**（linetype name string）
- `$CELWEIGHT` → code **370** (i16)
- `$CELTSCALE` → code **40** (f64)
- `$CETRANSPARENCY` → code **440** (i32)
- `$ANGBASE` → code **50** (f64, radians)

### 3. Writer 扩输出

`crates/h7cad-native-dxf/src/writer.rs::write_header` 追加 15 个 pair 块。按 AutoCAD 输出顺序（模式 → 当前属性 → 角度 → 线型空间）排布：

```rust
w.pair_str(9, "$ORTHOMODE");
w.pair_i16(70, if doc.header.orthomode { 1 } else { 0 });

w.pair_str(9, "$CLAYER");
w.pair_str(8, &doc.header.clayer);

// ... 余下 13 个 ...
```

### 4. 测试策略

新建 `crates/h7cad-native-dxf/tests/header_drawing_vars.rs`：

1. `header_reads_all_15_drawing_vars`：手写完整 HEADER（含 15 个新变量）→ 解析 → 断言每个字段都正确读取
2. `header_writes_all_15_drawing_vars`：构造 CadDocument 填充 15 个字段 → write → 断言 text 中每个 `$VAR\n<expected_code>\n<expected_value>` 都出现
3. `header_roundtrip_preserves_all_15_drawing_vars`：read → write → read → 每个字段保持
4. `header_default_values_survive_roundtrip`：fresh `CadDocument::new()` → write → read → header 与 default 逐字段相等（保证默认值不被意外丢失）
5. `header_legacy_file_without_new_vars_loads_with_defaults`：手写 HEADER 只含旧 15 变量 → 解析 → 新字段为 default

## 实施步骤

### M1 — model 扩字段（15 min）

- `DocumentHeader` 追加 15 pub 字段
- `impl Default` 填充默认值（匹配 AutoCAD 语义：`fillmode=true, psltscale=true, cecolor=256, celweight=-1` 等）
- `cargo check -p h7cad-native-model` 过

### M2 — reader 扩派发（20 min）

- `read_header_section` 的 match 追加 15 个 arm
- `cargo check -p h7cad-native-dxf` 过

### M3 — writer 扩输出（20 min）

- `write_header` 追加 15 个 `pair_str/9 + pair_xxx/<code>` 块
- `cargo check -p h7cad-native-dxf` 过

### M4 — 集成测试（30 min）

新建 `tests/header_drawing_vars.rs`，5 条测试

### M5 — 全量测试 + CHANGELOG（10 min）

- `cargo test -p h7cad-native-dxf` 93 → 98（+5）
- `cargo test --bin H7CAD io::native_bridge` 无回归
- CHANGELOG 追加 HEADER 扩充条目
- 注意：`src/io/native_bridge.rs` 或其他 H7CAD 主 crate 代码是否直接构造 `DocumentHeader`？如有，扩字段后需要加默认值 — `cargo check -p H7CAD` 会提示

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| bridge 或 UI 代码直接构造 `DocumentHeader { ... }` 缺字段 | `cargo check -p H7CAD` 必报错 E0063；编译驱动修复 |
| DXF code 赋错（如 $CLAYER 错写 code 2 而非 8）| 单测 1 明确验证每个字段的 code → value 映射 |
| `$ATTMODE` 当成 bool 处理 | plan 已标注 i16 三态；reader 用 `i16v(70)` 直接取，不做 bool 转换 |
| `$CECOLOR` / `$CELWEIGHT` 的 BYLAYER / ByLayer / Default 默认值约定差异 | 默认值取 AutoCAD 文档精确值（256 / -1），而非 0 |
| 测试 fixture 里 $ANGBASE 的浮点比较精度 | 用 `(a - b).abs() < 1e-10` 或字符串字面量比较 |

## 验收

- `cargo test -p h7cad-native-dxf` ≥ **98** passed（93 基线 + 5 新）
- `cargo test --bin H7CAD io::native_bridge` 仍 20/20
- `cargo check -p H7CAD` 无回归（除非构造点被发现需要补默认值）
- CHANGELOG 条目

## 执行顺序

M1 → M2 → M3 → M4 → M5（严格串行；每步过 compile）
