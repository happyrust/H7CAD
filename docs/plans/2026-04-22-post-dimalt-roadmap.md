# Post-DIMALT 路线图：四路径 ROI 评估（三十轮）

> 起稿：2026-04-22（第三十轮）
> 前置：二十九轮完成 DIMALT 家族 9 变量，HEADER 覆盖 112 变量（~37%）；
> plan §9 下一轮候选队列已清空；全链路绿。
> 目的：**本文件不是一个普通"某家族 N 变量"的 micro-iteration plan**；
> 它是二十九轮后续**战略选择**的决策文档，给 owner 选方向。

## 1. 背景与动机

二十五轮到二十九轮（5 轮 / 1 日）产出：

| 指标 | 二十四轮末 | 二十九轮末 | 增量 |
|------|----------|----------|------|
| HEADER 覆盖 | 82 | 112 | +30（~37%） |
| 新增 plan 文件 | 19 | 24 | +5 |
| DXF 测试数 | 149 | 169 | +20 |
| DIM 子系统字段 | 15 | 24 | +9 |
| io::native_bridge 测试 | 25 / 25 | 25 / 25 | 恒定 |
| lint 违规 | 0 | 0 | 恒定 |
| 其他链路（H7CAD / facade） | 绿 | 绿 | 恒定 |
| 副产品 | — | `format_f64` shortest round-trip 精度升级 | — |

但 plan §9 每次都留的"下一轮候选"队列现在**真的空了**，而 HEADER
覆盖 37% 的数字背后是：**剩下的 ~188 个 HEADER 变量多为 R2018+
新增 / 特定场景 flag / 几乎无人用的 legacy**，边际价值快速下降。
继续按 "一天一轮 5 vars" 节奏跑 30 轮也只是把数字从 37% 刷到 ~70%，
产品价值很薄。

该到决策岔路口了。

## 2. 四条候选路径全景

### 路径 A · HEADER 长尾扩展

**做什么**：继续按现有模板每轮 3-5 变量扩充。候选家族：

- `$DIMJUST / DIMAUNIT / DIMAZIN / DIMSOXD / DIMSD1 / DIMSD2 / DIMSE1 / DIMSE2 / DIMFIT / DIMATFIT` — DIM 视觉控制（10 vars）
- `$DIMBLK / DIMBLK1 / DIMBLK2 / DIMLDRBLK / DIMARCSYM / DIMJOGANG` — DIM 箭头 / 符号（6 vars）
- `$PSTYLEMODE / TILEMODE / MAXACTVP / PSVPSCALE` — paper space（4 vars）
- `$SURFTAB1 / SURFTAB2 / SURFTYPE / SURFU / SURFV / PFACEVMAX` — 3D 表面（6 vars）
- `$CEPSNID / CEPSNTYPE / PLOTSTYLE / STYLESHEET` — plot style（4 vars；部分存在性待核）
- `$TREEDEPTH / VISRETAIN / DELOBJ / PROXYGRAPHICS` — 杂项 flag（4 vars）

**ROI**：
- ✅ 工程模式稳定，每轮 20-35 min；与二十五到二十九轮同质
- ✅ 零回归风险；测试 / lint / check 自动守门
- ✅ 有长尾边际价值（真实 AutoCAD 文件偶尔会触碰罕用变量）
- ❌ 产品价值递减 —— 绝大多数用户永远不启用这些特性
- ❌ 继续 20 轮才把 HEADER 刷到 80%，30 轮才 90%
- ❌ 机会成本：挤占了 entity / DWG 这种**上层直接可见**的改进窗口

**Effort**：20 min/轮 × 30 轮 = 10 小时。
**Risk**：极低。
**Impact**：HEADER 覆盖从 37% → ~80%，真实工作流几乎无感。

### 路径 B · Entity 覆盖扩展

**做什么**：当前 `EntityData` 40 变体，AutoCAD 完整 ~100 种。缺失
的重要实体：

- **常见但未覆盖**：`OLE2FRAME`（OLE 对象嵌入）、`ACAD_PROXY_ENTITY`
  （第三方 ObjectARX 代理实体）、`BODY`（ACIS body）、`GEOPOSITIONMARKER`
  （地理位置标记）—— 4 个
- **中等重要**：`MPOLYGON`（多边形填充）、`MATERIAL`（材质引用实体）、
  `WIPEOUT` 已覆盖但 roundtrip 保真性待核实、`FIELD`（字段对象引用）—— 4 个
- **罕用**：`LOFTEDSURFACE`（与 Helix / Mesh 并列）、`SWEPTSURFACE`、
  `REVOLVEDSURFACE`、`EXTRUDEDSURFACE`、`PLANESURFACE`、`NURBSURFACE`
  —— 6 个

