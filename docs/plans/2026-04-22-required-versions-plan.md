# 开发计划：i64 helper 基建 + `$REQUIREDVERSIONS` 扩充（二十四）

> 起稿：2026-04-22（第二十四轮）  
> 前置：HEADER 已覆盖 81 变量（~27%）。上一轮挂账的 `$REQUIREDVERSIONS`
> 本轮落地：reader / writer 各加一个 i64 helper，然后加 1 个字段 +
> 1 arm + 1 对 pair + 4 测试。覆盖推到 82。

## 动机

上一轮（二十三）收尾时发现 H7CAD 的 DXF io 缺 **i64 group-code**
处理能力：

- reader 有 `f` / `i16v` / `sv` / `i32v` / `bv` 五个闭包，**无 i64v**
- writer 有 `pair_str` / `pair_i16` / `pair_i32` / `pair_f64`，
  **无 `pair_i64`**

于是 code 160（int64）的 `$REQUIREDVERSIONS` 当时被迫推迟。本轮的
主题就是**补基建 + 吞入这个变量**，让 HEADER 家族的 group-code 覆盖从
{1, 2, 3, 5, 6, 7, 8, 9, 10/20/30, 40, 50, 62, 70, 90, 280, 290, 370,
420, 440} 扩到包含 **160**。

`$REQUIREDVERSIONS`（code 160）是 AutoCAD R2018+ 引入的**必需特性版本
位字段**：每一 bit 对应一个 AutoCAD 功能 / 实体类型。AutoCAD 读取
drawing 时会检查这个字段，如果任何一个 bit 代表的特性当前版本不支持，
会提示"此图纸需要更新版本 AutoCAD"。默认值 0（不强制任何特性）。

## 目标

### 基建

1. **reader** `crates/h7cad-native-dxf/src/lib.rs` 添加：
   ```rust
   let i64v = |c: i16| -> i64 {
       codes
           .iter()
           .find(|(code, _)| *code == c)
           .and_then(|(_, v)| v.parse().ok())
           .unwrap_or(0)
   };
   ```
   位置：紧跟在现有 `i32v` 闭包定义之后（保持数值类型扩展阶梯）。

2. **writer** `crates/h7cad-native-dxf/src/writer.rs` 添加：
   ```rust
   fn pair_i64(&mut self, code: i16, value: i64) {
       self.line(code.to_string().as_str());
       self.line(value.to_string().as_str());
   }
   ```
   与现有 `pair_i32` 模板完全一致，只是存储类型 `i64`。

### 字段

| 字段 | 类型 | `$` 变量 | DXF code | Default | 语义 |
|---|---|---|---|---|---|
| `required_versions` | `i64` | `$REQUIREDVERSIONS` | 160 | `0` | R2018+ 所需特性 bitfield |

## 非目标

- **不**解码 `$REQUIREDVERSIONS` 各 bit 的业务含义（由 AutoCAD 版本
  文档决定，非 io 层职责；io 纯 i64 透传）
- **不**自动根据 drawing 里出现的 entity 类型去推算应该置位哪个 bit
  （那是 writer 未来的 "自动版本需求推断" 工作，本轮只做字段 io）
- **不**为负值 `required_versions` 做特殊处理（DXF 规范定义为非负
  bitfield，但 i64 存储容纳范围远大于实际 bit 宽度）

## 关键设计

### 1. Helper（见上文）

i64v 闭包与 writer 的 `pair_i64` 成员函数必须与现有同族保持**完全一致
的代码形状**（解析失败 fallback 到 0；输出两行 pair） —— 减少 reviewer
认知成本。

### 2. Model（`crates/h7cad-native-model/src/lib.rs`）

在 Tier 3 metadata 组（`cshadow` 之后 / `chamfera` 之前）追加：

