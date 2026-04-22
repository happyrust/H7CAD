# 开发计划：DXF HEADER 杂项 5 变量扩充（插入单位 + 显示 + 编辑标志）

> 起稿：2026-04-21（第十三轮）  
> 前置：HEADER 已覆盖 56 变量（15 原有 + 15 绘图 + 4 timestamps + 5 UCS + 3 视图 + 8 DIM Tier 1 + 6 Spline+MLine）。本轮加 5 个 misc 常用变量，覆盖块插入单位语义 + 线宽显示 / 外部编辑标志。

## 动机

AutoCAD HEADER 还有几组常用变量未覆盖：

| `$` 变量 | DXF code | 含义 |
|---|---|---|
| `$INSUNITS` | 70 | Default insertion units for blocks（0=unspec, 1=in, 2=ft, 3=mi, 4=mm, 5=cm, 6=m, 7=km, 8=μin, 9=mil, 10=yd, ...） |
| `$INSUNITSDEFSOURCE` | 70 | Source content units when source is "unspecified" |
| `$INSUNITSDEFTARGET` | 70 | Target drawing units when target is "unspecified" |
| `$LWDISPLAY` | 290 | Lineweight display on/off in editor |
| `$XEDIT` | 290 | Allow external edits to this drawing as XREF |

这 5 变量在真实 AutoCAD 输出的 DXF 里普遍出现。code 290 是 bool 类型（见 `tokenizer.rs::GroupValueKind` 的 `290..=299 => Bool`），需要 reader 用单独逻辑处理（既有 `i16v(70)` 不适用）。

## 目标

1. `DocumentHeader` 扩 5 字段
2. Reader 识别 5 变量；新增 inline `bv(c)` helper 处理 code 290 bool
3. Writer 对称输出
4. 测试：read / write / roundtrip / legacy 默认（4 条）

## 字段

| 字段 | 类型 | `$` 变量 | DXF code | Default |
|---|---|---|---|---|
| `insunits` | i16 | `$INSUNITS` | 70 | 0 (unspecified) |
| `insunits_def_source` | i16 | `$INSUNITSDEFSOURCE` | 70 | 0 |
| `insunits_def_target` | i16 | `$INSUNITSDEFTARGET` | 70 | 0 |
| `lwdisplay` | bool | `$LWDISPLAY` | 290 | false |
| `xedit` | bool | `$XEDIT` | 290 | true |

## 非目标

- 不引入"语义化 InsertionUnit enum"（保留 raw i16 与 AutoCAD 数值映射，UI 层格式化）
- 不接入 `$INSUNITS` 到实际 INSERT 实体的几何缩放（独立 scope）
- 不扩 `$LWUNITS`（lineweight units，次要）
- 不扩 `$INDEXCTL` (saved sort indexing，几乎不用)

## 关键设计

### 1. Model

`DocumentHeader`（紧跟 MLine 之后 / `handseed` 之前）：

```rust
// Insertion / display / edit miscellany.
/// `$INSUNITS` (code 70): default insertion units for blocks. AutoCAD
/// values: 0=unspec, 1=in, 2=ft, 3=mi, 4=mm, 5=cm, 6=m, 7=km, 8=μin,
/// 9=mil, 10=yd, 11=Å, 12=nm, 13=μm, 14=dm, 15=dam, 16=hm, 17=Gm,
/// 18=AU, 19=ly, 20=pc. Default 0.
pub insunits: i16,
/// `$INSUNITSDEFSOURCE` (code 70): source content units when source
/// drawing unit is "unspecified". Default 0.
pub insunits_def_source: i16,
/// `$INSUNITSDEFTARGET` (code 70): target drawing units when target
/// is "unspecified". Default 0.
pub insunits_def_target: i16,
/// `$LWDISPLAY` (code 290): lineweight display on/off. Default false.
pub lwdisplay: bool,
/// `$XEDIT` (code 290): allow external edits to this drawing when
/// referenced as XREF. Default true.
pub xedit: bool,
```