每个实体 = **parse_xxx + write_xxx arm + model EntityData variant +
integration test**，单实体工作量与单家族 4 vars 相当（20-40 min）。

**ROI**：
- ✅ **直接提升 roundtrip 保真度** —— 碰到这些实体的真实 AutoCAD 图纸
  现在会落入 `EntityData::Unknown`，变 UI 不可见
- ✅ 可快速演进到 50 变体（+10），覆盖 AutoCAD 常见场景 ~95%
- ✅ 每 entity 独立 mergeable，不互阻塞
- ⚠ OLE2FRAME / ACAD_PROXY_ENTITY 数据里有嵌入 binary（比 f64 复杂）
- ❌ 部分实体（Mesh, Surface, Solid3D）已存在但 `acis_data: String` 无效 parse —— 需重构
- ❌ 测试套数据要 hand-crafted（不像 HEADER 一串 `{var}\n{val}\n`）

**Effort**：30-60 min/entity × 10 entities = 5-10 小时。
**Risk**：中。ACIS binary 处理 / OLE frame 布局学习曲线。
**Impact**：真实 roundtrip 保真度提升最直接的路径。

### 路径 C · 修复 DWG 红灯

**做什么**：`crates/h7cad-native-dwg` 的 `real_dwg_samples_baseline_m3b`
目前挂在 "sample_AC1015.dwg: AC1015 baseline must recover at least 40
LINE entities, got 26"。诊断报告显示：

```
AC1015 recovery failure buckets:
  slice_miss=212  header_fail=166  handle_mismatch=2
  common_decode_fail=236  body_decode_fail=177  unsupported_type=254
family=LINE   body_decode_fail=82  (0x2C7 / 0x2CF / 0x517 ...)
family=POINT  body_decode_fail=34
family=CIRCLE body_decode_fail=9
family=ARC    body_decode_fail=3
family=TEXT   body_decode_fail=26
family=LWPOLYLINE body_decode_fail=17
family=HATCH  body_decode_fail=6
```

即 **AC1015 entity body decode** 在 7 个已实现 family 里都有失败，
LINE 一家独挂 82 次。这需要：

1. 挑 3 个失败 LINE handle（0x2C7 / 0x2CF / 0x517）hexdump + probe
2. 与 `entity_line::read_line_geometry` 的 bit-stream parser 比对
3. 找出 bit 布局差异（可能是新字段 / 对齐错误 / 可选字段的条件错读）
4. 修复 → 回归 → 把 LINE 成功数从 26 → 40+
5. 同法覆盖 CIRCLE / ARC / POINT / TEXT / LWPOLYLINE / HATCH 的 body_decode_fail

**ROI**：
- ✅✅ **上层 product 价值最高** —— 修复了就能在 facade 把 DWG load
  从 `"not implemented"` → 实际返回 `CadDocument`，相当于**开箱即用地
  多支持一种主流格式**
- ✅ `real_dwg_samples_baseline_m3b` 从红变绿，整个 dwg suite 恢复
  零红灯
- ⚠ 需要 hexdump / CDP 风格的 bit-level 调试，与我之前 HEADER 工作
  完全不同技术栈
- ⚠ sample_AC1015.dwg 是二进制文件（不在 git 历史里看不到），需
  `cargo test --release -- --nocapture` 配合诊断输出迭代
- ❌ 可能发现 AC1015 spec 本身的歧义 / OpenDWG 文档不全，卡很久
- ❌ 修复 LINE 后还有 6 个 family 的 `body_decode_fail`，工作量未知

**Effort**：2-5 小时（LINE 单 entity）+ 后续每 family 1-3 小时。
**Risk**：高。bit-parser 调试容易掉坑。
**Impact**：facade 可发布 DWG 读支持（README 里当前写"intentionally
unavailable"的那一段可以撕了）。

### 路径 D · DXF 读取保真度 fuzz

**做什么**：二十五轮给 writer 加了 shortest round-trip `format_f64`，
write 侧保真已证明。但 **reader 的 robustness** 未被系统测过：

- 超长 f64（科学记数 `1.23e-300`）读回是否准确？
- f64 写出形式 `1e100` vs `1.0e100` 是否都 parse？
- 1.234567890123456789（17 位有效数字）roundtrip 是否 bit-identical？
- HEADER 里罕用字段意外读到 NaN / Inf / -0 时 reader 行为？
- 边界坐标 (`f64::MAX` / `f64::MIN_POSITIVE`) 是否 overflow？
- 多 UTF-8 字符混合（`dim_apost = "①②③"`）是否 parse？
- 文件被截断在 HEADER 中间是否优雅报错？