```rust
/// `$CSHADOW` (code 280): current-entity shadow mode.
pub cshadow: i16,
/// `$REQUIREDVERSIONS` (code 160): R2018+ required-feature bitfield.
/// Each bit selects an AutoCAD feature / entity type that a reader
/// must support. H7CAD treats this as an opaque `i64` passthrough;
/// mapping individual bits to features is documented by AutoCAD and
/// is not interpreted at the io layer. Default 0.
pub required_versions: i64,
```

`Default::default` 追加：`required_versions: 0`。

### 3. Reader

在 "Drawing identity and render metadata" arm 组里追加：

```rust
"$CSHADOW" => doc.header.cshadow = i16v(280),
"$REQUIREDVERSIONS" => doc.header.required_versions = i64v(160),
```

### 4. Writer

在 `$CSHADOW` pair 之后追加：

```rust
w.pair_str(9, "$CSHADOW");
w.pair_i16(280, doc.header.cshadow);

w.pair_str(9, "$REQUIREDVERSIONS");
w.pair_i64(160, doc.header.required_versions);
```

### 5. 测试（`crates/h7cad-native-dxf/tests/header_required_versions.rs`）

4 条 + 一个"大整数"对比值选择：

- 测试用值：`0x0000_1F2E_4D5C_789Au64 as i64` = 34,275,408,493,830,298。
  这个数同时覆盖：(a) 远超 i32 范围，验证 helper 不是 i32 误实现；
  (b) 位模式在高低 32 bit 都不为零，能暴露任何 "32-bit truncation"
  bug；(c) 非 0 且非最大值，验证 Default (0) 与 legacy-zero 用例区分。

- `header_reads_required_versions`：`$REQUIREDVERSIONS=<big>` → 读后
  `doc.header.required_versions == <big>`
- `header_writes_required_versions`：构造大整数 → write → 字符串含
  `$REQUIREDVERSIONS` 且紧随的 `160` 值精确 == 大整数十进制表示
- `header_roundtrip_preserves_required_versions`：read → write → read
  仍为大整数（最关键的 64-bit 保真校验）
- `header_legacy_file_without_required_versions_loads_with_zero`：
  缺省 → 0

## 实施步骤

| 步骤 | 工作内容 | 预估 |
|---|---|---|
| H | reader 加 `i64v` 闭包 + writer 加 `pair_i64` 方法 | 4 min |
| M1 | `DocumentHeader` 扩 1 字段 + Default | 2 min |
| M2 | reader 1 个 arm | 1 min |
| M3 | writer 1 对 pair | 1 min |
| M4 | 新测试文件 4 条（含大整数 ground-truth） | 8 min |
| M5 | test + check + ReadLints + CHANGELOG | 6 min |

## 验收

- `cargo test -p h7cad-native-dxf` 145 → **149** (+4)
- `cargo test --bin H7CAD io::native_bridge` 25 / 25 不受影响
- `cargo check -p H7CAD` 零新 warning
- `ReadLints` 4 个文件零 lint
- CHANGELOG "2026-04-22（二十四）" 条目存在
- HEADER 覆盖：81 → **82**
- DXF io code-group 覆盖新增 **160 (int64)**

## 风险

| 风险 | 缓解 |
|---|---|
| `i64v` 闭包被现有 reader 其他字段误用（类型推导） | reader 所有现存字段都按显式类型消费 helper 返回值，新增闭包与旧闭包并存不会触发覆盖；ReadLints 会抓任何意外类型失配 |
| AutoCAD 某些 R2018 前版本可能写出负数值 `$REQUIREDVERSIONS` | i64 容纳任何 64-bit 模式；`.parse().ok().unwrap_or(0)` 对 malformed 输入兜底 |
| 测试大整数（~3.4e16）的字符串化精度 | i64 转 `String` 用 `to_string()` 精确；测试用 `assert_eq!` 数值比较（而非浮点 `<1e-9`） |

## 执行顺序

H → M1 → M2 → M3 → M4 → M5 → commit（严格串行）