### 2. Reader

`read_header_section` 加 inline helper `bv(c)` 与 `i32v` 同 scope:

```rust
let bv = |c: i16| -> bool {
    codes
        .iter()
        .find(|(code, _)| *code == c)
        .map(|(_, v)| v.trim() != "0")
        .unwrap_or(false)
};
```

5 arm：

```rust
"$INSUNITS" => doc.header.insunits = i16v(70),
"$INSUNITSDEFSOURCE" => doc.header.insunits_def_source = i16v(70),
"$INSUNITSDEFTARGET" => doc.header.insunits_def_target = i16v(70),
"$LWDISPLAY" => doc.header.lwdisplay = bv(290),
"$XEDIT" => doc.header.xedit = bv(290),
```

注意 `$XEDIT` default true 但 `bv(c)` 在缺失时返回 false——但 `bv` 只在 match arm 里被调用（即 `$XEDIT` 出现在 codes 里），所以 default 由 `DocumentHeader::default()` 保证，不依赖 reader fallback。

### 3. Writer

需要 `pair_bool(code, bool)` helper（writer.rs 中尚无）。我们直接写 i16 形式：`pair_i16(290, if b { 1 } else { 0 })`，code 290 在 DXF 中用整数 0/1 表示 bool。

实际 DXF 文本写出 `290\n     1\n` —— code 290 行 + value 行（数字）。AutoCAD 写也是这样。

```rust
// ── Insertion / display / edit miscellany ──
w.pair_str(9, "$INSUNITS");
w.pair_i16(70, doc.header.insunits);

w.pair_str(9, "$INSUNITSDEFSOURCE");
w.pair_i16(70, doc.header.insunits_def_source);

w.pair_str(9, "$INSUNITSDEFTARGET");
w.pair_i16(70, doc.header.insunits_def_target);

w.pair_str(9, "$LWDISPLAY");
w.pair_i16(290, if doc.header.lwdisplay { 1 } else { 0 });

w.pair_str(9, "$XEDIT");
w.pair_i16(290, if doc.header.xedit { 1 } else { 0 });
```

### 4. 测试

`tests/header_misc_units_display.rs`：
- `header_reads_all_5_misc_vars`
- `header_writes_all_5_misc_vars`
- `header_roundtrip_preserves_all_5_misc_vars`
- `header_legacy_file_without_misc_loads_with_defaults`

## 实施步骤

### M1 — model（5 min）

5 字段 + Default。

### M2 — reader（10 min）

加 inline `bv` helper + 5 arm。

### M3 — writer（5 min）

5 对 pair。

### M4 — 测试（15 min）

4 条集成测试。

### M5 — validator + CHANGELOG（10 min）

- `cargo test -p h7cad-native-dxf` 121 → **125** (+4)
- `cargo test --bin H7CAD io::native_bridge` 无回归
- CHANGELOG "2026-04-21（十三）"

## 风险

| 风险 | 缓解 |
|---|---|
| code 290 reader 用 i16 cast bool 可能与 AutoCAD 真实写法不一致 | DXF Reference 说 code 290 写 0/1 整数，AutoCAD 实际输出也是这样；用 i16 安全 |
| `$XEDIT` default true 但 reader fallback 给 false | 只在 match arm 里读，从未走 `bv` fallback；Default impl 保证 |
| `$INSUNITS = 0` 语义"unspecified"会被某些 AutoCAD 解析器认为"missing" | 透传不做转换，对齐 AutoCAD 自身行为 |

## 验收

- `cargo test -p h7cad-native-dxf` ≥ **125**
- `cargo test --bin H7CAD io::native_bridge` 20 / 20
- `cargo check -p H7CAD` 零新 warning
- CHANGELOG 条目

## 执行顺序

M1 → M2 → M3 → M4 → M5（严格串行）