输出：
1. `tests/roundtrip_fuzz.rs` — `quickcheck` 或 `proptest` 随机生成
   `DocumentHeader` → 写 → 读 → 比对；≥ 10 个 property：
   `forall doc: header_roundtrip_is_identical(doc)`
2. 发现任何 drift → 修 reader / writer / model 对应模块
3. 顺带补 `tests/edge_f64.rs` —— 一组手写 edge case（NaN / Inf /
   subnormal / -0）的断言测试

**ROI**：
- ✅ 发现 bug 就是高价值（会是真实 drift 的长尾）
- ✅ 随 property-based testing 的覆盖持续增强
- ❌ **bug 很可能 0 个**（`format_f64` shortest + `.parse::<f64>()`
  是已证明的 pair），投入回报比可能很低
- ❌ `quickcheck` / `proptest` 依赖引入会扩 Cargo.toml

**Effort**：2-4 小时（装框架 + 写 10 properties + 跑 fuzz 几小时定
种子）。
**Risk**：低。
**Impact**：中等偏低（概率性发现隐性 bug）。

### 路径 E · 工具打磨（意外发现的 small wins）

**做什么**：几个**可以 30 min 解决**的小事：

1. **`write_dxf` 错误类型升级**：当前签名
   `pub fn write_dxf(doc: &CadDocument) -> Result<String, String>`——
   错误用裸 `String`，下游只能 `eprintln!`，无法 pattern match。
   改成 `Result<String, DxfWriteError>` 的结构化 enum（对齐读侧
   `DxfReadError` 的设计）。对库调用方（如 `arbor`, `facade` /
   `H7CAD` CLI）是 **contract 改进**，不破坏 API 兼容（`String::from(err)`
   还能 `.to_string()`）。
   - Impact: ++ 下游消费者可精确处理 `IoError / Unsupported / …`
   - Risk: 低。DxfReadError 对齐已证明。
   - 时间: 30-45 min。
2. **RASTER_IMAGE / Wipeout 实体 roundtrip 断言加强**：当前
   `imagedef_roundtrip.rs` 9 条测试覆盖了 IMAGE-IMAGEDEF link，但
   Wipeout 的 roundtrip 是否保真未被专门验证。
   - Impact: + 堵一个潜在的实体 roundtrip bug
   - Risk: 低
   - 时间: 20 min
3. **examples/open_dxf.rs 增强** —— 目前只输出 entity 类型计数。可
   额外输出 HEADER 关键字段统计 + roundtrip verify（`read → write →
   read` 自动断言）作为一个可执行的 CLI smoke test。
   - Impact: + 开发阶段的 CLI 验证工具
   - Risk: 极低
   - 时间: 20 min

**ROI**：小但高确定性。单次 30-45 min 获得持久的 contract / 测试 /
开发体验改进。

## 3. 决策矩阵

| 维度 | A HEADER 长尾 | B Entity 扩 | C DWG 红灯 | D Fuzz 保真 | E 工具打磨 |
|---|---|---|---|---|---|
| 每轮 effort | 20 min | 30-60 min | 2-5 h | 2-4 h | 30-45 min |
| 风险 | 极低 | 中 | 高 | 低 | 极低 |
| 产品价值 | 低 | 高 | 极高 | 中 | 中 |
| 可持续度（多轮） | 高（20+ 轮） | 高（10 轮） | 中（3-5 轮） | 低（1-2 轮） | 低（3 轮） |
| 被卡死风险 | 0 | 小 | 大 | 小 | 0 |
| 知识新增 | 少 | 中 | 多（DWG bit parser） | 少 | 少 |
| 前置成本 | 0 | 读 AutoCAD entity ref | 学 AC1015 spec + hexdump | 装 proptest | 0 |

## 4. 推荐路线

**Phase 1（本轮立刻做）**：路径 E-1（`write_dxf` 错误类型升级）。

理由：
- 最短（30-45 min）+ 最低风险
- 真实改进 library contract，下游 `arbor` / `facade` 立刻受益
- 为后续任何路径（尤其是 B entity 扩展）铺路 —— 写入新 entity 时
  fail fast 的错误类型远比 `Err("something wrong")` 好
- 不阻塞策略讨论 —— 写完这个小改进，owner 仍有充分时间为 B/C/D 做
  决定

**Phase 2（下一会话选做）**：路径 B 第一步 —— 挑一个 entity 变体
做完整 reader + writer + test（推荐 `OLE2FRAME` 或 `GEOPOSITIONMARKER`
这种 "模型简单但目前为 Unknown" 的实体）。

**Phase 3（战略级）**：路径 C —— 一旦 B 跑 10 轮把 entity 覆盖刷到
50+，再转 DWG 红灯。届时：
- 项目已有 "DXF 写出任意实体" 的完整链
- 可以用 DXF 自动化生成 DWG test fixture（避免依赖不可控的第三方
  sample）
- 修 DWG 得到的 LINE body decode 也能复用在 DXF 正交验证

**Phase 4（长尾）**：路径 A + D，并行推进（A 每轮 20 min，D 每 sprint
加一组 property）。

## 5. 本轮立刻执行的 Phase 1：write_dxf 错误类型升级

### 5.1 目标

把 `pub fn write_dxf(doc) -> Result<String, String>` 升级到：

```rust
pub fn write_dxf(doc: &CadDocument) -> Result<String, DxfWriteError>
```

新 enum 放在 `crates/h7cad-native-dxf/src/writer.rs`：

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DxfWriteError {
    /// The document state is internally inconsistent and cannot be
    /// serialised (e.g. an IMAGE entity points to an IMAGEDEF handle
    /// that does not exist in `doc.objects`).
    InvalidDocument(String),
    /// A feature is not yet implemented in the writer
    /// (e.g. binary DXF output).
    Unsupported(String),
    /// An underlying I/O-like formatting error from `std::fmt::Write`.
    /// Currently unreachable in practice (we write to `String`), but
    /// reserved so future `&mut dyn Write` overloads can surface real
    /// I/O failures.
    Io(String),
}
```

`impl Display` + `impl std::error::Error` — 对齐 `DxfReadError`。

### 5.2 兼容性策略

公有 API 改变 `Result<String, String>` → `Result<String, DxfWriteError>`
是 **breaking change**。对冲方式：

- `impl From<DxfWriteError> for String`（通过 `to_string()`），让
  `.map_err(|e| e.to_string())` 的下游代码零改动
- `impl From<String> for DxfWriteError`（wrap 成 `InvalidDocument`），
  让 `Err("...".into())` 之类老代码仍能编译
- `lib.rs` 里 `pub fn write_dxf(doc) -> Result<String, String>` **保留
  wrapper** 返回 `.map_err(|e| e.to_string())`，新 API 用新名
  `write_dxf_strict` — 下游按需迁移

选后者（**保留旧 API + 新增 strict 版本**），最小破坏。

### 5.3 `writer.rs` 内部修改

- 所有 `Result<String, String>` → `Result<String, DxfWriteError>`
  的函数签名改动
- 现有三个 `Err("...".into())` 调用点改成具体 enum 变体
- `write_dxf_string_impl` 签名 + 内部 `?` 传播

实际上当前 writer 里**只有** `write_dxf_string_impl` 的返回是 `Result`；
HEADER / TABLES / BLOCKS / ENTITIES 的辅助函数都是 `fn(&mut W, &doc)
-> ()`（infallible）。所以改动面很小。

### 5.4 测试

新测试 `tests/writer_error_types.rs`：

- `writer_string_error_surface_remains_unchanged`：`write_dxf` 返回值
  仍能 `.map_err(|e| e.to_string())` ——下游不破
- `writer_strict_returns_dxf_write_error_enum`：`write_dxf_strict`
  返回 `DxfWriteError` 变体可 pattern match
- `dxf_write_error_from_string_round_trips`：`DxfWriteError::from("...")`
  + `.to_string()` 一致

### 5.5 验收

- `cargo test -p h7cad-native-dxf` 169 → 172 全绿
- `cargo test --bin H7CAD io::native_bridge` 25 / 25 恒定
- `cargo check -p H7CAD` 零新 warning
- `cargo check -p h7cad-native-facade` 零新 warning
- `ReadLints` 改动的 2 个文件零 lint
- 公有 API 增加 `DxfWriteError` enum + `write_dxf_strict` 函数；
  既有 `write_dxf(doc) -> Result<String, String>` **签名不变**

## 6. 状态

- [x] 战略 roadmap 定稿
- [x] Phase 1 落地（write_dxf 错误类型升级）— 2026-04-24 完成
- [ ] Phase 2 / 3 / 4 由 owner 选择后再展开

## 7. 未来路线图快照

```
R30 (本轮)     ── Phase 1: DxfWriteError enum + write_dxf_strict
R31            ── Phase 2 实体扩展: OLE2FRAME
R32-R35        ── Phase 2 实体扩展: GEOPOSITIONMARKER / MATERIAL / MPOLYGON / FIELD
                 （4 轮，每轮 1 entity）
R36-R40        ── Phase 2 实体扩展: Surface family 5 个
                 （RevolvedSurface / SweptSurface 等）
R41+           ── Phase 3 DWG 红灯：LINE body_decode 修复
                 （先确立 AC1015 sample_01 regression fixture）
R50+           ── Phase 4 长尾 + fuzz 并行
```

此快照非硬承诺，owner 可随时重排。
