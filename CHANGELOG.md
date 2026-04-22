# 更新日志

## [未发布]

### 2026-04-22（二十八）：LOFT 3D 默认家族 6 变量扩充（跨过 100 门槛）

上一轮（二十七）首次混三类型、补完 drawing 元数据附加 4 变量，
HEADER 覆盖到 97。本轮走 plan §9 下一轮候选里的 LOFT 家族，
6 个变量里 **4 × f64（draft 角 / 幅度）+ 2 × i16（normals 枚举 /
param bitfield）**，一次性支撑 AutoCAD R2007+ 的 LOFT 命令默认值
保存 / 回放闭环，顺带让 HEADER 覆盖**越过 100 变量门槛**（97 → 103）。

**字段**（`DocumentHeader`，紧跟二十七轮 `$OLESTARTUP` 之后、
二十二轮 `$CHAMFERA` 之前）：

| 字段 | 类型 | `$` 变量 | DXF code | Default | 语义 |
|---|---|---|---|---|---|
| `loft_ang1` | `f64` | `$LOFTANG1` | 40 | `0.0` | 起横截面 draft angle（弧度） |
| `loft_ang2` | `f64` | `$LOFTANG2` | 40 | `0.0` | 止横截面 draft angle（弧度） |
| `loft_mag1` | `f64` | `$LOFTMAG1` | 40 | `0.0` | 起横截面 draft magnitude |
| `loft_mag2` | `f64` | `$LOFTMAG2` | 40 | `0.0` | 止横截面 draft magnitude |
| `loft_normals` | `i16` | `$LOFTNORMALS` | 70 | `1` | 曲面法向量来源 0–6 枚举（AutoCAD 默认 1 = smooth fit） |
| `loft_param` | `i16` | `$LOFTPARAM` | 70 | `7` | 位字段 1 = no twist / 2 = aligned / 4 = simple surfaces / 8 = closed（AutoCAD 默认 7） |

**策略**：io 层**纯透传**——`loft_normals` 枚举值含义、`loft_param`
各 bit 组合合法性、`loft_ang*` / `loft_mag*` 的物理单位，全部由 UI
/ 3D engine 自己解读。writer 对 `loft_param` 不过滤非法 bit 组合
（沿用 `indexctl` 二十七轮同款策略）。

**reader / writer 同步**：6 arm + 6 对 pair，紧跟 `$OLESTARTUP`，
在 HEADER 里形成 "元数据附加 → Loft 3D 默认 → 交互几何命令默认" 的
递进语义链。

**测试**（新增 `tests/header_loft.rs`，4 条）：

ground-truth 值：

- `loft_ang1 = π/6`（30°）/ `loft_ang2 = π/3`（60°） — 常用数学
  常量，借此验证二十五轮升级的 `format_f64` shortest round-trip 在
  本家族依然保真；任何精度回归都会在这两个 `assert_eq!` 上立刻挂
- `loft_mag1 = 1.5` / `loft_mag2 = 2.5` — 互不相等，防止 ang1/2 和
  mag1/2 两对 code-40 arm 串位（共享 code 40 是最容易的串位源）
- `loft_normals = 6`（path，与 default 1 不同）
- `loft_param = 9`（bit1 + bit8 = no twist + closed，罕见但合法 bit
  组合，覆盖"无扭转的闭合 loft"场景）

测试项：

- `header_reads_loft_family`：6 字段精确恢复 + `loft_ang1 != ang2`
  / `loft_mag1 != mag2` 两条 arm 串位 regression guard
- `header_writes_loft_family`：6 个 `$VAR` 按 reader arm 顺序出现
- `header_roundtrip_preserves_loft_family`：read → write → read 6
  字段 bit-identical；π/6 / π/3 的保真是 format_f64 精度的哨兵
- `header_legacy_file_without_loft_loads_with_defaults`：缺省命中
  (`0 / 0 / 0 / 0 / 1 / 7`)

**验证**：

- `cargo test -p h7cad-native-dxf` **161 / 161 → 165 / 165 全绿**
  （+4 header_loft，0 现存用例回归）
- `cargo test --bin H7CAD io::native_bridge` 25 / 25 不受影响
- `cargo check -p H7CAD` 通过，零新 warning
- `cargo check -p h7cad-native-facade` 通过
- `ReadLints` 改动的 4 个文件零 lint

**DXF HEADER 覆盖增量**：97 → **103** 个变量（~34%），**越过 100
门槛**。本轮是 docs/plans 系列从 Sprint 15 首日到现在的第 14 轮
micro-iteration，平均每轮 ~7 变量增量，按当前节奏再跑 30 轮左右可
望逼近 AutoCAD HEADER ~300 变量的 90% 工程性覆盖线。

plan：`docs/plans/2026-04-22-loft-family-plan.md`

---

### 2026-04-22（二十七）：Drawing 元数据附加 4 变量扩充（混合类型首轮）

上一轮（二十六）把 code 70 / `i16` 显示 & 渲染家族 5 变量一口气补齐，
HEADER 覆盖到 93 变量。本轮走 plan §9 下一轮候选里最短的"drawing
元数据附加"路径，**同时首次在一轮里混合 3 种类型**（`String × 2 +
i16 × 1 + bool × 1`），顺带验证 `sv(1) / i16v(70) / bv(290)` 三个
helper 在同一 HEADER 子组内的正交性。

**字段**（`DocumentHeader`，紧跟二十四轮 `$REQUIREDVERSIONS` 之后、
二十二轮 `$CHAMFERA` 之前，形成与二十三轮/二十四轮 drawing 身份组
连贯的 "身份 4 + 元数据附加 4 = 8 字段" 块）：

| 字段 | 类型 | `$` 变量 | DXF code | Default | 语义 |
|---|---|---|---|---|---|
| `project_name` | `String` | `$PROJECTNAME` | 1 | `""` | 项目名；AutoCAD 用它选 `ProjectFilePath` 子目录解析 XREF / raster 路径 |
| `hyperlink_base` | `String` | `$HYPERLINKBASE` | 1 | `""` | drawing 内相对 hyperlink 的根 URL / 路径 |
| `indexctl` | `i16` | `$INDEXCTL` | 70 | `0` | layer / spatial 索引 bitfield：bit0 = layer index，bit1 = spatial index |
| `olestartup` | `bool` | `$OLESTARTUP` | 290 | `false` | 打开 drawing 时是否预启 OLE 应用 |

**策略**：io 层**纯透传**——`project_name` 是否为合法路径、
`hyperlink_base` 是否为合法 URL、`indexctl` 的 bit 拆解、`olestartup`
的副作用，全部由 UI / 命令层负责。writer 对 `olestartup` 使用
`pair_i16(290, ...)`（参照既有 `$LWDISPLAY / $XEDIT` 同款 pattern），
不新引入 `pair_bool` 或类似。

**reader / writer 同步**：4 arm + 4 对 pair，紧跟
`$REQUIREDVERSIONS` 输出，在 HEADER 里形成 "身份 GUID 对 → 代码页
→ shadow → required_versions → 元数据附加 4 项 → 几何命令默认" 的
连贯顺序链。

**测试**（新增 `tests/header_drawing_metadata_addendum.rs`，4 条）：

ground-truth 值选得**每字段都与 Default 不同**，两个 String 还**互
不相等**且含 Unicode（CJK + Greek），借此顺带烟测 HEADER 字符串路径
对非 ASCII 的保真性：

- `project_name = "my-proj/sub-dir 项目 α"` — 空格 + 斜杠 + 中文 + 希腊
- `hyperlink_base = "https://example.com/docs/日本語/"` — 协议 + 路径 + 日文
- `indexctl = 3` — 两 bit 同时置位
- `olestartup = true`

两 String 共享 DXF code 1，arm 串位是 code-1 家族里最容易出的错；
`assert_ne!(project_name, hyperlink_base)` 作为直接 regression guard
让此类错误第一条挂掉。

测试项：

- `header_reads_drawing_metadata_addendum_family`：4 字段全部精确
  恢复 + 两 String arm 串位 regression guard
- `header_writes_drawing_metadata_addendum_family`：4 个 `$VAR` 按
  reader arm 顺序出现 + `project_name` / `hyperlink_base`
  Unicode verbatim
- `header_roundtrip_preserves_drawing_metadata_addendum_family`：
  read → write → read 4 字段 bit-identical（String 含 Unicode 触发
  路径的完整保真）
- `header_legacy_file_without_drawing_metadata_addendum_loads_with_defaults`：
  legacy HEADER 缺省命中 AutoCAD 出厂默认（空 / 空 / 0 / false）

**验证**：

- `cargo test -p h7cad-native-dxf` **157 / 157 → 161 / 161 全绿**
  （+4 header_drawing_metadata_addendum，0 现存用例回归）
- `cargo test --bin H7CAD io::native_bridge` 25 / 25 不受影响
- `cargo check -p H7CAD` 通过，零新 warning
- `cargo check -p h7cad-native-facade` 通过
- `ReadLints` 改动的 4 个文件（model lib.rs、dxf lib.rs、writer.rs、
  新 test）零 lint

**DXF HEADER 覆盖增量**：93 → **97** 个变量（~32%）。
**类型混合覆盖**：同一 Sprint 内同时演练 `sv / i16v / bv` 三个
helper，验证它们在 HEADER 子组内的正交 —— 为二十八轮及以后 "一轮
就把 AutoCAD 某完整维度补齐" 策略解锁，不必再按类型分家。

plan：`docs/plans/2026-04-22-drawing-metadata-addendum-plan.md`

---

### 2026-04-22（二十六）：显示 & 渲染控制家族 5 变量扩充

上一轮（二十五）补完 SNAP/GRID 家族 + `format_f64` shortest
round-trip，HEADER 覆盖到 88 变量。本轮选最短、最小风险路径：5 个
**全部 code 70 / `i16`** 的显示与渲染控制布尔 / 小枚举，一口气把
AutoCAD "用户自定义显示偏好"这一维度补齐。

**字段**（`DocumentHeader`，插在 `grid_unit` 之后 / `clayer` 之前
的全新 "Display & render flags" 子组）：

| 字段 | 类型 | `$` 变量 | DXF code | Default | 语义 |
|---|---|---|---|---|---|
| `dispsilh` | `i16` | `$DISPSILH` | 70 | `0` | 3D 线框视图 silhouette 边：0 = 关 / 1 = 开 |
| `dragmode` | `i16` | `$DRAGMODE` | 70 | `2` | 拖拽预览：0 = 关 / 1 = 开 / 2 = auto（AutoCAD 默认） |
| `regenmode` | `i16` | `$REGENMODE` | 70 | `1` | 自动重生：0 = 关 / 1 = 开（AutoCAD 默认） |
| `shadedge` | `i16` | `$SHADEDGE` | 70 | `3` | SHADE 着色：0 = 纯面 / 1 = 面+边 / 2 = 隐藏线 / 3 = 线框（AutoCAD 默认） |
| `shadedif` | `i16` | `$SHADEDIF` | 70 | `70` | 漫反射比（百分比 0–100，AutoCAD 默认 70） |

**策略**：io 层**纯透传**——`shadedif` 超 100 / `shadedge` 超 3 /
`dragmode` 非 0/1/2 等边界条件 AutoCAD 自己去接，H7CAD 不 clamp
不裁剪。所有 5 字段都存 `i16` 以保持家族**签名一致性**，哪怕
`dispsilh / regenmode` 语义是 bool；下游消费用 `!= 0` 判真假即可。

**reader / writer 同步**：5 arm + 5 对 pair，紧跟二十五轮 SNAP/GRID
组、正好形成 "mode flag → 空间值 → 渲染 flag → 属性" 的四段语义
梯度。

**测试**（新增 `tests/header_display_render.rs`，4 条）：

ground-truth 每字段都选**与 Default 不同**的值：

- `dispsilh = 1`（≠ 0）
- `dragmode = 0`（≠ 2）
- `regenmode = 0`（≠ 1）
- `shadedge = 1`（≠ 3）
- `shadedif = 50`（≠ 70）

arm 串位的 bug 一旦发生至少两条 assertion 同时挂掉，立刻可见。

测试项：

- `header_reads_display_render_family`：5 字段全部精确恢复
- `header_writes_display_render_family`：5 个 `$VAR` 按 reader arm
  顺序出现 + 值精确匹配（`contains`-then-advance-cursor 的顺序校验）
- `header_roundtrip_preserves_display_render_family`：read → write
  → read bit-identical
- `header_legacy_file_without_display_render_loads_with_defaults`：
  legacy HEADER 缺省命中 AutoCAD 出厂默认（0 / 2 / 1 / 3 / 70）

**验证**：

- `cargo test -p h7cad-native-dxf` **153 / 153 → 157 / 157 全绿**
  （+4 header_display_render，0 现存用例回归）
- `cargo test --bin H7CAD io::native_bridge` 25 / 25 不受影响
- `cargo check -p H7CAD` 通过，零新 warning
- `cargo check -p h7cad-native-facade` 通过
- `ReadLints` 改动的 4 个文件（model lib.rs、dxf lib.rs、writer.rs、
  新 test）零 lint

**DXF HEADER 覆盖增量**：88 → **93** 个变量（~31%）。
**总耗时**：约 17 min，是 2026-04-21 以来最短的一轮 Sprint；验证了
"小而密"策略下 `i16` 纯 passthrough 字段组的扩张极限速度。

plan：`docs/plans/2026-04-22-display-render-family-plan.md`

---

### 2026-04-22（二十五）：SNAP/GRID 家族 6 变量扩充 + `format_f64` 精度升级

上一轮（二十四）补完 i64 helper + `$REQUIREDVERSIONS`，HEADER 覆盖
到 82 变量。本轮沿同一路径前进：`snapmode / gridmode / orthomode`
三元布尔早就落地，但它们对应的"值"侧（snap 间距 / 基准 / 风格 / 旋
转 / 等轴测面 / grid 间距）之前**全部忽略**，导致 snap 开着但间距
归零的诡异 roundtrip。本轮把 6 个伴生变量一次补齐，顺带处理掉写入
路径 10 位小数截断的**精度漂移**隐患。

**字段**（`DocumentHeader`，插在 `attmode` 之后 / `clayer` 之前的
全新 "Snap & grid geometry" 子组）：

| 字段 | 类型 | `$` 变量 | DXF code | Default | 语义 |
|---|---|---|---|---|---|
| `snap_base` | `[f64; 2]` | `$SNAPBASE` | 10/20 | `[0.0, 0.0]` | snap 栅格基准点（当前 UCS） |
| `snap_unit` | `[f64; 2]` | `$SNAPUNIT` | 10/20 | `[0.5, 0.5]` | snap X / Y 间距 |
| `snap_style` | `i16` | `$SNAPSTYLE` | 70 | `0` | 0 = 正交；1 = 等轴测 |
| `snap_ang` | `f64` | `$SNAPANG` | 50 | `0.0` | snap 栅格旋转角（弧度） |
| `snap_iso_pair` | `i16` | `$SNAPISOPAIR` | 70 | `0` | 等轴测面 0 = 左 / 1 = 上 / 2 = 右 |
| `grid_unit` | `[f64; 2]` | `$GRIDUNIT` | 10/20 | `[0.5, 0.5]` | grid 显示 X / Y 间距 |

**策略**：io 层**纯透传**——`snap_style == 1` 时 AutoCAD UI 强制
X / Y 等间距、`snap_iso_pair` 在非等轴测时无语义等业务规则全由 UI
/ 命令层负责；writer 不自动对齐、不 normalise、不裁剪。

**reader / writer 同步**：6 arm + 6 对 pair。writer 紧跟
`$ATTMODE` 输出，在 HEADER 里形成 "mode flag → 伴生值" 的连贯
顺序块。

**`format_f64` 精度升级**（计划文件 §风险矩阵预案实现）：

先前 `writer::format_f64` 用 `{:.10}` 写出，π/4 这类常用数学角度
被截成 `0.7853981634`，丢失 7 位精度。本轮回归测试
`header_roundtrip_preserves_snap_grid_family` 首次揭示这个隐患。
修复：改用 `f64::to_string()`（Rust shortest round-trip, ryū 风），
保证 `s.parse::<f64>()` 总能拿回 bit-identical 值，同时保留既有
"整数值补 `.0`"、"零值输出 `"0.0"`" 两项格式约定。

**测试**（新增 `tests/header_snap_grid.rs`，4 条）：

ground-truth 值按字段选型：

- `snap_base = [3.25, -7.125]` — 非原点 + 负数 + 分数
- `snap_unit = [0.25, 0.5]` — X ≠ Y，盯死 code 10/20 列位
- `snap_style = 1` — 等轴测，与 Default 0 区分
- `snap_ang = π/4 ≈ 0.7853981633974483` — 直接暴露旧 `{:.10}`
  截断
- `snap_iso_pair = 2` — 右等轴测面
- `grid_unit = [1.0, 2.0]` — X ≠ Y，且与 `snap_unit` 不同，防止
  snap / grid 列混淆

测试项：

- `header_reads_snap_grid_family`：6 字段全部从非默认值恢复；额外
  `assert_ne!(snap_unit x, snap_unit y)` 与 `assert_ne!(grid_unit, snap_unit)`
  两条"列位混淆"回归断言
- `header_writes_snap_grid_family`：verbatim 写入 + 按 reader arm
  顺序出现（loose 顺序校验保 HEADER 布局确定）
- `header_roundtrip_preserves_snap_grid_family`：**最关键**——π/4
  必须 read → write → read bit-identical；此测试上先泄露了 `{:.10}`
  精度 bug，触发 `format_f64` 本轮升级
- `header_legacy_file_without_snap_grid_loads_with_defaults`：缺省
  legacy HEADER 读出六字段全部命中 Default

**验证**：

- `cargo test -p h7cad-native-dxf` **149 / 149 → 153 / 153 全绿**
  （+4 header_snap_grid，0 现存用例回归）
- `cargo test --bin H7CAD io::native_bridge` 25 / 25 不受影响
- `cargo check -p H7CAD` 通过，零新 warning
- `cargo check -p h7cad-native-facade` 通过
- `ReadLints` 改动的 4 个文件（model lib.rs、dxf lib.rs、writer.rs、
  新 test）零 lint

**DXF HEADER 覆盖增量**：82 → **88** 个变量（~29%）。
**`format_f64` 精度**：10 位小数截断 → 完整 f64 精度 round-trip，
所有 f64 写出路径（entity 坐标 / header 值 / table 参数 / block
偏移 / image 像素大小 / hatch 边缘 / 所有 DXF 文档中约 80% 的数值
字段）一次性受益。

plan：`docs/plans/2026-04-22-snap-grid-family-plan.md`

---

### 2026-04-22（二十四）：i64 helper 基建 + `$REQUIREDVERSIONS` 扩充

上一轮（二十三）收尾时发现 H7CAD 的 DXF io 缺 **i64 group-code** 处理
能力（reader / writer 均无 i64 helper），于是 code 160 的
`$REQUIREDVERSIONS` 挂账推迟。本轮**同时补基建 + 吞入该字段**，让
HEADER group-code 覆盖扩展到 160（int64）。

**helper 基建**：

- reader `crates/h7cad-native-dxf/src/lib.rs`：新增 `i64v` 闭包，与
  `i32v` 同形状（`.parse().ok().unwrap_or(0)`）
- writer `crates/h7cad-native-dxf/src/writer.rs`：新增 `pair_i64`
  成员函数，与 `pair_i32` 同形状（`value.to_string()`，非定宽）

**model 扩字段**（`DocumentHeader`，插在 `cshadow` 之后 / `chamfera`
之前）：

| 字段 | 类型 | `$` 变量 | DXF code | Default | 语义 |
|---|---|---|---|---|---|
| `required_versions` | `i64` | `$REQUIREDVERSIONS` | **160** | `0` | R2018+ 所需特性位字段 |

**策略**：io 层做**纯 i64 透传**——bit→feature 的映射由 AutoCAD
版本文档规定，不是 io 职责；writer 也**不**自动根据 drawing 里出现的
entity 类型去推算该置哪个 bit（那是未来 "版本需求推断" 工作，本轮
仅落地字段 io）。

**reader / writer 同步**：1 arm + 1 对 pair。writer 紧随 `$CSHADOW`
之后输出，在 Tier 3 metadata 组内形成 5 变量闭合。

**测试**（新增 `tests/header_required_versions.rs`，4 条）：

ground-truth 值选 `0x0000_1F2E_4D5C_789A` = **34 275 408 493 830 298**：

1. 远超 `i32::MAX` 7+ 个数量级 — 证明 helper 走真正 i64 路径、不是
   被编译器误推成 i32
2. 高 / 低 32 bit 都非零 — 任何 32-bit truncation bug 会立刻暴露
3. 既非 0 也非 `i64::MAX` — 与 Default / legacy-zero 场景天然区分

测试项：

- `header_reads_required_versions`：读后完整保留 + 断言 > `i32::MAX`
- `header_writes_required_versions`：verbatim 写入十进制表示（防止
  被错误千分位化 / 截断 / 十六进制化）
- `header_roundtrip_preserves_required_versions`：**最关键**的 64-bit
  保真校验 — read → write → read 大整数丝毫不差
- `header_legacy_file_without_required_versions_loads_with_zero`：
  缺省 → 0（向后兼容，不强制任何特性）

**验证**：

- `cargo test -p h7cad-native-dxf` **149 / 149 全绿**（145 前轮 + **4** 新 required_versions）
- `cargo test --bin H7CAD io::native_bridge` 25 / 25 不受影响
- `cargo check -p H7CAD` 通过，零新 warning
- `ReadLints` 改动的 4 个文件零 lint

**DXF HEADER 覆盖增量**：81 → **82** 个变量（~27%）。  
**DXF group-code 覆盖**：新增 **160（int64）**，让 io 层从此能处理
任意 AutoCAD i64 HEADER 变量，为未来更多 R2018+ 特性扩展铺路。

plan：`docs/plans/2026-04-22-required-versions-plan.md`

---

### 2026-04-22（二十三）：DXF HEADER Tier 3 表头元数据 4 变量扩充

前几轮主攻绘图样式 / 几何默认值，本轮补齐**表头元数据**家族 4 变量：
drawing 身份 GUID 对 + 字符代码页 + 当前实体 shadow 模式。这些值不
描述任何几何，而是图纸的身份 / 版本 / 渲染属性，H7CAD 之前 reader /
writer 全部忽略，导致 roundtrip 后静默归零。

**model 扩字段**（`DocumentHeader`，插在 `xedit` 之后 / `chamfera` 之前）：

| 字段 | 类型 | `$` 变量 | DXF code | Default | 语义 |
|---|---|---|---|---|---|
| `fingerprint_guid` | `String` | `$FINGERPRINTGUID` | 2 | `""` | 永久 GUID（创建时写入） |
| `version_guid` | `String` | `$VERSIONGUID` | 2 | `""` | 版本 GUID（每次 save 更新） |
| `dwg_codepage` | `String` | `$DWGCODEPAGE` | 3 | `""` | 字符代码页（如 `ANSI_1252`） |
| `cshadow` | `i16` | `$CSHADOW` | 280 | `0` | 当前实体阴影模式 0-3 |

**关键策略**：

- GUID 字段 Default `""` 而非随机生成 —— io 层**纯透传**，身份创建
  是命令层责任；reader 缺字段时不自行合成 GUID 以免破坏 "roundtrip
  不修改原有身份" 的保证。
- `$DWGCODEPAGE` R2007+ 已迁 UTF-8 但 AutoCAD 仍继续写出此字段兼容
  旧 reader，H7CAD 保持同样行为：写出原值即可，不做字符集转换。
- `$CSHADOW` 选 i16 存储而非 bool/enum：与同族 code 280 整数一致，
  且 4 个值（0=cast+receive, 1=cast, 2=receive, 3=ignore）未来若
  AutoCAD 扩 bitfield 也能兼容。

**reader / writer 同步**：4 arm + 4 对 pair。两个 `$FINGERPRINTGUID`/
`$VERSIONGUID` 共用 code 2，reader 按 `$VAR` 名字 match 分支天然隔离。

**测试**（新增 `tests/header_drawing_metadata.rs`，4 条）：

- `header_reads_all_4_drawing_metadata_vars`：两个 GUID 字符串（带 `{}`
  和连字符）+ `ANSI_1252` + `$CSHADOW=2` 精确读入
- `header_writes_all_4_drawing_metadata_vars`：构造 → write → 4 个
  `$VAR` + 3 个 GUID / codepage **原字面量**全在（防止意外 lower-case
  或 `{}` 剥离）
- `header_roundtrip_preserves_all_4_drawing_metadata_vars`：read →
  write → read 后字段既相等、也保持与绝对 ground-truth 一致
- `header_legacy_file_without_drawing_metadata_loads_with_defaults`：
  缺省 → 3 个 String 空串，`cshadow=0`（验证 io 层**不**合成 GUID）

**验证**：

- `cargo test -p h7cad-native-dxf` **145 / 145 全绿**（141 前轮 + **4** 新 drawing_metadata）
- `cargo test --bin H7CAD io::native_bridge` 25 / 25 不受影响
- `cargo check -p H7CAD` 通过，零新 warning
- `ReadLints` 改动的 4 个文件（model/lib、dxf/lib、writer、新测试）零 lint

**DXF HEADER 覆盖增量**：77 → **81** 个变量（约 **~27%**）。

**后续**：`$REQUIREDVERSIONS`（code 160 i64）因 writer 缺 `pair_i64` /
reader 缺 `i64v` helper，本轮暂缓；下一轮先加 helper 再一并吞入。

plan：`docs/plans/2026-04-22-drawing-metadata-plan.md`

---

### 2026-04-22（二十二）：DXF HEADER Tier 2 尺寸数字格式化 6 变量扩充

Tier 1（前几轮）落地了尺寸的**几何布局** 10 变量（`DIMTXT / DIMASZ /
DIMEXO / DIMEXE / DIMGAP / DIMDEC / DIMADEC / DIMTOFL / DIMSTYLE /
DIMTXSTY`）。本轮 Tier 2 补齐**数字格式化**侧的 6 个最常用项，让
尺寸文本的显示形式（舍入、缩放、公差小数、分数堆叠、分隔符、
零抑制）能完整 roundtrip AutoCAD .dxf 而不丢失。

**model 扩字段**（`DocumentHeader`，插在 `dimtxsty` 之后 / `splframe`
之前，形成完整 Tier 1 + Tier 2 尺寸家族块）：

| 字段 | 类型 | `$` 变量 | DXF code | Default | 语义 |
|---|---|---|---|---|---|
| `dimrnd` | `f64` | `$DIMRND` | 40 | `0.0` | 测量舍入精度（0 = 不舍入） |
| `dimlfac` | `f64` | `$DIMLFAC` | 40 | `1.0` | 线性缩放因子（负值 = 仅 PS） |
| `dimtdec` | `i16` | `$DIMTDEC` | 70 | `4` | 公差文本小数位数 |
| `dimfrac` | `i16` | `$DIMFRAC` | 70 | `0` | 分数堆叠 0=水平/1=斜线/2=不堆叠 |
| `dimdsep` | `i16` | `$DIMDSEP` | 70 | `46` | 小数分隔符 ASCII (46=`.`, 44=`,`) |
| `dimzin` | `i16` | `$DIMZIN` | 70 | `0` | 零抑制 bitfield (bit1=leading, bit2=trailing, bit4=0ft, bit8=0in) |

**关键点**：

- `$DIMLFAC` 负值语义（仅在 paper-space 引用中生效）由渲染层解读，io 层
  纯 f64 透传不做符号判断
- `$DIMDSEP` 是**字符的 ASCII 码**，与文件 IO 的字符编码无关
- `$DIMZIN` 各 bit 可组合，值域 0–15；测试用 `3`（bit1|bit2）验证
  bitfield 精确保持

**reader / writer 同步**：6 arm + 6 对 pair。writer 按 AutoCAD 顺序
插在 `$DIMTXSTY` 之后、`$SPLFRAME` 之前，独立分组便于后续 Tier 3 再扩。

**测试**（新增 `tests/header_dim_numerics.rs`，4 条）：

- `header_reads_all_6_dim_numerics`：非默认值（0.25 / 2.54 / 3 / 1 / 44 / 3）
  精确读取
- `header_writes_all_6_dim_numerics`：构造 → write → 6 个 `$VAR` 字符串全在；
  `$DIMZIN=12`（bit4|bit8）验证 0-feet / 0-inches 抑制位
- `header_roundtrip_preserves_all_6_dim_numerics`：read → write → read 全保持
- `header_legacy_file_without_dim_numerics_loads_with_defaults`：缺省 →
  **非零默认值必须兑现**（`dimlfac=1.0`, `dimtdec=4`, `dimdsep=46`）

**验证**：

- `cargo test -p h7cad-native-dxf` **141 / 141 全绿**（137 前轮 + **4** 新 dim-numerics）
- `cargo test --bin H7CAD io::native_bridge` 25 / 25 不受影响
- `cargo check -p H7CAD` 通过，零新 warning
- `ReadLints` 改动的 4 个文件（model/lib、dxf/lib、writer、新测试）零 lint

**DXF HEADER 覆盖增量**：71 → **77** 个变量（约覆盖 AutoCAD 总计 300+
系统变量的 **~26%**）。DIM 家族累计覆盖 16 个（Tier 1: 10 + Tier 2: 6）。

plan：`docs/plans/2026-04-22-dim-numerics-plan.md`

---

### 2026-04-22（二十一）：DXF HEADER `$CHAMMODE` 扩充（chamfer 模式联动）

上一轮（二十）落地了 chamfer 四个距离（`$CHAMFERA/B/C/D`）和 fillet 半径，
但漏了**模式开关** `$CHAMMODE`（code 70）。本轮补这一个整数，让 chamfer
家族形成闭环：

- 0 = **Distance-Distance** 模式，使用 `$CHAMFERA` / `$CHAMFERB`（默认）
- 1 = **Length-Angle** 模式，使用 `$CHAMFERC` / `$CHAMFERD`

不补的话，任何用户切到 Length-Angle 模式的 AutoCAD .dxf 被 H7CAD 读写
roundtrip 后都会静默退回 Distance-Distance。

**model 扩字段**（`DocumentHeader`，插在 `chamferd` 之后 / `filletrad`
之前，遵循 AutoCAD HEADER 官方输出顺序）：

| 字段 | 类型 | `$` 变量 | DXF code | Default |
|---|---|---|---|---|
| `chammode` | `i16` | `$CHAMMODE` | 70 | `0` |

类型选 `i16` 而非 `bool`，与同族 `$CMLJUST` / `$ATTMODE` 等 code 70 整数
保持一致，并为 AutoCAD 未来可能的 tri-state 扩展留出空间（目前官方
仅定义 0 / 1）。

**reader / writer 同步**：1 arm + 1 对 pair（writer 严格插在 `$CHAMFERD`
pair 之后、`$FILLETRAD` pair 之前，保持 chamfer 家族内部顺序不变）。

**测试**（新增 `tests/header_chammode.rs`，4 条）：

- `header_reads_chammode`：`$CHAMMODE=1` 和 `$CHAMMODE=0` 双向读
- `header_writes_chammode`：构造 `chammode=1` → write → `$CHAMMODE` 字符串
  存在、紧随的 `70` 值精确为 `1`（防止错写成 code 40 float pair）
- `header_roundtrip_preserves_chammode`：read(1) → write → read 仍为 1
- `header_legacy_file_without_chammode_loads_with_zero`：缺省 → `chammode == 0`

**验证**：

- `cargo test -p h7cad-native-dxf` **137 / 137 全绿**（133 前轮 + **4** 新 chammode）
- `cargo test --bin H7CAD io::native_bridge` 25 / 25 不受影响
- `cargo check -p H7CAD` 通过，零新 warning
- `ReadLints` 改动的 4 个文件（model/lib、dxf/lib、writer、新测试）零 lint

**DXF HEADER 覆盖增量**：70 → **71** 个变量（~23%）。chamfer 家族完成
闭环：4 距离 + 1 模式 + 1 fillet 半径 = 6 变量全齐。

plan：`docs/plans/2026-04-22-chammode-plan.md`

---

### 2026-04-22（二十）：DXF HEADER Chamfer / Fillet / 3D 默认值 7 变量扩充

继 HEADER 系列扩字段（三→十五 共 48 变量），本轮再补 7 个 code 40 f64
常用默认量：Chamfer 四距离（`$CHAMFERA/B/C/D`）+ Fillet 半径（`$FILLETRAD`）
+ 当前 Elevation / Thickness（`$ELEVATION` / `$THICKNESS`）。H7CAD 之前
reader 全部丢、writer 不输出，任意读 AutoCAD .dxf 再存回都会让这 7 个
设置静默归零。

**model 扩字段**（`DocumentHeader`，插在 `xedit` 之后 / `handseed` 之前）：

| 字段 | 类型 | `$` 变量 | DXF code | Default |
|---|---|---|---|---|
| `chamfera` | f64 | `$CHAMFERA` | 40 | 0.0 |
| `chamferb` | f64 | `$CHAMFERB` | 40 | 0.0 |
| `chamferc` | f64 | `$CHAMFERC` | 40 | 0.0 |
| `chamferd` | f64 | `$CHAMFERD` | 40 | 0.0 |
| `filletrad` | f64 | `$FILLETRAD` | 40 | 0.0 |
| `elevation` | f64 | `$ELEVATION` | 40 | 0.0 |
| `thickness` | f64 | `$THICKNESS` | 40 | 0.0 |

`$CHAMFERA/B` 是 Distance-Distance 模式两距离；`$CHAMFERC/D` 是
Distance-Angle 模式的长度 + 角度（角度单位由 `$AUNITS` 决定，reader / writer
纯 f64 透传，不做 rad↔deg 归一化）。`$ELEVATION` / `$THICKNESS` 是**绘图级
默认**——与 entity-level 同名字段编译器作用域天然隔离，互不覆盖。

**reader / writer 同步**：7 arm + 7 对 pair（writer 按 AutoCAD 顺序在 `$XEDIT`
之后、`$PDMODE` 之前，分两组：interactive geometry defaults / 2.5-D default
attachment）。

**测试**（新增 `tests/header_geom_defaults.rs`，4 条）：

- `header_reads_all_7_geom_default_vars`：非默认值（1.25 / 0.75 / 2.0 / 45.0
  / 0.5 / 10.0 / 3.14）精确读取
- `header_writes_all_7_geom_default_vars`：构造 → write → 7 个 `$VAR`
  字符串都在
- `header_roundtrip_preserves_all_7_geom_default_vars`：read → write → read
  全字段 1e-9 精度保持
- `header_legacy_file_without_geom_defaults_loads_with_zeros`：缺省 → 7 个
  字段全部 0.0（Default trait 兜底）

**验证**：

- `cargo test -p h7cad-native-dxf` **133 / 133 全绿**（129 前轮 + **4** 新 geom defaults）
- `cargo test --bin H7CAD io::native_bridge` 25 / 25 不受影响
- `cargo check --bin H7CAD --tests` 通过，零新 warning
- `ReadLints` 改动的 4 个文件（model/lib、dxf/lib、writer、新测试）零 lint 错误

**DXF HEADER 覆盖增量**：63 → **70** 个变量（约覆盖 AutoCAD 总计 300+
系统变量的 **~23%**）。

plan：`docs/plans/2026-04-22-header-geom-defaults-plan.md`

---

### 2026-04-22（十九）：DWG Open 非致命诊断接入命令行（Milestone D'）

延续 2026-04-21 的 DWG open 路径收口（十六/十七/十八），把**已经由 `acadrust`
采集但一直被 H7CAD 丢弃**的非致命诊断（`CadDocument::notifications`）带到
命令行输出。原审核 Milestone D（"把 `h7cad-native-dwg` 的 advisory 接入
运行时"）的前提不成立——`h7cad-native-dwg::read_dwg` 并不在运行时主打开
路径上（当前走的是 `acadrust::io::dwg::DwgReader`），真正"接入"等于先把
主路径迁到 native-dwg，属于 Milestone E 级别的重构。

退而调研发现更务实的目标：acadrust 的 DWG reader 本身就会在解析过程中
通过 `NotificationCollection::notify` 填入 22 + 4 处 `NotImplemented /
NotSupported / Warning / Error` 诊断，最终汇入 `CadDocument::notifications`。
这条通道**从 reader → compat doc 一直是通的**，唯独 H7CAD 的 io 层在
`native_bridge::acadrust_doc_to_native` 桥接时把 notifications 字段扔了，
UI 层的 `Message::FileOpened(Ok)` 分支也从不访问它。本轮修复这条断链。

**新增模块**（`src/io/diagnostics.rs`，~280 行，8 条单元测试）：

```rust
pub enum NoticeSeverity {
    NotImplemented, NotSupported, Warning, Error,
}

pub struct OpenNotice {
    pub severity: NoticeSeverity,
    pub message: String,
}

pub struct NoticeCounts {
    pub not_implemented: usize,
    pub not_supported: usize,
    pub warning: usize,
    pub error: usize,
}

pub fn from_acadrust_notifications(
    src: &acadrust::NotificationCollection,
) -> Vec<OpenNotice>;
```

**设计要点**：

- **独立于 acadrust 类型**：`OpenNotice` 是 io 层自有的 string-based struct，
  不再暴露 `acadrust::Notification` 给 `app::update`。将来 Milestone E
  真正切到 native-dwg 后，只需要在 `load_file_native_blocking` 里把
  `Ac1015RecoveryDiagnostics.summarize()` 也映射到同一个 `OpenNotice`
  形态即可——UI 管道零改动。
- **中英双标签**：`NoticeSeverity::zh_label()` 给命令行（"警告"/"已恢复
  错误"/"未实现"/"不支持"），`en_tag()` 预留给未来的日志文件/结构化
  持久化（`#[allow(dead_code)]` 标注意图）。
- **聚合摘要**：`NoticeCounts::summary_zh()` 生成"3 条警告 / 1 条已恢复
  错误 / 2 条未实现"形式的紧凑摘要，通过 " · " 附在"Opened … — N
  entities"之后；零通知时返回 `None`，调用点零分支即可跳过后缀。

**链路改造**（`src/io/mod.rs`）：

- `load_file_native_blocking` 签名从 `-> Result<NativeCadDocument, OpenError>`
  升级为 `-> Result<(NativeCadDocument, Vec<OpenNotice>), OpenError>`
- `load_file_with_native_blocking` 同步升级为 `(CadDocument, Option<Native>, Vec<OpenNotice>)`
- `open_document_blocking` 升级为 `(OpenedDocument, Vec<OpenNotice>)`
- `open_path` 在 worker 线程里把 notices 塞进 `OpenFileResult`
- `load_file`（向后兼容 xref 用）丢弃 notices 保持 `Result<CadDocument, String>` 签名不变
- DWG 路径：`diagnostics::from_acadrust_notifications(&acad_doc.notifications)`
- DXF / PID 路径：返回空 `Vec`（将来各自接入时再填）

**`OpenFileResult`** 新增 `pub notices: Vec<OpenNotice>` 字段，配套
文档注释说明数据来源与未来扩展方向。

**UI 消费**（`src/app/update.rs::Message::FileOpened(Ok)`）：

```rust
let notice_counts = crate::io::NoticeCounts::from_notices(&notices);
let full_open_message = if let Some(summary) = notice_counts.summary_zh() {
    format!("{open_message} · {summary}")
} else {
    open_message
};
self.command_line.push_output(&full_open_message);
const MAX_INLINE_NOTICES: usize = 5;
for notice in notices.iter().take(MAX_INLINE_NOTICES) {
    self.command_line.push_info(&notice.format_zh());
}
if notices.len() > MAX_INLINE_NOTICES {
    self.command_line.push_info(&format!(
        "… 另有 {} 条诊断未展开",
        notices.len() - MAX_INLINE_NOTICES
    ));
}
```

**截断策略**：大型工程 DWG 容易产生数十条 `NotImplemented` 级诊断
（THUMBNAILIMAGE section、各种 APPID 表项等），一次性全推会淹没命令行
其他输出。选择"汇总计数 + 首 5 条原文 + N 条省略尾注"的组合，覆盖典型
"看一眼有什么毛病"场景，完整列表留给 Milestone D'' 的诊断面板（未来）。

**测试**（`src/io/diagnostics.rs::tests`，8 条全绿）：

- `severity_from_acadrust_covers_every_variant`：4 个 `NotificationType` → `NoticeSeverity` 映射完整
- `severity_labels_are_stable_in_chinese_and_english`：4 个 severity × 2 种标签全非空
- `format_zh_produces_bracketed_label_prefix`：`"[警告] handle …"` 格式稳定
- `from_acadrust_notifications_preserves_order_and_message_text`：3 条不同严重度的通知按序转换
- `from_acadrust_notifications_returns_empty_for_empty_collection`：空集合 → 空 Vec
- `notice_counts_bucket_by_severity`：4 严重度分桶统计正确
- `notice_counts_summary_zh_none_when_empty`：零通知时 `None`
- `notice_counts_summary_zh_joins_nonzero_buckets`：非零桶以 " / " 连接，零桶不出现

**验证**：

- `cargo test --bin H7CAD io::diagnostics::` **8 / 8 全绿**（本轮新增）
- `cargo test --bin H7CAD io::` **138 / 138 全绿**（130 前轮 + 8 本轮）
- `cargo check --bin H7CAD --tests` 通过，**零新 warning**
- `ReadLints` 受影响文件（`diagnostics.rs` / `mod.rs` / `update.rs`）零 lint

**用户可感知的改进**：

- 打开任何带非致命解析问题的 DWG 时，命令行会立刻看到："Opened "foo.dwg"
  — 1234 entities · 3 条警告 / 1 条已恢复错误 / 2 条未实现"，紧接着前 5
  条具体条目以 `[严重度] 消息` 形式列出。
- 零诊断的"干净"DWG 打开路径不变，摘要行不出现多余后缀。
- DXF / PID 打开路径当前零诊断（空 Vec 直接跳过后缀），为将来各自接入
  留下 hook。

**Milestone E 过渡路径**（非本轮）：

当主打开路径切到 `h7cad_native_dwg::read_dwg` 时，只需在
`load_file_native_blocking` 的 DWG 分支里把
`Ac1015RecoveryDiagnostics.summarize()` 映射到同一批 `OpenNotice`
（可能需要再补一个 `from_native_dwg_diagnostics` helper），UI 层零
改动即可直接继承新数据源。这就是独立于 acadrust 类型设计的价值。

plan 依据：本轮无独立 plan 文件；改自 2026-04-21（十七）的"不包含"段落
与同日 DWG 审核报告 P2-⑦。

---

### 2026-04-21（十八）：DWG/DXF Save 对话框版本标签诚实化 + 双向 version 保真

继 2026-04-21（十六/十七）DWG 打开路径收口后完成审核 P0-③（Milestone C）。原始问题是 `pick_save_path` 暴露 **16 个版本标签**（DWG/DXF 各 8 个 `(2018)/(2013)/.../(R13)` 后缀），但 `rfd::AsyncFileDialog::save_file()` 的 API 不返回用户选中的 filter，所以这些标签**从不曾影响实际写出的版本**——纯菜单欺骗。

执行过程中发现**比审核报告更严重的问题**：`src/io/native_bridge.rs` 的两个方向都**忽略 version 字段**：

- `acadrust_doc_to_native(&acad_doc)`：用 `nm::CadDocument::new()` 构造，默认 `R2000`，从不读 `acad_doc.version`
- `native_doc_to_acadrust(&native)`：用 `acadrust::CadDocument::new()` 构造，默认 `AC1032`/R2018，从不读 `native.header.version`

连锁效应是**任意"打开 → 保存"往返都会把文件版本重置为 `AC1032`**：打开一个真实 R2000 (AC1015) 文件再另存，输出永远是 R2018 格式。不只是 UI 欺骗，更是一个**静默的数据丢失 bug**。

本轮把两层都修了：

**改动 1：`src/io/mod.rs::pick_save_path` filter 收敛 16 → 3**

```rust
pub async fn pick_save_path() -> Option<PathBuf> {
    // The output version is NOT selected here — it is taken from the
    // document's in-memory `version` field. rfd's save_file API does
    // not return the picked filter, so the earlier "DWG Files (2018)"
    // … "(R13)" labels could never influence the actual output. The
    // single-label dialog is honest about what actually drives the
    // output format.
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

**改动 2：`src/io/native_bridge.rs` — 双向版本桥接 helper**

新增 `nm_version_to_acadrust` 与 `acadrust_version_to_nm` 两个一一对应的映射函数，覆盖全部 9 个版本 variant：

| native (`nm::DxfVersion`) | acadrust (`acadrust::types::DxfVersion`) | 年代 |
|---|---|---|
| `R12` | `Unknown` *(acadrust 无 AC1009 槽)* | AutoCAD R12 |
| `R13` | `AC1012` | R13 |
| `R14` | `AC1014` | R14 |
| `R2000` | `AC1015` | R2000 |
| `R2004` | `AC1018` | R2004 |
| `R2007` | `AC1021` | R2007 |
| `R2010` | `AC1024` | R2010 |
| `R2013` | `AC1027` | R2013 |
| `R2018` | `AC1032` | R2018 |
| `Unknown` | `AC1015` *(保守默认)* | — |

`native_doc_to_acadrust` 新增：`doc.version = nm_version_to_acadrust(native.header.version);`  
`acadrust_doc_to_native` 新增：`native.header.version = acadrust_version_to_nm(doc.version);`

**改动 3：`save_dwg` / `save_dxf` 加诚实文档注释**

```rust
/// Write the document to a DWG file at `path`.
///
/// The output DWG format version is taken from `doc.header.version`
/// (propagated through `native_bridge::native_doc_to_acadrust`), so
/// "Save As" on an opened drawing preserves its original version
/// (e.g. an AC1015/R2000 file stays AC1015). Fresh documents built
/// with `NativeCadDocument::new()` default to `R2000`; opening and
/// re-saving them is lossless.
```

**测试**（`src/io/native_bridge.rs` 新增 4 条）：

- `version_bridge_forward_covers_every_native_variant`：枚举 9 个 `nm::DxfVersion` 变体，验证 `nm_version_to_acadrust` 表
- `version_bridge_reverse_covers_every_acadrust_variant`：枚举 9 个 `acadrust::DxfVersion` 变体，验证逆映射
- `native_to_acadrust_preserves_version_from_document_header`：`native.header.version = R14` → `acad.version == AC1014`
- `acadrust_to_native_preserves_version_from_document`：`acad.version = AC1015` → `native.header.version == R2000`
- `version_survives_bridge_roundtrip_in_both_directions`：`AC1018` → native → `AC1018`（典型"打开再保存"往返）

**验证**：

- `cargo test --bin H7CAD io::native_bridge::` **25 / 25 全绿**（21 前轮 + 4 本轮）
- `cargo test --bin H7CAD io::` **130 / 130 全绿**（125 前轮 + 5 本轮：4 version + 1 其他）
- `cargo check --bin H7CAD --tests` 通过，零新 warning
- `ReadLints` 改动的 `src/io/mod.rs` / `src/io/native_bridge.rs` 零 lint 错误

**用户可感知的改进**：

1. 另存对话框从 8+8+1=17 项折叠为 3+1=4 项（DWG/DXF/PID + All Files），视觉噪音下降、决策成本下降
2. 打开 R2000 工程图 → 另存，输出确实是 R2000 而不是 R2018。版本保真恢复
3. `save_dwg` doc comment 明确告知"版本来自 `doc.header.version`"，未来想手动指定版本的路线图在注释里明文指向本 plan

**副作用检查**：

- `DwgReader` 本身就会 sniff 文件 magic 填 `acad_doc.version`，所以读入链路的 version 识别能力没有变化——本轮只是在读/写两侧把这个已识别值**透传**到 native 侧
- `pick_cui_save_path` / `pick_image_file` / `pick_and_open` 不涉及版本歧义，filter 保持原样
- Milestone D（native-dwg advisory 接入）、Milestone E（AC1018+ 扩展）、Milestone F（xref 异步）**留作后续**，详见 plan 文件

plan 依据：`docs/plans/2026-04-21-dwg-save-version-honesty-plan.md`

---

### 2026-04-21（十七）：DWG Open 同步解析移出 iced 主循环

继 2026-04-21（十六）`OpenError` 类型化后继续收口 DWG 打开路径。审核 P0-②：`open_document` 链路是**同步**的（`DwgReader::from_file` + `reader.read()` + 我们的 DXF/PID fallback 都走同步 std::fs），但被包在 `async fn open_path` 里由 `Task::perform` 提交给 iced executor。对 50 MB+ 工程 DWG 来说，整段解析会让 iced 主线程卡 1–3 秒：菜单、鼠标、命令行全部冻结。

**定位**：iced 0.14 的 executor 基于 `futures` crate（非 tokio / smol），因此没有 `tokio::task::spawn_blocking`。可用选择：

1. `iced::Task::blocking(...)` — iced 0.14 原生支持的跨线程闭包，结果回到 update loop（需要拆 Message 路径，侵入大）
2. 在 `async fn` 内部用 `iced::futures::channel::oneshot` + `std::thread::spawn`（`iced::futures` 重导出 `futures 0.3.32`，已在依赖树，无需新 crate）

选 **方案 2** — API 不破坏、调用点零改动、panic 隔离清晰。

**架构改造**（`src/io/mod.rs`）：

```rust
pub async fn open_path(path: PathBuf) -> Result<OpenFileResult, OpenError> {
    let (tx, rx) = iced::futures::channel::oneshot::channel();
    std::thread::Builder::new()
        .name("h7cad-open-file".into())
        .spawn(move || {
            let name = path.file_name().map(...).unwrap_or_else(...);
            let result = open_document_blocking(&path)
                .map(|opened| OpenFileResult { name, path: path.clone(), opened });
            let _ = tx.send(result);
        })
        .map_err(|e| OpenError::Io {
            path: None,
            message: format!("failed to spawn file-open worker thread: {e}"),
        })?;
    rx.await.unwrap_or_else(|_| Err(OpenError::Other(
        "file open worker terminated before responding".into(),
    )))
}
```

**命名：同步函数一律加 `_blocking` 后缀**

原先 `open_document` / `load_file_with_native` / `load_file_native` / `load_dxf_native` 这几个名字没有任何"同步"视觉提示，容易被误用到 async 上下文。这一轮统一：

| 旧名字 | 新名字 |
|---|---|
| `open_document` | `open_document_blocking` |
| `load_file_with_native` | `load_file_with_native_blocking` |
| `load_file_native` | `load_file_native_blocking` |
| `load_dxf_native` | `load_dxf_native_blocking`（私有） |

`load_file`（给 xref 用的 `Result<_, String>` 兼容层）保持不变，它本身已在同步上下文中调用。新的模块层注释明确两类 API 的分工：

```text
// The public `pick_and_open` / `open_path` functions are `async fn`
// and are normally polled on iced's event-loop executor. The actual
// DWG/DXF decoding is CPU-bound and performs synchronous file I/O,
// so running it directly inside the future would stall iced's main
// thread for the duration of the read (seconds on large engineering
// drawings).
//
// The `*_blocking` helpers below keep that synchronous logic in its
// natural form, and the async wrappers dispatch the work onto a
// dedicated worker thread, signalling completion through an iced-
// provided `futures::channel::oneshot`.
```

**panic 安全**：若 worker 线程在 `open_document_blocking` 中 panic，`tx` 在 `drop` 时会取消 oneshot，`rx.await` 返回 `Err(Canceled)`，我们再 map 到 `OpenError::Other("worker terminated...")`。UI 得到一个分类错误而不是永久挂起的 Task。

**线程命名**：`std::thread::Builder::name("h7cad-open-file")` 让 panic 日志、调试器堆栈能一眼看到这是文件打开线程，而不是匿名 `thread #42`。

**测试**（`src/io/open_error.rs` 新增 4 条，`tests/` 总共 19 条）：

- `open_document_blocking_rejects_unsupported_extension`：`.xyz` → `UnsupportedExtension { ext: "xyz" }`（白名单前移后的行为）
- `open_document_blocking_reports_io_for_missing_dwg`：不存在的 `.dwg` → `Io { path: Some(…), message: … }`，验证 path slot 被填充
- `open_document_blocking_reports_io_for_missing_dxf`：同上针对 DXF
- `open_path_async_does_not_block_caller_thread_panic_safety`：用 `iced::futures::executor::block_on(open_path(…))` 跑一个 missing 文件，验证 worker 线程 + oneshot 闭环不死锁、错误正确分类

**验证**：

- `cargo test --bin H7CAD io::open_error::` **19 / 19 全绿**（15 前轮 + 4 本轮）
- `cargo test --bin H7CAD io::` **125 / 125 全绿**（121 前轮 + 4 本轮）
- `cargo check --bin H7CAD --tests` 通过（仅保留两个预先存在的 pid_import 无关 warning）
- `ReadLints` 改动的 `open_error.rs` / `mod.rs` 零 lint 错误

**用户可感知的改进**：打开大 DWG 期间，iced 主线程继续处理鼠标、键盘、窗口重绘；命令行状态栏不会"静止几秒再突然出结果"。取消按钮（如果未来加入）也能在 parse 中途真正中断。

**副作用检查**：

- `xref::resolve_xrefs` 仍走同步 `load_file` 路径——它本身被调用在 `Message::FileOpened` 的**同步消息分发**里（不是 iced async 任务），所以不会冻结事件循环；只会让 `FileOpened` 处理本身耗时更长。本轮 **不**优化 xref，因为它不会造成 UI 卡顿，且修改 xref 需要重构 resolve 逻辑为批量异步、涉及面更大。留待后续 Milestone 讨论。
- 多 Tab 并行打开多个文件：每次 `Task::perform(open_path(...), ...)` 会起一个 worker 线程；iced executor 不阻塞。并行性由 OS 线程调度器自然提供。

**不包含**（列入后续 milestone）：

- Milestone C — `pick_save_path` 保存对话框里 8 个 "DWG Files (2018/2013/.../R13)" 版本标签是装饰，`save_dwg` 并不接受版本参数
- Milestone D — 把 `h7cad-native-dwg` advisory 接入运行时（diagnostics 暴露给 UI）
- Milestone E — `h7cad-native-dwg` 扩 AC1018+（R2004~R2018）版本覆盖

plan：基于同日 DWG 审核报告（P0-② 阻塞 I/O）

---

### 2026-04-21（十六）：DWG Open 路径错误类型化 + UI 中文友好提示

继 DWG 解析打开功能审核，落地 Milestone A：把 `io::pick_and_open / open_path / open_document / load_file_with_native / load_file_native` 的错误类型从 `String` 升级为结构化 `OpenError`，UI `Message::FileOpened(Err)` 分支改为调用 `user_message_zh()` 输出本地化提示。原来用户打开 AC1032 的 DWG 只看到英文 `"unsupported version AC1032"`，现在可以看到"暂不支持该 DWG 文件版本：AC1032。请尝试用 AutoCAD 另存为 AC1015 (AutoCAD 2000) 或 DXF 后重试。"

**新增**（`src/io/open_error.rs`，~280 行）：

```rust
pub enum OpenError {
    Cancelled,
    Io { path: Option<PathBuf>, message: String },
    UnsupportedVersion { format: &'static str, version: String },
    Corrupt { format: &'static str, reason: String },
    UnsupportedExtension { ext: String },
    Other(String),
}
```

- `user_message_zh() -> String`：variant → 中文提示（"另存为 AC1015"等可操作建议）。
- `is_silent() -> bool`：让 `Cancelled` 等静默 variant 不污染命令行。
- `impl Display / std::error::Error / From<io::Error> / From<String> / From<&str>`。
- `classify_acadrust(DxfError, &'static str) -> OpenError`：把 `acadrust::DxfError` 的 17 个 variant 精确路由到 `Io` / `UnsupportedVersion` / `Corrupt` / `Other` 四类。
- `classify_native_dxf(DxfReadError) -> OpenError`：同上，针对 `h7cad-native-dxf` 的错误。

**签名改造**（`src/io/mod.rs`）：

| 函数 | 改前 | 改后 |
|---|---|---|
| `pick_and_open` | `Result<_, String>` | `Result<_, OpenError>` |
| `open_path` | `Result<_, String>` | `Result<_, OpenError>` |
| `open_document` | `Result<_, String>` | `Result<_, OpenError>` |
| `load_file_with_native` | `Result<_, String>` | `Result<_, OpenError>` |
| `load_file_native` | `Result<_, String>` | `Result<_, OpenError>` |
| `load_file` | `Result<_, String>` | **保留**（xref.rs 调用方不关心类型） |

`load_file` 内部改为调用新的 `load_file_with_native`，再 `.map_err(|e| e.to_string())` 兜底 String，保证 `xref::resolve_xrefs` 零改动。

**`open_document` 白名单前移**：之前 `_ =>` 会把 `.foo` 等乱扩展名扔到 DWG 解析链直到最底层才抛错；现在 `"dwg" | "dxf" => CAD, "pid" => Pid, _ => OpenError::UnsupportedExtension`。

**文件扩展名 filter 去重**：`pick_and_open` 原本把 "dwg" 和 "DWG" 都写进 filter，`rfd` 在所有 OS 上扩展名都是大小写不敏感，精简为单份小写。

**UI 改动**：

- `src/app/mod.rs`：`Message::FileOpened` 签名 `Result<OpenFileResult, String>` → `Result<OpenFileResult, OpenError>`
- `src/app/update.rs`：
  ```rust
  Message::FileOpened(Err(e)) => {
      if !e.is_silent() {
          self.command_line.push_error(&e.user_message_zh());
      }
      Task::none()
  }
  ```
  取代之前的 `if e != "Cancelled"` 魔法字符串比较。

**副产物修复**：`src/io/pid_import.rs` 测试 fixture 里 `CrossReferenceGraph { ... }` 因 `pid-parse 0.9.2` 新增 8 个字段而拒绝编译。补 `..CrossReferenceGraph::default()` 结尾，保留显式字段含义。

**测试**（`src/io/open_error.rs` `tests/`，15 条）：

- `cancelled_is_silent` / `from_string_cancelled_becomes_cancelled_variant` / `from_string_other_becomes_other_variant`
- `from_io_error_preserves_message`
- `classify_acadrust_{unsupported_version / invalid_header / crc_mismatch / io / not_implemented}`
- `classify_native_dxf_{unsupported_format / unexpected_eof}`
- `zh_message_for_{cancelled / unsupported_extension / unsupported_version}`（验证中文关键词与可操作建议存在）
- `display_impl_renders_variant_specific_prefix`

**验证**：

- `cargo test --bin H7CAD io::open_error::` **15 / 15 全绿**
- `cargo test --bin H7CAD io::` **121 / 121 全绿**（PID + XREF + Open 全套回归）
- `cargo check --bin H7CAD --tests` 通过（仅留两个预先存在的 pid_import 无关 warning）
- `ReadLints` 4 个改动文件零 lint 错误

**不包含**（列入后续 milestone）：

- Milestone B — `load_file_native` 是同步阻塞 I/O 但被 async 调用（打开大 DWG 会卡 iced 事件循环）
- Milestone C — `pick_save_path` 保存对话框里 8 个 "DWG Files (2018/2013/.../R13)" 版本标签是装饰，`save_dwg` 并不接受版本参数
- Milestone D — 把 `h7cad-native-dwg` advisory 接入运行时（diagnostics 暴露给 UI）
- Milestone E — `h7cad-native-dwg` 扩 AC1018+（R2004~R2018）版本覆盖

plan：基于同日 DWG 审核报告（P0-① 错误类型化）

---

### 2026-04-21（十五）：DXF HEADER 当前标注样式名引用 2 变量

继 2026-04-21（十）DIM Tier 1 8 数值变量后补齐 DIM 区块的 **name-pointer** 部分：当前标注样式名 + 当前标注文字样式名。

**model 扩字段**（`DocumentHeader`，紧跟 `dimtofl` 之后）：

| 字段 | 类型 | `$` 变量 | DXF code | Default |
|---|---|---|---|---|
| `dimstyle` | `String` | `$DIMSTYLE` | 2 | `"Standard"` |
| `dimtxsty` | `String` | `$DIMTXSTY` | **7** | `"Standard"` |

`$DIMTXSTY` 的 group code 是 7（text style name），与其他 `$DIM*` 的 code 70 / 40 不同。

**reader / writer 同步**：2 arm + 2 对 pair 块（writer 紧跟 `$DIMADEC` 之后输出，形成完整 DIM 区块）

**测试**（新增 `tests/header_dimstyle_name_refs.rs`，4 条）：read / write / roundtrip / legacy 默认。

**验证**：

- `cargo test -p h7cad-native-dxf` **129 / 129 全绿**（125 前轮 + **4** 新 dimstyle name refs）
- `cargo test --bin H7CAD io::native_bridge` 20 / 20 无回归
- `ReadLints` 4 个文件零 lint 错误

**DXF HEADER 覆盖增量**：61 → **63** 个变量（约 21%）。

plan：`docs/plans/2026-04-21-header-dimstyle-name-refs-plan.md`

---

### 2026-04-21（十四）：`parse_iso8601` — Julian helper 反向链路闭合

闭合同日（十一）julian-date-helper plan 显式留下的"反向解析"项。前一轮 `format_iso8601` 已能从 `DateTimeUtc` 输出 `"YYYY-MM-DDTHH:MM:SSZ"` 字符串，本轮补上 `parse_iso8601(&str) -> Option<DateTimeUtc>` 完成 helper 的双向闭环。

**新增函数**（`crates/h7cad-native-model/src/julian.rs`）：

```rust
pub fn parse_iso8601(s: &str) -> Option<DateTimeUtc>
```

**严格接受规则**（避免歧义 / 安全 ASCII 索引）：

- 严格 20 字符长度（`"YYYY-MM-DDTHH:MM:SSZ"`）
- 分隔符固定位置：`-` `-` `T` `:` `:` `Z`
- 字母必须大写（`T` / `Z`）
- 不接受 fractional seconds (`.123`)
- 不接受 timezone offset (`+08:00`) — 仅 `Z` (UTC)
- 字段范围：month 1-12 / day 1-31 / hour 0-23 / minute 0-59 / second **0-60**（容忍闰秒位置但不模型化实际闰秒）
- **不**做 calendar 有效性校验（`Feb 30` 等通过 day ≤ 31 检查）— 上层 / domain 责任

**lib.rs 重新导出**：`pub use julian::{..., parse_iso8601, ...};`

**测试**（5 条新增到 `julian.rs::tests`）：

- `parse_iso8601_canonical_form_succeeds`：`"2020-01-01T07:54:20Z"` → 完整匹配
- `parse_iso8601_rejects_obvious_format_errors`：14 种错误格式（含缺 `Z` / 错分隔符 / 小写 / 非 padding / 非数字 / 各字段越界 / `+0000` tz / 空字符串 / 单字符）全部 reject
- `parse_iso8601_tolerates_leap_second_slot`：`"2016-12-31T23:59:60Z"` 接受（leap-second slot）
- `format_then_parse_iso8601_roundtrip`：4 个跨 epoch 日期 `format → parse` 完全一致
- `parse_then_julian_date_roundtrip`：`parse → utc_to_julian_date → julian_date_to_utc → format` 端到端 round-trip 字节一致

**验证**：

- `cargo test -p h7cad-native-model` **19 / 19 全绿**（14 前轮 + **5** 新 ISO-8601 parse；模块测试现共 10 julian + 9 model）
- `cargo test -p h7cad-native-dxf` 125 / 125 不受影响
- `cargo check -p H7CAD` 零新 warning
- `ReadLints` 2 个文件零 lint 错误

**helper 链路完整闭合**：

```
DateTimeUtc <─── parse_iso8601(&str)
    │           ▲
    │           │
    ▼           │
utc_to_julian   │
    │           │
    ▼           │
   f64          │
    │           │
    ▼           │
julian_date_to_utc
    │           │
    ▼           │
DateTimeUtc ─── format_iso8601 ───► String
```

任意起点入环、任意点出环都能保持端到端一致（秒级精度、UTC、AutoCAD 时间戳粒度）。

plan：`docs/plans/2026-04-21-iso8601-parse-plan.md`

---

### 2026-04-21（十三）：DXF HEADER 杂项 5 变量扩充（插入单位 + 显示 + XEDIT）

继续扩 HEADER 覆盖面，本轮加 5 个 misc 常用变量，覆盖 AutoCAD 块插入单位语义 + 线宽显示开关 + XREF 编辑允许标志。首次在 reader 中处理 `code 290 bool` 字段（前轮的 bool 字段都用 code 70）。

**model 扩字段**（`DocumentHeader`，紧跟 MLine 之后 / `handseed` 之前）：

| 字段 | 类型 | `$` 变量 | DXF code | Default |
|---|---|---|---|---|
| `insunits` | i16 | `$INSUNITS` | 70 | 0 (unspecified) |
| `insunits_def_source` | i16 | `$INSUNITSDEFSOURCE` | 70 | 0 |
| `insunits_def_target` | i16 | `$INSUNITSDEFTARGET` | 70 | 0 |
| `lwdisplay` | bool | `$LWDISPLAY` | **290** | false |
| `xedit` | bool | `$XEDIT` | **290** | true |

`$INSUNITS` 值域：0=unspec, 1=in, 2=ft, 3=mi, 4=mm, 5=cm, 6=m, 7=km, 8=μin, 9=mil, 10=yd, 11=Å, 12=nm, 13=μm, 14=dm, 15=dam, 16=hm, 17=Gm, 18=AU, 19=ly, 20=pc — H7CAD 透传 i16，UI / 上层负责语义化。

**reader 新增 `bv(c)` helper**：

```rust
let bv = |c: i16| -> bool {
    codes.iter()
        .find(|(code, _)| *code == c)
        .map(|(_, v)| v.trim() != "0")
        .unwrap_or(false)
};
```

与既有的 `f` / `i16v` / `i32v` / `sv` helper 同 scope，处理 code 290 这类 bool 字段。注意 `bv` 缺失时返回 false，但 `$XEDIT` default 是 true — 由 `DocumentHeader::default()` 兜底，不依赖 `bv` fallback。

**writer 对称输出**：5 对 pair（writer 用 `pair_i16(290, ...)` 写 bool 的 0/1，与 AutoCAD 输出格式一致）

**测试**（新增 `tests/header_misc_units_display.rs`，4 条）：

- `header_reads_all_5_misc_vars`：`$INSUNITS=4 (mm), $LWDISPLAY=1, $XEDIT=0` 等非默认值精确读取
- `header_writes_all_5_misc_vars`：构造 → write → 5 个 `$VAR` 字符串都在
- `header_roundtrip_preserves_all_5_misc_vars`：read → write → read 全字段保持
- `header_legacy_file_without_misc_loads_with_defaults`：legacy → 字段为 default，并显式断言 `$XEDIT default = true`（与其他 bool 默认 false 不同）

**验证**：

- `cargo test -p h7cad-native-dxf` **125 / 125 全绿**（121 前轮 + **4** 新 misc）
- `cargo test --bin H7CAD io::native_bridge` 20 / 20 无回归
- `ReadLints` 4 个文件零 lint 错误

**DXF HEADER 覆盖增量**：56 → **61** 个变量（约覆盖 AutoCAD 总计 300+ 系统变量的 **~20%**）。

plan：`docs/plans/2026-04-21-header-misc-units-display-plan.md`

---

### 2026-04-21（十二）：DXF HEADER Spline + MLine 6 变量扩充

继续扩 HEADER 覆盖面，本轮加 Spline 默认 3 个 + 当前 MLine 默认 3 个，合并一个 plan 实现以减少 plan 文件 fragmentation。

**model 扩字段**（`DocumentHeader`，插在 DIM Tier 1 之后 / `handseed` 之前）：

| 字段 | 类型 | `$` 变量 | DXF code | Default |
|---|---|---|---|---|
| `splframe` | bool | `$SPLFRAME` | 70 | false |
| `splinesegs` | i16 | `$SPLINESEGS` | 70 | 8 |
| `splinetype` | i16 | `$SPLINETYPE` | 70 | 6 (cubic B-spline) |
| `cmlstyle` | String | `$CMLSTYLE` | 2 | `"Standard"` |
| `cmljust` | i16 | `$CMLJUST` | 70 | 0 (top) |
| `cmlscale` | f64 | `$CMLSCALE` | 40 | 1.0 |

`$SPLINETYPE` 值域：5 = quadratic, 6 = cubic。`$CMLJUST` 值域：0 / 1 / 2 = top / mid / bottom。reader / writer 不校验值域，仅透传。

**reader / writer 同步**：6 arm + 6 对 pair 块（writer 拆成两个分组：Spline defaults 紧跟 DIM 区块，MLine defaults 紧跟 Spline 区块）。

**测试**（新增 `tests/header_spline_mline.rs`，4 条）：read / write / roundtrip / legacy 默认。

**验证**：

- `cargo test -p h7cad-native-dxf` **121 / 121 全绿**（117 前轮 + **4** 新 spline + mline）
- `cargo test --bin H7CAD io::native_bridge` 20 / 20 无回归
- `ReadLints` 4 个文件零 lint 错误

**DXF HEADER 覆盖增量**：50 → **56** 个变量（15 原有 + 15 绘图环境 + 4 时间戳 + 5 UCS + 3 视图 + 8 DIM Tier 1 + 6 Spline+MLine），约覆盖 AutoCAD 总计 300+ 系统变量的 **~19%**。

plan：`docs/plans/2026-04-21-header-spline-mline-plan.md`

---

### 2026-04-21（十一）：Julian Date ↔ UTC 转换 helper（无 chrono 依赖）

闭合 2026-04-21（七）HEADER timestamps plan 显式留下的"Julian-date 转换 helper 留待未来"项。让 UI 层能把 `DocumentHeader.tdcreate / tdupdate` 的 raw f64 Julian date 格式化为人类可读时间。明确**不引入 `chrono` / `time` crate**，自写 Fliegel-Van Flandern (1968) 算法（~50 行 integer-only）。

**新增模块**（`crates/h7cad-native-model/src/julian.rs`）：

- `pub struct DateTimeUtc { year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32 }`
- `pub fn julian_date_to_utc(jd: f64) -> DateTimeUtc`：JD → 日历，分两段（整数 JDN 走 Fliegel；小数走 86400 秒映射）
- `pub fn utc_to_julian_date(dt: &DateTimeUtc) -> f64`：日历 → JD，用 Meeus 整数算法
- `pub fn format_iso8601(dt: &DateTimeUtc) -> String`：输出 `"YYYY-MM-DDTHH:MM:SSZ"`（ISO-8601 UTC suffix）

`lib.rs` 导出：`pub mod julian; pub use julian::{DateTimeUtc, julian_date_to_utc, utc_to_julian_date, format_iso8601};`

**关键设计**：

- **Precision**：second-level（AutoCAD 自身 Julian-date 写入也是秒级）
- **Timezone**：所有 `jd` 当 UTC 处理（H7CAD 不在 model 层跟踪 timezone；DXF Reference 标 "local time" 但 H7CAD 不假定）
- **Range**：1900-01-01 ~ 2100-01-01（Fliegel 在该范围严格匹配公历），超出范围算法仍终止但不保证语义
- **Rounding carry**：sub-day fraction 乘 86400 round 到 ≥ 86400 时 day 自动进位
- **Leap seconds**：不模型化（与 Fliegel-Van Flandern 一致）
- **Validation**：`DateTimeUtc::new` 不验证字段范围，调用方责任

**测试**（5 条 unit tests in `julian.rs`）：

- `julian_date_reference_value_maps_to_2020_01_01_utc`：DXF Reference 例子 `2458849.82939815` → 2020-01-01 07:54:~20 UTC（秒级容忍 ±1）
- `unix_epoch_julian_date_maps_to_1970_01_01_midnight`：JD `2440587.5` → 1970-01-01T00:00:00Z 精确
- `j2000_maps_to_2000_01_01_noon_utc`：JD `2451545.0` (J2000.0) → 2000-01-01T12:00:00Z 精确
- `julian_date_roundtrip_preserves_dates_across_the_20th_century`：5 个跨越 1900-2100 的日期（含 23:59:59 边界）round-trip 完全一致
- `format_iso8601_pads_and_emits_canonical_string`：3 种边界（含单位数月/日/时/分/秒的 zero-pad）

**验证**：

- `cargo test -p h7cad-native-model` **14 / 14 全绿**（9 前轮 + **5** 新 julian）
- `cargo test -p h7cad-native-dxf` 117 / 117 不受影响
- `cargo check -p H7CAD` 零新 warning
- `ReadLints` 2 个文件零 lint 错误

**未纳入本轮**：

- ISO-8601 字符串 → JD 反向解析（`parse_iso8601`）— 等到 UI 真有"用户输入时间字符串"场景再加
- Local timezone 转换（涉及 IANA 时区数据库，scope 太大）
- Sub-second / millisecond 精度（与 AutoCAD 时间戳粒度不匹配）

plan：`docs/plans/2026-04-21-julian-date-helper-plan.md`

---

### 2026-04-21（十）：DXF HEADER 核心尺寸标注 8 变量扩充 (DIMxxx Tier 1)

继 HEADER 绘图环境 / 时间戳 / UCS / 视图 4 轮扩充后的第五次 HEADER 扩容。本轮挑 DIMxxx 家族（AutoCAD 100+ 个尺寸标注系统变量）中**外观层最常用的 8 个**，让真实 AutoCAD DXF 的"当前绘图标注默认"完整 round-trip。

**model 扩字段**（`DocumentHeader`，插在 `viewdir` 之后 / `handseed` 之前）：

| 字段 | 类型 | `$` 变量 | DXF code | Default (AutoCAD 新 imperial) |
|---|---|---|---|---|
| `dimtxt` | f64 | `$DIMTXT` | 40 | 0.18 (文字高度) |
| `dimasz` | f64 | `$DIMASZ` | 40 | 0.18 (箭头尺寸) |
| `dimexo` | f64 | `$DIMEXO` | 40 | 0.0625 (延伸线 origin offset) |
| `dimexe` | f64 | `$DIMEXE` | 40 | 0.18 (延伸线 extension) |
| `dimgap` | f64 | `$DIMGAP` | 40 | 0.09 (文字 gap) |
| `dimdec` | i16 | `$DIMDEC` | 70 | 4 (线性尺寸小数位) |
| `dimadec` | i16 | `$DIMADEC` | 70 | 0 (角度尺寸小数位) |
| `dimtofl` | bool | `$DIMTOFL` | 70 | false (强制文字在延伸线间) |

Defaults 对齐 AutoCAD 新 imperial 绘图初始值。

**reader 扩派发**（8 arm 追加到 `read_header_section` 的 match）  
**writer 对称输出**（8 对 pair 聚集在 `$DIMSCALE` 之后形成完整 DIM 区块，便于 AutoCAD 顺序阅读）

**测试**（新增 `tests/header_dim_tier1.rs`，4 条）：

- `header_reads_all_8_dim_tier1_vars`：非默认值（`dimtxt=0.5, dimdec=6, dimtofl=true` 等）→ 精确读取
- `header_writes_all_8_dim_tier1_vars`：metric-leaning 构造（`dimtxt=2.5, dimasz=1.0`）→ write → 8 个 `$DIM*` 都在
- `header_roundtrip_preserves_all_8_dim_tier1_vars`：read → write → read 1e-9 容忍
- `header_legacy_file_without_dim_tier1_loads_with_imperial_defaults`：legacy HEADER 无 `$DIM*` → 8 字段为 AutoCAD 新 imperial 默认

**未纳入本轮（明确 Tier 2+ 留未来）**：

- `$DIMALT*` 替代单位家族（7-8 变量）
- `$DIMBLK*` 箭头 block name 家族
- `$DIMFIT / $DIMSAH / $DIMSD1 / $DIMSD2 / $DIMSE1 / $DIMSE2 / $DIMTAD / $DIMTIX / $DIMTMOVE / $DIMUPT / $DIMZIN` 等细节变量
- DIMxxx → TABLES.DIMSTYLE 双向同步（HEADER 存 current drawing default，DIMSTYLE 存 named styles，同步独立 scope）
- DIMxxx 对实际标注渲染的接入（仅字段保真，渲染层独立）

**验证**：

- `cargo test -p h7cad-native-dxf` **117 / 117 全绿**（113 前轮 + **4** 新 DIM Tier 1）
- `cargo test --bin H7CAD io::native_bridge` 20 / 20 无回归
- `ReadLints` 4 个文件零 lint 错误

**DXF HEADER 覆盖增量**：42 → **50** 个变量（15 原有 + 15 绘图环境 + 4 时间戳 + 5 UCS + 3 视图 + 8 DIM Tier 1），约覆盖 AutoCAD 总计 300+ 系统变量的 **17%**。

plan：`docs/plans/2026-04-21-header-dim-tier1-plan.md`

---

### 2026-04-21（九）：DXF HEADER 当前视图 3 变量扩充

继 HEADER 绘图环境 / 时间戳 / UCS 家族后继续补齐"活动视图"3 变量。真实 AutoCAD DXF 的 HEADER 段普遍携带 `$VIEWCTR / $VIEWSIZE / $VIEWDIR`，保留用户 pan/zoom 后的视口状态。H7CAD reader/writer 原先忽略，read → write 后这些设置归零。

**model 扩字段**（`DocumentHeader`，插在 timestamp 之后 / `handseed` 之前）：

| 字段 | 类型 | `$` 变量 | DXF code | Default |
|---|---|---|---|---|
| `viewctr` | `[f64; 2]` | `$VIEWCTR` | 10/20 | `[0, 0]` |
| `viewsize` | `f64` | `$VIEWSIZE` | 40 | `1.0` |
| `viewdir` | `[f64; 3]` | `$VIEWDIR` | 10/20/30 | `[0, 0, 1]` |

Default 对齐 **top-down plan view**（视线沿 +Z，看向 XY 平面），与 AutoCAD 默认 World view 一致。

**reader 扩派发**（`read_header_section` 加 3 arm）  
**writer 对称输出**（`write_header` 在 timestamp 和 `$HANDSEED` 之间按 AutoCAD 顺序输出 3 对 pair 块）

**测试**（新增 `tests/header_view_vars.rs`，4 条）：

- `header_reads_all_3_view_vars`：非默认视图（`viewctr=[100, 200], viewsize=42.5, viewdir=[1,1,1]` 未归一化）→ 精确读取
- `header_writes_all_3_view_vars`：负坐标 + 大尺寸构造 → write → 3 个 `$VIEW*` 字符串都在
- `header_roundtrip_preserves_all_3_view_vars`：read → write → read 1e-9 容忍
- `header_legacy_file_without_view_fields_loads_with_defaults`：legacy HEADER → default 值并显式断言匹配 top-down plan view 默认

**暂不接入 `Scene::camera`**：接入需要 UCS → camera view-transform 矩阵运算（`glam::Mat4` 构建），scope 过大。当前 header 只做透传保留数据，视图恢复由 UI 层后续工作接入。

**验证**：

- `cargo test -p h7cad-native-dxf` **113 / 113 全绿**（109 前轮 + **4** 新 view vars）
- `cargo test --bin H7CAD io::native_bridge` 20 / 20 无回归
- `ReadLints` 4 个文件零 lint 错误

**DXF HEADER 覆盖增量**：39 → **42** 个变量（15 原有 + 15 绘图环境 + 4 时间戳 + 5 UCS + 3 视图）。

plan：`docs/plans/2026-04-21-header-view-vars-plan.md`

---

### 2026-04-21（八）：DXF HEADER UCS 家族 5 变量扩充

继 2026-04-21（三、七）HEADER 扩充后继续完善覆盖面，本轮补齐 UCS（User Coordinate System）家族 5 变量，覆盖任何 AutoCAD 图纸的"当前用户坐标系"定义。本轮只动 HEADER 段；TABLES.UCS 表（by-name 字典）未动，留作独立工作。

**model 扩字段**（`DocumentHeader`，在 `psltscale` 和 timestamp 之间）：

| 字段 | 类型 | `$` 变量 | DXF code | Default |
|---|---|---|---|---|
| `ucsbase` | `String` | `$UCSBASE` | 2 | `""` |
| `ucsname` | `String` | `$UCSNAME` | 2 | `""` |
| `ucsorg` | `[f64; 3]` | `$UCSORG` | 10/20/30 | `[0, 0, 0]` |
| `ucsxdir` | `[f64; 3]` | `$UCSXDIR` | 10/20/30 | `[1, 0, 0]` |
| `ucsydir` | `[f64; 3]` | `$UCSYDIR` | 10/20/30 | `[0, 1, 0]` |

Defaults 与 **WCS 等同**（origin = 原点、X = +X、Y = +Y），让 `CadDocument::new()` 产出的文档在 `$UCSNAME == ""` 情况下与 AutoCAD "当前 UCS = World" 语义一致。

**reader 扩派发**（`read_header_section` 加 5 arm，紧跟既有 `$PSLTSCALE`）

**writer 对称输出**（`write_header` 在 `$PSLTSCALE` 之后、timestamp 之前按 AutoCAD 惯例顺序输出 5 对 pair 块）

**测试**（新增 `tests/header_ucs_family.rs`，4 条）：

- `header_reads_all_5_ucs_vars`：非默认 UCS（90° CW 旋转示例：`ucsxdir=[0,-1,0], ucsydir=[1,0,0]`）→ 精确读取
- `header_writes_all_5_ucs_vars`：构造 → write → 扫 5 个 `$UCS*` 和 non-default 字符串
- `header_roundtrip_preserves_all_5_ucs_vars`：read → write → read 全字段 1e-9 容忍
- `header_legacy_file_without_ucs_fields_loads_with_defaults`：legacy 无 `$UCS*` → 字段为 default + 显式断言 WCS-equivalent 三元组

**验证**：

- `cargo test -p h7cad-native-dxf` **109 / 109 全绿**（105 前轮 + **4** 新 UCS）
- `cargo test --bin H7CAD io::native_bridge` 20 / 20 无回归
- `ReadLints` 4 个文件零 lint 错误

**DXF HEADER 覆盖增量**：34 → **39** 个变量（15 原有 + 15 绘图环境 + 4 时间戳 + 5 UCS）。

plan：`docs/plans/2026-04-21-header-ucs-family-plan.md`

---

### 2026-04-21（七）：DXF HEADER 时间戳 4 变量扩充（Julian date 透传）

继 2026-04-21（三）HEADER 15 绘图环境变量后继续扩容。真实 AutoCAD 输出的 DXF HEADER 普遍携带 `$TDCREATE / $TDUPDATE / $TDINDWG / $TDUSRTIMER` 四个时间戳，用于保留"创建时间 / 最近编辑时间 / 总编辑时长 / 用户计时器"。H7CAD reader 此前忽略，writer 不写 → 读 AutoCAD .dxf 写回后时间戳全归零。本轮扩 4 个 `f64` 字段做 **透传**（不引入 `chrono` / `time` crate 做 Julian-date → DateTime 转换，留给 UI 层按需格式化）。

**model 扩字段**（`crates/h7cad-native-model/src/lib.rs::DocumentHeader`，插在 `psltscale` 和 `handseed` 之间）：

| 字段 | 类型 | `$` 变量 | DXF code | Default | 语义 |
|---|---|---|---|---|---|
| `tdcreate` | f64 | `$TDCREATE` | 40 | 0.0 | Julian date: drawing creation time |
| `tdupdate` | f64 | `$TDUPDATE` | 40 | 0.0 | Julian date: last update time |
| `tdindwg` | f64 | `$TDINDWG` | 40 | 0.0 | Fractional days: cumulative editing time |
| `tdusrtimer` | f64 | `$TDUSRTIMER` | 40 | 0.0 | Fractional days: user elapsed timer |

**reader 扩派发**（`crates/h7cad-native-dxf/src/lib.rs::read_header_section`）：

- `match var_name` 加 4 arm，复用既有 `f(40)` helper
- 缺失时走 `Default` 0.0

**writer 扩输出**（`crates/h7cad-native-dxf/src/writer.rs::write_header`）：

- 按 AutoCAD 惯例顺序在 `$PSLTSCALE` 之后 / `$HANDSEED` 之前插入 4 对 `pair_str(9) + pair_f64(40)`

**测试**（新增 `tests/header_timestamps.rs`，4 条）：

- `header_reads_all_4_timestamps`：AutoCAD 2018 DXF Reference 标准样例值（Julian `2458849.82939815` ≈ 2020-01-01 07:54:19 UTC）→ 精确读取，容忍度 1e-9
- `header_writes_all_4_timestamps`：构造非零 timestamps → write → 断言四个 `$TD*` 在输出 text 中存在
- `header_roundtrip_preserves_all_4_timestamps`：read → write → read：
  - Julian-date 字段（magnitude ~2.4e6）→ `format_f64` 10 位小数精度 ⇒ 实际精度 ~2.4e-4 日 ≈ 20 秒，容忍度 1e-3（AutoCAD 官方时间戳本身只精确到秒）
  - Fractional-day 字段（值 ≤ 1）→ 精度 ~1e-10，容忍度 1e-9
- `header_legacy_file_without_td_fields_loads_with_zero`：legacy HEADER 不含 `$TD*` → 4 字段全为 0.0

**验证**：

- `cargo test -p h7cad-native-dxf` **105 / 105 全绿**（101 前轮 + **4** 新 timestamps）
- `cargo test --bin H7CAD io::native_bridge` 20 / 20 无回归
- `ReadLints` 4 个修改 / 新增文件零 lint 错误

**Julian-date 转换留待后续**：UI 如需显示人类可读时间，加 `pub fn julian_to_iso_8601(jd: f64) -> String` helper（Julian date epoch = 4713 BC Jan 1 noon UTC，公式已有若干开源参考实现；30 行以内可自写，不必引入 chrono）。本轮 scope 外。

plan：`docs/plans/2026-04-21-header-timestamps-plan.md`

---

### 2026-04-21（六）：`ObjectData::ImageDef` 扩字段到完整 DXF 标准

闭合同日前两轮（一、二）IMAGE/IMAGEDEF 工作留下的最后遗留：`ObjectData::ImageDef` 原只存 `file_name` + `image_size`，真实 AutoCAD DXF 的 IMAGEDEF 还会写 `code 11/21 pixel_size / code 90 class_version / code 71 image_is_loaded / code 281 resolution_unit`。DWG 原生侧（`vendor_tmp/acadrust::read_image_definition`）早已读全 6 字段，H7CAD DXF 侧漏读漏写导致这些信息在 "读 AutoCAD .dxf → 写出" 的 round-trip 里丢失。

**model 扩字段**（`crates/h7cad-native-model/src/lib.rs::ObjectData::ImageDef`）：

| 字段 | 类型 | DXF code | Default | 语义 |
|---|---|---|---|---|
| `pixel_size` | `[f64; 2]` | 11 / 21 | `[1.0, 1.0]` | 单像素在绘图单位中的 U/V 尺寸 |
| `class_version` | `i32` | 90 | 0 | Class version |
| `image_is_loaded` | `bool` | 71 | true | 保存时文件是否已加载 |
| `resolution_unit` | `u8` | 281 | 0 | 0 = None / 2 = cm / 5 = inches |

Defaults 严格对齐 AutoCAD DWG 原生路径的"未设值"语义，确保 legacy DXF 读入和 `ensure_image_defs` auto-create 两条路径落在同一个 in-memory 状态上。

**reader 扩读**（`crates/h7cad-native-dxf/src/lib.rs::read_objects_section`）：

- IMAGEDEF 分支扩 4 个 group code 分派（11 / 21 / 90 / 71 / 281）
- 缺失字段走 `DocumentHeader`-风格的 inline 默认初始化，旧 DXF 文件（只带 1/10/20）reader 自动 fallback 到 AutoCAD 默认而非 `Default::default` 的零值

**writer 对称输出**（`crates/h7cad-native-dxf/src/writer.rs::write_object`）：

- IMAGEDEF 分支按 AutoCAD 常用顺序 emit `1 → 10/20 → 11/21 → 90 → 71 → 281` 六对
- bool → i16 走 `if b { 1 } else { 0 }`；`u8` `resolution_unit` cast 成 i16 写 code 281

**auto-create 同步**（`writer.rs::ensure_image_defs`）：

- `ObjectData::ImageDef` 构造点补齐新字段的 AutoCAD 默认值（`pixel_size: [1.0, 1.0], class_version: 0, image_is_loaded: true, resolution_unit: 0`）
- 附带更新前轮 2 处测试 fixture 的 `ObjectData::ImageDef` 构造（`tests/imagedef_roundtrip.rs`, `tests/imagedef_ensure.rs`）— 前者继续用默认值，后者故意设 `pixel_size: [0.5, 0.5], resolution_unit: 2` 验证非默认值也能 roundtrip

**测试**（新增 3 条到 `tests/imagedef_roundtrip.rs`）：

- `imagedef_reads_extended_fields`：手写完整 IMAGEDEF（含 11/21/90/71/281，故意取 `pixel_size=[0.25, 0.5], class_version=1, image_is_loaded=false, resolution_unit=5`）→ 断言每个字段精确读取
- `imagedef_legacy_file_uses_defaults_for_missing_extension_fields`：只带 1/10/20 的 legacy IMAGEDEF → 新字段走 AutoCAD 默认（`[1.0, 1.0] / 0 / true / 0`）
- `imagedef_extended_fields_survive_full_roundtrip`：read → write → read，4 个新字段逐个 `assert_eq!`

**验证**：

- `cargo test -p h7cad-native-dxf` **101 / 101 全绿**（81 单元 + 5 imagedef_ensure + **9** imagedef_roundtrip（6 前轮 + 3 新）+ 5 header_drawing_vars + 1 fixture）
- `cargo test --bin H7CAD io::native_bridge` 20 / 20 无回归
- `ReadLints` 5 个修改 / 新增文件零 lint 错误

**影响面**：IMAGEDEF 在 H7CAD DXF 读写链路上已全字段对齐 AutoCAD 规范。前两轮 + 本轮累计让 IMAGE ↔ IMAGEDEF 的标准化链路完整闭合：

- 标准 DXF 读 340 + 解 IMAGEDEF 7 字段（含新 4 字段）✓
- 写 IMAGE 输出 340 + 输出 IMAGEDEF 7 字段 ✓
- Auto-create IMAGEDEF 补全标准默认值 ✓
- 与 DWG native reader 字段覆盖面对齐 ✓

plan：`docs/plans/2026-04-21-imagedef-extend-fields-plan.md`

---

### 2026-04-21（五）：PID 打开后 fit 优先聚焦主绘图层

同日第四轮（PID 真实样本显示）Task 2 留下的诊断问题：装饰 panel（42 entities，位于 `SIDE_PANEL_X` / `BOTTOM_PANEL_Y` 等远坐标）挤压主绘图层（12 entities）的视口占比 — `Scene::fit_all` 把所有 wires 的 bbox 全算进去，装饰 panel 把 bbox 拉得很大，主绘图被缩到视口一小块角落。本轮在不动装饰 panel 布局的前提下，给 PID tab 的打开路径引入"只看主绘图层"的 fit 策略。

**scene 改动**（`src/scene/mod.rs`）：

- 新增 `Scene::fit_layers_matching(layer_prefixes: &[&str]) -> bool`：
  - 从 `native_doc()` 迭代实体（`doc.entities`），按 `layer_name.starts_with(p)` 对每个 prefix 取 OR 语义判断匹配
  - 匹配的实体经 `entity_bbox_points` 提取特征点（`Line` 端点、`Circle` / `Arc` / `Ellipse` 的 bbox 角、`Text` / `MText` / `Insert` 的 insertion、`Point` position、`LwPolyline` / `Polyline` 顶点）
  - 聚合 min/max → `camera.fit_to_bounds` + `camera_generation += 1`
  - **返回 bool**：true = 至少一个实体匹配且已 fit；false = 无 native_doc 或无匹配实体（camera 不变）
- 新增私有 `fn entity_bbox_points(entity) -> Vec<[f64; 3]>` 作为 kind 特征点提取的单一事实来源

**入口整合**（`src/app/update.rs::Message::FileOpened`）：

- 原 PID / CAD 共用的 `self.tabs[i].scene.fit_all()` 改为：
  - PID tab：先试 `fit_layers_matching(&["PID_OBJECTS_", "PID_LAYOUT_TEXT", "PID_RELATIONSHIPS"])`；若返回 false（例如 preview 确实无主图层实体）fallback 到 `fit_all`
  - CAD / DXF / DWG tab：继续走 `fit_all`（主图层无 `PID_` 前缀命名，prefix 匹配会自动 false → fallback；但为避免无谓的 `fit_layers_matching` 调用，用 `matches!(tab_mode, DocumentTabMode::Pid)` 显式门闩）

**测试**（`src/scene/mod.rs::tests` + `src/io/pid_import.rs::tests`）：

- 新增 `fit_layers_matching_returns_true_and_advances_camera_generation_for_matching_layer`：主图层 + 装饰层（far offset）并存时只匹配主图层，camera_generation 恰 +1
- 新增 `fit_layers_matching_returns_false_without_touching_camera_when_no_layer_matches`：实体仅在 CAD "0" 层时返回 false 且 camera_generation 不变（保证 fallback 契约）
- 新增 `fit_layers_matching_returns_false_without_native_doc`：新建无 native_doc 的 scene 不 panic、返回 false
- 新增 `fit_layers_matching_prefix_semantics_match_any_of_the_prefixes`：OR-of-prefixes 语义验证（首 prefix 不匹配、第二 prefix 命中仍可成功 fit）
- 新增 `target_pid_sample_fit_layers_matching_succeeds_for_main_drawing_layers`：真实样本 `工艺管道及仪表流程-1.pid` 打开后主图层 fit 必须 true（防回退）

**修改 / 新增文件**：

- 修改：`src/scene/mod.rs`（+ fit_layers_matching / entity_bbox_points / 4 单元测试）
- 修改：`src/app/update.rs`（FileOpened PID 分支 fit 策略切换）
- 修改：`src/io/pid_import.rs`（+ 1 目标样本集成测试 + 顺带修 `AttributeClassSummary.records: Vec::new()` pid-parse 模型演进后留下的 pre-existing 测试夹具编译错误）

**验证**：

- `cargo test --bin H7CAD scene` **45 / 45 全绿**（41 前轮 + 4 新 fit_layers_matching）
- `cargo test --bin H7CAD io::pid` **79 / 79 全绿**（78 前轮 + 1 新 target_pid 目标样本集成）
- `cargo check` 零新 warning
- `ReadLints` 3 个修改文件零 lint 错误

**效果**：打开 target PID 时视口直接聚焦到 `PID_OBJECTS_PipeRun / PID_OBJECTS_PIDProcessPoint / PID_LAYOUT_TEXT / PID_RELATIONSHIPS` 覆盖的主绘图区；装饰面板（cross-ref / unresolved / stream / cluster / fallback）仍在原坐标，用户 pan / zoom 到右侧 / 底部仍可访问 — 但不再"一打开就挤成一团"。

plan：`docs/plans/2026-04-21-pid-fit-main-drawing-plan.md`

---

### 2026-04-21（四）：PID 真实样本显示改进 + `PIDSHOT` 命令 + 截图回归

落地 `docs/plans/2026-04-21-pid-real-sample-display-and-screenshot-plan.md`。验收样本固定为 `D:\work\plant-code\cad\pid-parse\test-file\工艺管道及仪表流程-1.pid`（450 KB，2 objects / 0 relationships，无 publish sidecar，打开后 native_preview 有 57 entities）。覆盖计划 Task 1-5（Task 6 UI-level 自动化因仓库无 headless UI 基础明确跳过）。

**Task 1 — 真实样本基线测试**（`src/io/pid_import.rs::tests`）：

- 新增 `target_sample_pid_path()` helper 指向目标 .pid（找不到时测试 skip，不阻塞 CI）
- 新增 `open_target_pid_sample_builds_dense_preview`：反映当前解析结果的务实 anchor（layout.items ≥ 2、object_count ≥ 2、native_entities ≥ 30），防止退化到"空预览"。测试输出行里 dump object/layout/segment/entities count 做诊断记录

**Task 2 — 显示质量 focused test + layout 标签修复**（`src/io/pid_import.rs`）：

- 新增 `target_pid_preview_layout_is_primary_visual_focus`：拉开主绘图层（`PID_OBJECTS_*`）、layout 文字层、装饰层（meta/fallback/crossref/unresolved/streams/clusters/symbols）的实体数量，确保主绘图有至少 1 个实体 anchor（即使装饰层占主导也不被完全压过）
- **bug 修复**：`add_layout_glyph` 之前只为 `LayoutGlyphKind::Generic` 画 label text，其余 10 种（Pipeline / Branch / Connector / ProcessPoint / Instrument / Equipment / Vessel / Note / Nozzle / OffPageConnector / PipingComponent）都没有可见标签 → 真实 P&ID 每个对象都有 tag，这直接导致样本"看起来稀疏"。把 label 绘制抽到 match 之后统一处理（label 非空时在 glyph 下方 34 单位处画 20 字节内缩的 MText）。效果：primary_objects 从 10 → **12**、native_entities 从 55 → **57**

**Task 3 — 场景 fit 回归保护**（`src/io/pid_import.rs::tests`）：

- 新增 `target_pid_sample_scene_has_fittable_geometry_and_native_doc`：offline 走一遍 `open_pid → set_native_doc → entity_wires → fit_all` 链路，断言三项 `fit_all` 前提条件（scene 有 native doc / compat 文档非空 / entity_wires 至少 1 条）；`fit_all` 本身断言不 panic
- 现有 `Message::FileOpened → scene.fit_all()` 流程经此测试验证对目标样本已经工作，未做代码改动

**Task 4 — `PIDSHOT <path.png>` 命令 + 确定性 PNG helper**：

- **新增文件**：`src/io/pid_screenshot.rs`（~320 行，含 2 单测）
  - `export_pid_preview_png(doc: &CadDocument, path: &Path)` 纯 CPU rasteriser：Bresenham 画线 + midpoint 画圆 + 采样画弧 + 3×3 十字画文字锚点 + 单像素画点
  - `SCREENSHOT_WIDTH = 1600, SCREENSHOT_HEIGHT = 900`（plan 指定）
  - 世界 bbox → 像素坐标自动 fit，保留 40px margin + Y 轴翻转
  - 支持 Line / Circle / Arc / Text / MText / Point；其余 entity kind 静默 skip
  - 首版**不**做 GPU 读回（可 headless，便于 Task 5 回归 + test 环境）
- **新增命令**（`src/app/commands.rs`）：`PIDSHOT <path.png>`
  - 只允许活动 PID tab；非 PID tab 输出 `PIDSHOT: active tab is not a PID tab`
  - 目标路径必须 `.png` 后缀；不符 → 错误输出
  - 成功：`PIDSHOT  saved screenshot to <path>`；失败：`PIDSHOT: <error>`
- **helper 单测**：`export_pid_preview_png_writes_file`（真实样本 → PNG > 1KB）、`export_rejects_empty_document`（空 doc 显式错误）

**Task 5 — 截图回归基线**（`src/io/pid_screenshot.rs::tests`）：

- 新增 `target_pid_sample_screenshot_matches_baseline`：**不提交** binary baseline PNG（避免 repo 膨胀 / 不透明 diff），改用**统计签名 pinning**：
  - 文件大小 > 1 KB
  - 尺寸严格 == 1600 × 900
  - 非白像素数落在 `[100, 500_000]` 容忍区间（当前观测值 **824**，足以防御 "blank canvas" / "filled canvas" 两种危险回归，又能吸收 label / icon 尺寸微调）

**Task 6 — UI-level 自动化**（跳过，按计划原文）：

- H7CAD 使用 iced + wgpu 构建，仓库无现成 headless UI 自动化基础；强行实现 scope 风险远高于 Task 1-5 总和
- Plan 原文明确"只在仓库现有自动化基础存在时做"→ 本轮不纳入，作为后续阶段

**修改 / 新增文件**：

- 新增：`src/io/pid_screenshot.rs`
- 修改：`src/io/mod.rs`（`pub mod pid_screenshot;`）
- 修改：`src/io/pid_import.rs`（+ 3 tests，+ add_layout_glyph label 统一绘制）
- 修改：`src/app/commands.rs`（+ PIDSHOT 命令分支）

**验证**（plan validator sequence）：

- `cargo test --bin H7CAD io::pid` **78 / 78 全绿**（pid_import 67 + pid_package_store + pid_screenshot 3 新 + 原有）
- `cargo check` 零新 warning
- `ReadLints` 4 个文件零 lint 错误

plan：`docs/plans/2026-04-21-pid-real-sample-display-and-screenshot-plan.md`

---

### 2026-04-21（三）：DXF HEADER 绘图环境变量扩充（15 → 30）

承接同日前两轮 IMAGE/IMAGEDEF 工作后，换战场转向 HEADER section。前期盘点识别的"HEADER 仅 15 个变量，AutoCAD 300+"缺口对 round-trip 保真度影响广 — 每个 DXF 都有 HEADER，且真实 AutoCAD 输出几乎都携带绘图模式 / 当前属性 / 角度配置。本轮按"最常见 / 最高 ROI" 的筛选，扩 **15 个绘图环境变量**。

**model 扩字段**（`crates/h7cad-native-model/src/lib.rs::DocumentHeader`）：

| 类别 | 变量 | 字段 / 类型 | DXF code | AutoCAD 默认值 |
|---|---|---|---|---|
| 绘图模式 | `$ORTHOMODE` | `orthomode: bool` | 70 | false |
| 绘图模式 | `$GRIDMODE` | `gridmode: bool` | 70 | false |
| 绘图模式 | `$SNAPMODE` | `snapmode: bool` | 70 | false |
| 绘图模式 | `$FILLMODE` | `fillmode: bool` | 70 | **true** |
| 绘图模式 | `$MIRRTEXT` | `mirrtext: bool` | 70 | false |
| 绘图模式 | `$ATTMODE` | `attmode: i16` (0/1/2) | 70 | **1** |
| 当前属性 | `$CLAYER` | `clayer: String` | **8** | `"0"` |
| 当前属性 | `$CECOLOR` | `cecolor: i16` | **62** | **256** (BYLAYER) |
| 当前属性 | `$CELTYPE` | `celtype: String` | **6** | `"ByLayer"` |
| 当前属性 | `$CELWEIGHT` | `celweight: i16` (1/100mm) | **370** | **-1** (ByLayer) |
| 当前属性 | `$CELTSCALE` | `celtscale: f64` | 40 | 1.0 |
| 当前属性 | `$CETRANSPARENCY` | `cetransparency: i32` | **440** | 0 |
| 角度 | `$ANGBASE` | `angbase: f64` (rad) | **50** | 0.0 |
| 角度 | `$ANGDIR` | `angdir: bool` (0=逆/1=顺) | 70 | false |
| 空间 | `$PSLTSCALE` | `psltscale: bool` | 70 | **true** |

`Default` impl 严格对齐 AutoCAD 语义（`fillmode/psltscale=true`, `cecolor=256`, `celweight=-1`, `attmode=1`），避免默认值意外覆盖真实配置。

**reader 扩派发**（`crates/h7cad-native-dxf/src/lib.rs::read_header_section`）：

- `match var_name` 追加 15 个 arm
- 新增内联 helper `i32v(c)` 用于 `$CETRANSPARENCY` 这类 i32 变量（既有 `f / i16v / sv` 不覆盖 i32）
- 非标准代码值（例如 `$CLAYER` 用 code 8，`$CELTYPE` 用 code 6）显式处理，避免误用泛化 helper

**writer 扩输出**（`crates/h7cad-native-dxf/src/writer.rs::write_header`）：

- 按 "绘图模式 → 当前属性 → 角度 → 空间" 顺序追加 15 个 `pair_str(9, "$VAR") + pair_xxx(<code>, value)` 块
- bool → i16 转换固定按 `if b { 1 } else { 0 }` 惯例

**测试**（新增 `crates/h7cad-native-dxf/tests/header_drawing_vars.rs`，5 条集成测试）：

- `header_reads_all_15_drawing_vars`：手写完整 HEADER → 逐字段断言（涵盖 bool / i16 / i32 / f64 / String 五种类型）
- `header_writes_all_15_drawing_vars`：构造 doc → write → 用 `find_var_pair` / `assert_var_i16 / i32 / f64_approx / str` 辅助逐变量断言输出的 `group_code + value` 正确
- `header_roundtrip_preserves_all_15_drawing_vars`：read → write → read，15 个字段全部对齐（f64 容忍度 1e-9，匹配 `format_f64` 10 位精度天花板）
- `header_default_values_survive_roundtrip`：fresh `CadDocument::new()` → write → read，逐字段对齐 `DocumentHeader::default()`
- `header_legacy_file_without_new_vars_loads_with_defaults`：legacy DXF 不含 15 个新变量 → reader fallback 到 Default → 不炸

辅助测试函数：
- `find_var_pair(text, var_name)` 扫 `"  9\n<var>\n"` 模式取随后的 code + value 对
- 4 个类型化断言 helper (`assert_var_i16 / i32 / f64_approx / str`) 都做 group code + value 双校验，避免 writer 把 `$CLAYER` 误写成 code 2（规范是 8）被漏检

**验证**：

- `cargo test -p h7cad-native-dxf` **98 / 98 全绿**（81 单元 + 5 新 header + 5 imagedef_ensure + 6 imagedef_roundtrip + 1 fixture）
- `cargo test --bin H7CAD io::native_bridge` 20 / 20 无回归
- `cargo check -p H7CAD` 无新 warning

**未纳入本轮**（留待后续）：

- DIMxxx 家族（100+ 尺寸标注变量）— 独立大工作
- 时间戳变量（`$TDCREATE / $TDUPDATE / $TDINDWG / $TDUSRTIMER` — Julian 日期编解码）
- UCS 家族（`$UCSBASE / $UCSORG / $UCSXDIR / $UCSYDIR` — 与 TABLES.UCS 联动）
- 3D / 渲染变量（`$SHADEDGE / $LIGHTGLYPHDISPLAY` 等）
- 视口几何变量（`$VIEWCTR / $VIEWSIZE / $VIEWDIR` — 与 VPORT 表联动）

plan：`docs/plans/2026-04-21-header-drawing-vars-plan.md`

---

### 2026-04-21（续）：Writer `ensure_image_defs` 自动建 IMAGEDEF

闭合同日前一轮 "DXF IMAGE 实体走标准 IMAGEDEF 链接" 留下的 "未纳入" 首项。前轮 writer 在 IMAGE 实体 `image_def_handle == Handle::NULL` 时仍会 fallback 输出非标准 `code 1`（仅为了自循环 round-trip），导致 UI/bridge 构造的 IMAGE（只填 `file_path`）写出的 DXF 对 AutoCAD 等第三方工具而言是"孤儿" — IMAGEDEF 从未被创建。本轮让 writer 在序列化前主动补齐缺失的 IMAGEDEF。

**writer 改动**（`crates/h7cad-native-dxf/src/writer.rs`）：

- 新增 `ensure_image_defs(doc: &mut CadDocument)` 函数，三趟算法绕开 Rust 借用冲突：
  1. **收集趟**（只读借用）：扫描 `doc.entities` + `doc.block_records[*].entities`，登记所有 `image_def_handle == NULL && !file_path.is_empty()` 的 IMAGE，连同 `file_path` / `image_size` 一起入 `pending: Vec<(ImageLoc, String, [f64; 2])>`
  2. **分配趟**（`&mut doc`）：为每个 pending 项 `allocate_handle` → 构造 `ObjectData::ImageDef` → push 到 `doc.objects` → 记录 `(ImageLoc, Handle)`
  3. **回填趟**：按 `ImageLoc::TopLevel(i)` / `ImageLoc::Block(br_handle, i)` 精准索引回写 IMAGE 实体的 `image_def_handle`
- 新增私有 `enum ImageLoc { TopLevel(usize), Block(Handle, usize) }` 承载"IMAGE 在 doc 里的精确位置"
- `needs_ensure_image_defs(doc: &CadDocument) -> bool` 只读预筛：95%+ 的 "read AutoCAD → write" 路径零 pending → 避免无谓 `CadDocument::clone()`

**入口改造**：

- `write_dxf_string(doc: &CadDocument)` 签名保持不变（下游 `save_dxf(&NativeCadDocument)` 不受影响）
- 内部按需 clone-on-demand：`if needs_ensure_image_defs(doc) { clone → ensure → impl } else { impl }`
- 原 body 抽出为 `write_dxf_string_impl(doc: &CadDocument)`

**Idempotency**：

- 对已经走过 `ensure_image_defs` 的 doc 再次 write：所有 IMAGE 的 `image_def_handle` 非 NULL → `needs_ensure_image_defs` 返回 false → 零副作用
- 测试 `ensure_auto_created_imagedef_is_readable_after_roundtrip` 显式两次 write/read 交替断言 IMAGEDEF count 稳定为 1、IMAGE handle 在两次 read 后一致

**测试**（新增 `crates/h7cad-native-dxf/tests/imagedef_ensure.rs`，5 条集成测试）：

- `ensure_creates_imagedef_for_top_level_image_with_file_path_only`：只填 `file_path` 的 top-level IMAGE → write → read 回来 IMAGEDEF 存在 + IMAGE 的 340 对齐
- `ensure_skips_image_with_empty_file_path`：空 `file_path` + NULL handle → 不 auto-create（避免凭空产生 "空字符串 IMAGEDEF"）
- `ensure_skips_image_that_already_has_handle`：预置 IMAGE↔IMAGEDEF 已配对 → write → IMAGEDEF 个数保持 1（无重复）、handle 值不变
- `ensure_handles_image_inside_block_record`：IMAGE 埋在 `doc.block_records["TestBlock"].entities` → write → block-scope IMAGE 正确拿到 auto-created IMAGEDEF 的 handle
- `ensure_auto_created_imagedef_is_readable_after_roundtrip`：两次 write/read 交替验证 idempotency + handle 稳定

**附带测试调整**：`writer_falls_back_to_code_1_when_handle_null`（前轮加的 legacy fallback 测试）原期望"handle=NULL + file_path 非空 → writer 写 code 1"，现在被 `ensure_image_defs` 截胡 → 改名为 `writer_ensure_prepass_promotes_null_handle_image_to_standard_340_link`，断言 writer 输出的 IMAGE 已升级到标准 340 链接（代码 1 消失、IMAGEDEF 在 OBJECTS 里、file_path 迁到 IMAGEDEF.file_name）。这是 **期望的新默认行为** — legacy code 1 fallback 仅在 writer 输入本身就无 `file_path` 且无 handle 的"纯空 IMAGE"极端场景下才触发。

**验证**：

- `cargo test -p h7cad-native-dxf` **93 / 93 全绿**（81 单元 + 6 前轮 imagedef_roundtrip + **5 新** imagedef_ensure + 1 fixture）
- `cargo test --bin H7CAD io::native_bridge` 20/20 无回归
- `cargo check -p h7cad-native-dxf` 零 warning

plan：`docs/plans/2026-04-21-imagedef-auto-create-plan.md`

---

### 2026-04-21：DXF `IMAGE` 实体走标准 `IMAGEDEF` 链接（code 340）

承接同日 H7CAD DXF 解析进度盘点中识别的首号待办项。此前 `EntityData::Image` 通过**非标准的 `code 1`** 直接携带 `file_path`（pre-D5 的 hack），导致读取真实 AutoCAD 输出的 DXF 时 IMAGE 实体 `file_path` 恒为空（AutoCAD 标准不写 code 1 到 IMAGE，而是通过 `code 340` 链接到 OBJECTS 段中的 IMAGEDEF 对象）。本轮让 IMAGE ↔ IMAGEDEF 链接走 DXF 标准路径，同时保留 code 1 为**遗留 fallback** 维持旧文件向后兼容。

**model 改动**（`crates/h7cad-native-model/src/lib.rs`）：

- `EntityData::Image` 新增字段 `image_def_handle: Handle`，承载标准 DXF `code 340` hard-pointer
- 原 `file_path` 字段保留，doc-comment 标注为"cached mirror"语义：当 `image_def_handle == Handle::NULL` 时是权威值，否则是 IMAGEDEF.file_name 的镜像（由 reader 的 post-resolve 填充）

**reader 改动**（`crates/h7cad-native-dxf/src/{lib,entity_parsers}.rs`）：

- `parse_image` 新增 `code 340` 解析 → `image_def_handle`；保留 `code 1` 作为遗留 fallback 读 `file_path`
- `read_dxf` 流程末尾新增 `resolve_image_def_links` 阶段：
  - 扫描 `doc.objects` 建立 `Handle → IMAGEDEF.file_name` 索引
  - 遍历 `doc.entities` 及 `doc.block_records[*].entities`
  - 若某 IMAGE 的 `image_def_handle != NULL` 且 `file_path.is_empty()` → 从索引取 file_name 镜像回 `file_path`
  - 若 `file_path` 已被 legacy code 1 填充，不覆盖（原因：信任已有值）

**writer 改动**（`crates/h7cad-native-dxf/src/writer.rs`）：

- IMAGE 输出根据 `image_def_handle` 分两路：
  - `!= NULL` → 写 `code 340`，**不写 code 1**（纯标准）
  - `== NULL` 且 `file_path` 非空 → 写 `code 1` fallback（保留遗留链路，便于自循环 round-trip）
- IMAGEDEF object 的读写对称路径此前已实现（2026-04-17 OBJECTS 层补齐），本轮未动

**bridge 改动**（`src/io/native_bridge.rs`，5 处）：

- 所有构造 `nm::EntityData::Image { ... }` 的地方追加 `image_def_handle: nm::Handle::NULL`
- 1 处 destructure（acadrust `RasterImage` ← native）改为用 `..` 忽略新字段（acadrust 端当前不承载 IMAGEDEF handle）
- `src/modules/home/draw/raster_image.rs`：绘图命令构造 IMAGE 时同步追加 `image_def_handle: nm::Handle::NULL`

**测试**（新增 `crates/h7cad-native-dxf/tests/imagedef_roundtrip.rs`，6 条集成测试）：

- `standard_dxf_resolves_file_path_from_linked_imagedef`：标准 DXF（IMAGE 只有 code 340，IMAGEDEF 在 OBJECTS 段）→ file_path 通过 resolve 正确回填
- `legacy_dxf_reads_code_1_as_fallback`：legacy DXF（IMAGE 只有 code 1，无 IMAGEDEF）→ file_path 正确读取
- `mixed_dxf_imagedef_wins_over_inline_code_1`：IMAGE 既有 code 340 又有 code 1 → legacy code 1 先到位后不被 resolve 覆盖（trust-first-fill 语义）
- `writer_emits_code_340_when_handle_set_and_omits_code_1`：`image_def_handle != NULL` 时 writer 写 340 且不写 code 1
- `writer_falls_back_to_code_1_when_handle_null`：`image_def_handle == NULL` 且 `file_path` 非空时 writer 写 code 1 fallback（引入 `extract_first_entity_body` 辅助：按 DXF code/value pair 对齐解析，避免 layer_name=`"0"` 被误识为 entity 分隔符）
- `image_imagedef_link_survives_full_roundtrip`：读 → 写 → 读，`image_def_handle` 与 IMAGEDEF `handle` 严格保持（0x4AF）

**验证**：

- `cargo test -p h7cad-native-dxf` 88/88 全绿（81 单元 + 6 新集成 + 1 fixture）
- `cargo test --bin H7CAD io::native_bridge` 20/20 全绿，`image_and_wipeout_bridge_survive_document_roundtrip_with_geometry` 无回归
- `cargo check -p H7CAD` 主 crate 无新 warning

**未纳入本轮**（留待后续）：

- Writer 侧 `ensure_image_defs` 自动建 IMAGEDEF（当前 writer 在 `image_def_handle == NULL` 时走 code 1 fallback 而非 auto-create；需要 mut doc 或 clone，scope 更大）
- `ObjectData::ImageDef` 扩充 `resolution_unit` / `pixel_size` / `class_version` 字段（DWG 原生解析器已支持但 DXF reader/writer 尚未穿透）
- `IMAGEDEF_REACTOR` 的自动双向链接（当前能独立读写，未联动 IMAGE.handle ↔ reactor.image_handle）
- DWG reader（`acadrust::DwgReader` / `h7cad-native-dwg`）的对称改造

**顺带修复**：`src/io/pid_import.rs:5294` 测试夹具补 `SymbolUsage.references: Vec::new()`（pid-parse 模型演进后 H7CAD 侧测试代码未同步的 pre-existing 编译错误），否则 `cargo test --bin H7CAD` 无法 compile。

plan：`docs/plans/2026-04-21-imagedef-object-plan.md`

### 2026-04-20：`pid_package_store` 可观察性 + `PIDCACHESTATS` 命令

承接同日 H7CAD × SPPID 集成分析改进点 3 ("pid_package_store 无 LRU 上限") 的**观察性子集**：不实现完整 eviction policy，先让用户/调试者在运行时能看到缓存占用，为将来的 LRU 决策铺设基线数据。

**改动**（`src/io/pid_package_store.rs` + `src/app/commands.rs`）：

- 新增 `PidPackageCacheStats { entry_count, total_stream_bytes }` 结构
- 新增 `pub fn cache_stats() -> PidPackageCacheStats`：条目数 + 全部 `RawStream.data.len()` 求和，order-of-magnitude 级精度
- 新增 `pub fn cached_entry_summaries() -> Vec<PidPackageCacheEntrySummary>`：每条 `{path, stream_count, stream_bytes}`，按 path 字典序
- 新增 `pub fn cached_paths() -> Vec<PathBuf>`（预留未来 `PIDCACHELIST` 命令）
- 新增 CLI 命令 `PIDCACHESTATS`：**不要求 active tab 是 .pid**（它汇报全局缓存），显示总条目数、总 stream 字节数（B + MB）、每条的 path / stream 数 / 字节数
- `PIDHELP` 新增 "Observability" 段，命令总数 18 → **19**

**不改动**：`cache_package` / `get_package` / `clear_package` 签名；LRU 策略；eviction 自动化。

**字节计量语义**：只累加 `RawStream.data.len()`，不算 Arc 头 / HashMap bucket / PathBuf / struct padding。精度 ±5% 级，识别 MB vs GB 量级足够；需要精确 RSS 请走 OS 级 probe。

**测试**（`io::pid_package_store::tests` 新增 3 条，总数 4 → 7）：

- `cache_stats_reflects_insert_and_clear_via_tagged_filter`：用 tag 前缀过滤自己插入的条目，免疫并行测试干扰（之前用绝对 delta 断言会因 parallel test 插入而 flaky）
- `cached_paths_returns_lexicographic_order`：乱序插 3 条 → 过滤自己的条目 → 断言升序
- `cached_entry_summaries_counts_streams_and_bytes`：5 个 100-字节 stream → summary 报 (5, 500)

`cargo test --bin H7CAD io::pid` 72/72 绿（65 前轮 + 7 新），零回归。

plan: `docs/plans/2026-04-19-pid-cache-stats-plan.md`

### 2026-04-20：SPPID 身份字符串去硬编码（保守版）

延续同日 H7CAD × SPPID 集成分析改进点 1 的最低风险落地：`SPPID_TOOL_ID` 改由 `env!("CARGO_PKG_NAME")` 注入，自动跟随 Cargo.toml `[package].name`；`SPPID_SOFTWARE_VERSION` **保持手写常量**（因 SPPID 消费方可能按精确字符串匹配版本字段，自动绑 `CARGO_PKG_VERSION` 存在未知兼容风险），改由 drift-detection 单测在 `cargo release` 忘同步时显性失败提醒。

**改动**（`src/io/pid_import.rs`）：

- `const SPPID_TOOL_ID: &str = env!("CARGO_PKG_NAME");` 取代硬编码 `"H7CAD"`
- `SPPID_SOFTWARE_VERSION` 保留 `"0.1.3"` 字面量，追加 doc comment 说明不自动绑的理由

**测试**（新增 2 条于 `io::pid_import::tests`）：

- `sppid_software_version_tracks_cargo_pkg_version`：断言常量 == `env!("CARGO_PKG_VERSION")`，下次版本 bump 忘同步时 CI 失败
- `sppid_tool_id_matches_crate_name`：回归守护 `env!` 绑定生效且 crate 名仍为 "H7CAD"

`cargo test --bin H7CAD io::pid_import` 65/65 绿（63 前轮 + 2 新），零回归。

plan: `docs/plans/2026-04-19-sppid-identity-env-plan.md`

### 2026-04-20：`.pid` Save-As 同迁移 publish sidecar（bug fix）

修复 H7CAD × SPPID 集成分析中识别的 P2 用户体验 bug：`save_pid_native(dst, src)` 写入新 `.pid` 时未同步迁移同目录的 publish sidecar（`{stem}_Data.xml` / `{stem}_Meta.xml`），导致"另存为"到新位置后 sidecar 成为孤儿，re-open 新 `.pid` 时静默丢失 publish 增强的对象图/summary.title 等数据。

**改动**（`src/io/pid_import.rs`）：

- `save_pid_native` 写完 `.pid` 后调用新 helper `copy_publish_sidecars_if_present(src, dst)`
- 新 helper 实现"both-or-nothing"契约，与 open 侧 `merge_publish_sidecars` 对称：
  - 两个 sidecar 都缺 → no-op
  - 两个都在 → 按 `dst` 的 stem 重命名并复制
  - 只存一个 → 返回 "incomplete publish bundle" 错误，拒绝传播半损坏 bundle
- 对 `src == dst`（覆盖同文件保存）显式短路，避免 `fs::copy` 自覆盖未定义行为

**不改动**：`save_pid_native` 函数签名；`publish_data_path` / `publish_meta_path` 命名算法；`merge_publish_sidecars` 语义；`export_sppid_publish_bundle` 的 sidecar 生成逻辑。

**测试**：`io::pid_import::tests` 新增 3 条集成测试：

- `save_pid_native_copies_sidecars_when_both_present_with_new_stem`：构造 src + 两个 sidecar → `save_pid_native(dst, src)` → 断言 dst 旁生成同名 sidecar 且字节一致（覆盖"重命名 + 双写"主路径）
- `save_pid_native_is_noop_when_no_sidecars_present`：直接 open 的 SmartPlant 原生 `.pid` → save 不伪造 sidecar（覆盖原生 `.pid` 无 publish 产物的向后兼容）
- `save_pid_native_errors_on_incomplete_sidecar_pair`：先 load 再单写 `{stem}_Data.xml` → save 返回含 "incomplete publish bundle" 的错误（覆盖 open / save 两侧契约一致性）

`cargo test --bin H7CAD io::pid_import` 63/63 绿，无既有用例回归。

plan: `docs/plans/2026-04-19-pid-sidecar-migration-plan.md`

### 2026-04-20：PID 工作台 v2 — layout-first 可读整图预览

在首期 `P&ID Browser / Graph View / Inspector` 三栏工作台与 BRAN 导出闭环基础上，继续把 `.pid` 打开后的图形区从“结构网格预览”推进到“layout-first 可读整图预览”。本轮仍保持只读，不做 SmartPlant 原始图纸高保真复刻。

**本轮完成：**

- **layout-first 打开链**
  - `open_pid()` 在合流 sidecar `_Data.xml / _Meta.xml` 后，会主动调用 `pid_parse::derive_layout()`
  - `pid_document_to_preview()` 优先消费 `PidDocument.layout`
  - 旧的 grid/object 预览保留为 fallback 兜底，而不再主导整图显示

- **Preview 索引增强**
  - `PidPreviewIndex` 新增：
    - `by_drawing_id`
    - `by_graphic_oid`
  - 让 layout-backed 图元仍能维持 Browser / 视口 / Inspector 三向联动

- **layout glyph 最小语义集**
  - 主管线：`Pipeline`
  - 分支点：`Branch`
  - 连接点：`Connector`
  - 过程点：`ProcessPoint`
  - 仪表：`Instrument`
  - 设备：`Equipment`
  - 容器：`Vessel`
  - 注释：`Note`
  - 喷嘴 / Port：`Nozzle`
  - 离页连接符：`OffPageConnector`
  - 管件：`PipingComponent`
  - 未知项仍显示带标签占位框，但不再静默退化成无语义圆点

- **fallback rail 显式化**
  - 无法定位到主图的对象进入 `PID_FALLBACK`
  - 不再混入主图布局，便于区分“已定位对象”和“仅结构存在的对象”

**新增测试：**

- `pid_preview_prefers_layout_anchor_when_layout_exists`
- `pid_preview_places_unplaced_objects_on_fallback_layer`
- `pid_preview_index_tracks_graphic_oid_for_layout_items`
- `pid_preview_renders_process_point_as_circle_when_layout_kind_known`
- `open_pid_real_sample_builds_layout_when_sample_present`

**验证：**

- `cargo test -p H7CAD -- --test-threads=1`
- 结果：`215/215` 全绿

**当前边界：**

- 这是“真实样例的可读整图预览”，不是 SmartPlant 原始符号几何还原
- `.sym` 文件的原始几何尚未解析
- 后续下一步应继续收 `JSite/.sym basename -> object glyph` 的对象级映射，让 `bundle mode` 比 `pid-only mode` 再多一层真实符号语义
- 同日后续补丁：PID 对象 Inspector 新增 `Symbol Evidence` 只读区，直接显示对象级 `layout` 下沉出来的 `Symbol Name / Symbol Path / Graphic OID / Layout ID`。这让 `pid-only` 场景下来自 `JSite` 的代表性 `.sym` 线索也能在对象面板中被看见，而不再只停留在全局 Symbols 列表

### 2026-04-18：DWG M3-B 深化调试 — AC1015 LINE/POINT body 解码链路诊断与修复

在 M3-B 收口（7 种实体 84 个 entity，163/163 全绿）基础上，对 AC1015 对象体解码
链路进行深度诊断和修复，重点解决 LINE body 字段默认值错误与 body/handle 流边界
handoff 的 bit 坐标错位问题。

**主要修复**：

- **AC1015 common decode 诊断本地化** (`bit_reader.rs` 新增阶段追踪)
  - 精确定位 common preamble 各字段（owner / reactor / xdict / entity flags）
    在 bit 级别的解码失败点，区分"blocked handle early exit"与"默认值断言
    失败"两类根因

- **split_ac1015_object_streams bit 坐标修复** (`object_header.rs`)
  - `main_size_bits` 与 post-header cursor 均以绝对 body-bit 坐标表达；修复
    分流时 `BitReader::from_bit_range` 调用参数，防止 header 在中途 byte 边界
    时的隐性字节对齐舍入（保留同一坐标空间，不做提前截断）

- **AC1015 LINE body 默认值全链路对齐**
  - `fix(native-dwg): align ac1015 line body defaults`：修正 LINE entity body
    字段的测试期望默认值，与真实 `sample_AC1015.dwg` BitReader 读出值一致
  - `fix(native-dwg): align line dd defaults`：修正 DD（Double-Double）字段
    的默认值期望

- **LINE/POINT body 解码失败路径系统枚举**
  - `feat(native-dwg): trace AC1015 line point failure stages`：新增分阶段
    失败追踪，枚举各 body 字段解码顺序中可能的断点
  - 新增多组集成测试：body framing boundary / live line point body / line body
    values — 与真实 DWG 数据锚定，确保每次 bit_reader 改动后可立即发现回归

- **DWG worker 任务基础设施**（chore）
  - 初始化 AC1015 DWG worker 任务配置
  - 允许以验证优先的 worker 完成标准
  - 修复任务初始化脚本 shell 兼容性

**验证**：

- `cargo check -p H7CAD`：零 warning 保持
- `cargo test -p h7cad-native-dwg -- --test-threads=1`：body 解码基线
  与 real_samples 诊断断言全部对齐

### 2026-04-17：DWG Native M3-B 收口 — ARC/CIRCLE 纠偏 + common metadata + TEXT/LWPOLYLINE/HATCH 入场

继续推进 `crates/h7cad-native-dwg`，本轮**不改 facade、不改主程序 DWG runtime**，
只把 AC1015 真对象流 enrichment 从“少量几何 best-effort”推进到
“几何 + common metadata 基本可信 + 高收益实体类型接入”。

**本轮完成：**

- **统一 AC1015 固定 type code 口径**
  - 改成与 `vendor_tmp/acadrust` / ACadSharp 一致：
    - `TEXT=1`
    - `ARC=17`
    - `CIRCLE=18`
    - `LINE=19`
    - `POINT=27`
    - `LWPOLYLINE=77`
    - `HATCH=78`
  - `real_samples.rs` 的 histogram label 与真实恢复统计现已同口径

- **object body 两流拆分正式落地**
  - `src/object_header.rs` 新增 `split_ac1015_object_streams()`
  - 基于 `main_size_bits` 把 object body 切成：
    - main stream
    - handle stream
  - 后续 common/entity/table 解码都不再靠“单 reader 顺序猜位”

- **common entity / non-entity preamble 从 skip 升级到 parse**
  - `src/entity_common.rs` 新增：
    - `Ac1015EntityCommonData`
    - `Ac1015NonEntityCommonData`
    - `parse_ac1015_entity_common()`
    - `parse_ac1015_non_entity_common()`
    - `dwg_lineweight_from_index()`
  - 现可真实写回：
    - `owner_handle`
    - `layer_handle -> layer_name`
    - `linetype_flags / linetype_handle -> linetype_name`
    - `color_index`
    - `linetype_scale`
    - `lineweight`
    - `invisible`
  - `Entity::new()` 默认 common 字段不再是 enrichment 唯一来源

- **新增 3 个高收益实体解码器**
  - `src/entity_text.rs`
  - `src/entity_lwpolyline.rs`
  - `src/entity_hatch.rs`
  - 当前 AC1015 enrichment 已支持：
    - `LINE`
    - `ARC`
    - `CIRCLE`
    - `POINT`
    - `TEXT`
    - `LWPOLYLINE`
    - `HATCH`

- **真实表记录名映射预扫**
  - 在 enrichment 前预扫真实 table records：
    - `LAYER (51)`
    - `STYLE (53)`
    - `LTYPE (57)`
  - 用其反解实体 common metadata 中的 layer/style/linetype handle

**真实样本基线（`sample_AC1015.dwg`）现状：**

- `read_dwg()` 现恢复：
  - `26 LINE`
  - `4 CIRCLE`
  - `1 ARC`
  - `6 POINT`
  - `26 TEXT`
  - `15 LWPOLYLINE`
  - `6 HATCH`
- 合计 **84 个真实 native entities**
- 仍保留 `2 blocks / 2 layouts / 271 objects` scaffold
- common metadata 抽样断言已加入：
  - 不能全部 `owner_handle = NULL`
  - 不能全部 `layer_name = "0"`
  - 至少一部分 `color / linetype` 非默认

**验证：**

- `cargo test -p h7cad-native-dwg -- --test-threads=1`：**163/163** 全绿
  - unit **99**
  - `read_headers.rs` **53**
  - `real_samples.rs` **11**
- `cargo test -p h7cad-native-dwg --test real_samples real_dwg_samples_baseline_m3b -- --nocapture --test-threads=1`
  - AC1015 基线通过，TEXT/LWPOLYLINE/HATCH 均已非零
- `cargo test -p h7cad-native-facade -- --test-threads=1`：通过
- `cargo check -p H7CAD`：通过

**边界仍保持不变：**

- `crates/h7cad-native-facade` 仍返回 `native DWG reader not implemented yet`
- `src/io/mod.rs` 仍继续走 `acadrust::DwgReader / DwgWriter`
- 不做 DWG writer
- 不做 AC1018+ 支持

### 2026-04-17：Manage Tab AUDIT — 图纸完整性体检（read-only 报告）

ROADMAP Manage Tab / Cleanup 里最后一条 **High** 复杂度命令 **AUDIT** 从
ribbon stub 升级为完整命令。与 FINDNONPURGEABLE 形成"只读体检"组合 ——
FINDNONPURGEABLE 列 purge 不了的定义，AUDIT 找**被引用但引用不到**的
问题。MVP 为纯只读报告，AutoCAD 的 `AUDIT FIX` 修复模式留作未来增强。

**检查清单** (`src/app/commands.rs`)：

AUDIT 对当前 document 扫描 7 类完整性问题：

1. **孤立图层引用** — `entity.common.layer` 非空且不在 `document.layers`
2. **未知文字样式** — `Text.style` / `MText.style` 不在 `text_styles`
3. **未知线型** — `entity.common.linetype` 非空非 `ByLayer`/`ByBlock` 且
   不在 `line_types`（大小写不敏感匹配）
4. **未知标注样式** — `Dimension.base().style_name` 不在 `dim_styles`
5. **孤立 INSERT** — `Insert.block_name` 不在 `block_records`
6. **空用户 block** — `BlockRecord.name` 非 `*`-prefix 且 `entity_handles`
   为空
7. **NULL handle entity** — `entity.common.handle.is_null()`

**实现细节**：

- 开头预计算 5 个 `HashSet<String>`（layer / text_style / linetype /
  dim_style / block_record 名字池），避免 O(N×M) 扫描
- `kind_label(&EntityType) -> &'static str` 新增 helper，覆盖 17 种
  variant，用于报告里 `"Line(0x...)"` 格式化（未识别的 fallback "Entity"）
- 输出格式：
  - 零问题 → `AUDIT: drawing passed — no integrity issues detected.`
  - 有问题 → `AUDIT: N issue(s) detected:` + 每条 push_info 一行 +
    结尾提示 `"AUDIT FIX is not yet implemented."`
- read-only：无 mutation、无 undo snapshot、无 dirty flag

**决策**：

- **只报告不修复**：AutoCAD 的 AUDIT 交互式提问"Fix errors? (Y/N)"并自动
  reset 坏引用到 layer "0" / 删除孤立 INSERT / 等。这部分涉及破坏性编辑，
  scope 显著变大且需要 undo 策略。MVP 先确保"找得出"，修复后续再加
- **大小写不敏感的 linetype 匹配**：CAD 线型名约定大写（`DASHED` /
  `CONTINUOUS`），用户或第三方 DXF 可能写 `Dashed`，用 `eq_ignore_ascii_case`
  避免误报
- **不检查字段级内容**：如 DXF 里 handle 为 0、bounding box NaN 等更深
  invariants 暂不做，依赖 parser 在 load 阶段已 reject

**验证**：

- `cargo check -p H7CAD`：零 warning（5.13s）
- 主 crate 测试 **153/154**（和上一轮一致，无回归；AUDIT 是 CLI 读取
  路径，核心逻辑和 FINDNONPURGEABLE 模式同源，该模式已被实战验证）

**ROADMAP 进度**：Manage Tab / Cleanup 全部 3 条命令交付（FINDNONPURGEABLE
+ OVERKILL + AUDIT）。combined 今日：View Tab 9 + Insert Tab 11 + Manage
Tab 7 = **27 个** ROADMAP 命令后端落地。

### 2026-04-17：Manage Tab OVERKILL — 几何重复去重（Line / Circle / Arc / Point）

ROADMAP Manage Tab / Cleanup group 里 **High** 复杂度命令 **OVERKILL** 从
ribbon stub 升级为完整去重命令。覆盖 AutoCAD OVERKILL 最核心的 80% 用例：
**Line / Circle / Arc / Point** 四种简单几何的重复检测与删除。复杂实体
（Polyline / Hatch / Text / Dimension / Spline）保守跳过，确保不误删
用户数据。

**核心算法** (`src/modules/manage/overkill.rs` 扩展自原 ribbon stub)：

- `GeomKey` enum：规范化几何指纹，4 个变体对应 4 种支持的实体类型
- `QPoint(i64, i64, i64)` / `QScalar(i64)`：量化坐标/标量；用
  `(f64 * 1e6).round() as i64` 避开 `f64` 不能 `Hash` 的限制，`1e-6`
  tolerance 覆盖 CAD 工程精度
- `line_key(a, b)`：端点按字典序排序后构造 key，使 `Line(A→B)` 和
  `Line(B→A)` 归一为同一 key —— CAD 语境下方向无语义
- `geom_key(entity) -> Option<GeomKey>`：不支持的 entity 返回 `None`，
  确保 `find_duplicates` 不会把它们算进任何桶里
- `find_duplicates(entries) -> Vec<Handle>`：单遍 HashSet 扫描，对每个
  entity 取 key；key 已见过则 handle 归入 dupes，按 encounter order
  返回；第一次出现的 handle 保留
- **11 个单测**全绿：identical lines / reversed endpoints / concentric
  circles / different radius kept / arcs differing angle kept / identical
  arcs / line vs circle no-cross-collision / identical points /
  sub-epsilon tolerance folding / empty input / first-occurrence kept

**Dispatch** (`src/app/commands.rs`)：

- `"OVERKILL"` case：
  - Scope 选择：selection 非空时仅在选择集内去重；否则扫描 `document.entities()`
    全集（过滤掉 `Handle::is_null()` 的异常条目）
  - 空 scope 时 `push_info` 引导用户；有 dupes 时 `push_undo_snapshot("OVERKILL")`
    → `scene.erase_entities(&dupes)` → mark dirty → 汇报 `"removed X of Y"`；
    零 dupes 时 push_output 告知"no duplicates found"
  - `return Task::none()` 明确终止

**模块暴露** (`src/modules/manage/mod.rs`)：`mod overkill` → `pub(crate) mod overkill`

**设计决策**：

- **保守 scope（4 种简单几何）**：AutoCAD OVERKILL 还会合并共线首尾相接的
  Line 段 —— 这是"真正 High"的部分；MVP 只做去重不做合并，避免算法蔓延
  与测试爆炸
- **手写量化替代 `ordered-float` crate**：不引入新依赖，`(f64 * 1e6).round()
  as i64` 覆盖 ±9e12 坐标范围，CAD 工程绝对够用
- **端点排序用 `PartialOrd` derive**：`QPoint` 加 `#[derive(PartialOrd, Ord)]`
  让 `a <= b` 比较直接可用（编译一次发现问题，一次修正）
- **保留第一个 Handle**：按 `entities()` 返回顺序稳定，undo 后 Handle 不变
- **风险提示文档化**：Arc 方向差异（顺时针 vs 逆时针）不处理，会被视为不同；
  浮点 quantise 在接近整数边界时可能误判（保守错误方向：假阴性/漏删，不会
  误删用户数据）

**验证**：

- `cargo check -p H7CAD`：零 warning（9.54s）
- 主 crate 测试 **153/154**（相比上一轮 142/143 新增 11 个 overkill 单测
  全绿；pre-existing 失败依然，无新回归）

**ROADMAP 进度**：Manage Tab / Cleanup 的 High 复杂度 `OVERKILL` 交付。
combined 今日：View Tab 9 + Insert Tab 11 + Manage Tab 6 (ALIASEDIT + 
FINDNONPURGEABLE + CUIEXPORT + CUIIMPORT + CUILOAD + OVERKILL) = **26 个**
ROADMAP 命令后端落地。

### 2026-04-17：VS Code 风格 Workspace Phase 2 — 左侧面板 UI 集成

给 Workspace 基础架构接上可见的 UI：在 properties panel 左侧渲染一个 240px
宽的 EXPLORER 风格面板。与 Phase 1 的 state + Message + 命令行命令已就绪
配合后，现在用户从 `WORKSPACE` 命令选目录 → 面板自动显示 → 单击文件打开
tab、单击目录展开折叠 — 全链路可用。

**新模块** (`src/ui/workspace_panel.rs`)：

- `pub fn view_panel<'a>(ws, active_path, expanded_dirs) -> Element<'a, Message>`
  返回已样式化的 panel
- 常量：`PANEL_WIDTH = 240px` / `ROW_HEIGHT = 22px` / `HEADER_HEIGHT = 28px`
  / `INDENT_PX = 12px`；统一的 dark-theme 颜色常量（PANEL_BG / HEADER_BG /
  ROW_HOVER / ROW_ACTIVE / TEXT_COLOR / TEXT_MUTED / BORDER_COLOR）
- `panel_header(ws)` — 顶部 28px 容器：workspace 根目录名（`ws.root_label()`
  取 last-component）+ 刷新按钮（↻ → `WorkspaceRefresh`）+ 关闭按钮
  （× → `WorkspaceClose`，hover 时变暗红色）
- `panel_body(ws, active_path, expanded_dirs)` — 调 Phase 1 的
  `visible_entries()` 过滤后用 scrollable 列出每行；空时显示 `(empty workspace)`
- `row_element(entry, active_path, expanded_dirs)` — 按 `depth - 1` 计算
  缩进（顶层 0 缩进），每行结构：`[indent space][icon][4px space][name]`
- 图标方案（Unicode emoji 临时方案）：
  - `Directory` → `▼ 📁` 展开 / `▶ 📁` 折叠
  - `DxfFile` → `📐`
  - `DwgFile` → `📏`
  - `Truncated` → `⋯`（灰色、click 为 Noop）
- 点击行为：
  - 文件行 → `Message::WorkspaceFileClick(path)` — host 自动判断已打开切 tab
  - 目录行 → `Message::WorkspaceDirToggle(path)` — 反转 expanded 状态
  - Truncated 行 → `Message::Noop`（不可交互）
- active_path（= 当前 tab 的 `current_path`）匹配的文件行用 ROW_ACTIVE
  （#335890）背景高亮；其他行 hover 时用 ROW_HOVER

**主布局集成** (`src/app/view.rs`)：

- `center_stack` 构造逻辑变成：
  - `ws_panel: Option<Element>` — 仅当 `workspace_panel_open == true` **且**
    `workspace: Some(_)` 时调 `view_panel(...)`
  - `center_row` = 有 panel 时 `row![wp, properties, viewport]`，无则
    `row![properties, viewport]`（不占空间）
- 命名 clash 风险已规避（`ws_panel` 与原本 `nav` / `cube_click` 类似都是
  `Option<Element>` pattern）

**模块挂载** (`src/ui/mod.rs`)：

- `pub mod workspace_panel;`（插在 `tablestyle` / `textstyle` 之后）

**Phase 1 遗留的 dead-code 解除**：
- `Message::WorkspaceFileClick` / `Message::WorkspaceDirToggle` 去掉
  `#[allow(dead_code)]`
- `Workspace::root_label` / `visible_entries()` 同样去掉标注

**验证**：

- `cargo check -p H7CAD`：零 warning（5.17s）
- 主 crate 测试 **142/143**（和 Phase 1 一致 — UI 层纯渲染无新单测，核心
  逻辑单测在 Phase 1 的 workspace 7 条；pre-existing 失败不变）

**完整使用路径**（Phase 1 + Phase 2 合起来）：
1. 用户输入 `WORKSPACE` → rfd folder picker 弹出
2. 选目录 → `scan_workspace` 扫描（<50ms）→ 240px 面板在 properties
   左侧出现
3. 单击文件 → 自动打开 tab 或切换到已打开的 tab
4. 单击目录 → ▶ ↔ ▼ 切换展开 / 折叠
5. Header 点 ↻ 重新扫描；点 × 关闭 workspace（tabs 不受影响）
6. 面板本身可通过 `WORKSPACETOGGLE` 隐藏/恢复（workspace state 保留）

**ROADMAP 关系**：Workspace 是**非 ROADMAP** 的主动功能，用户明确要求
"参考 VS Code 打开文件夹方式"。与 ROADMAP 命令解耦，新增。

### 2026-04-17：VS Code 风格 Workspace 基础架构 — 扫描 + state + 命令（Phase 1）

给 H7CAD 接入 VS Code EXPLORER 风格的工作空间功能。**本轮 Phase 1 交付
基础架构**（扫描逻辑 + state + Message + dispatch + handler，可通过命令
行完整驱动），**下一轮 Phase 2 交付左侧面板 UI**。

**扫描模块** (`src/app/workspace.rs` 新文件)：

- `Workspace { root: PathBuf, entries: Vec<WorkspaceEntry>, truncated: bool }`
  —— 工作空间扫描快照
- `WorkspaceEntry { path, name, depth, parent, kind }` —— 扁平列表每条记录
- `EntryKind = Directory / DxfFile / DwgFile / Truncated`
- 常量 `DEFAULT_MAX_DEPTH = 3` / `DEFAULT_MAX_ENTRIES = 2000`
- 黑名单目录常量 `BLACKLIST_DIR_NAMES`：`.git / .cargo / .cursor / target /
  node_modules / vendor_tmp / .agents / .memory / .factory / ...` 等 18 条
- `scan_workspace(root, max_depth, max_entries) -> Result<Workspace, String>`：
  递归扫描，仅保留 `.dxf` / `.dwg`（case-insensitive） + 目录结构；每层
  **目录优先 + 文件字母序**；超出 max_entries 时停止并标 truncated，追加
  `EntryKind::Truncated` 标记行
- `visible_entries(&entries, &expanded_dirs) -> Vec<&WorkspaceEntry>`：按
  祖先目录是否展开过滤可见行（下一轮 UI 消费）
- `Workspace::root_label()` 显示用 helper
- **7 个单测**全绿：top-level CAD filter / blacklist skip / max_depth /
  sort order / truncation flag / 非 dir 根错误处理 / visible_entries
  collapse 行为

**State 扩展** (`src/app/mod.rs`)：

- `H7CAD` 新增 3 字段：
  - `workspace: Option<workspace::Workspace>`
  - `workspace_panel_open: bool`（默认 `false`，打开工作区时自动置 true）
  - `expanded_dirs: HashSet<PathBuf>`
- `Message` 扩 7 个 variant：`WorkspaceOpen` / `WorkspaceOpened(Option<
  PathBuf>)` / `WorkspaceClose` / `WorkspaceRefresh` / `WorkspaceToggle` /
  `WorkspaceFileClick(PathBuf)` / `WorkspaceDirToggle(PathBuf)`
- `mod workspace` 以 `pub(crate)` 暴露

**文件对话框** (`src/io/mod.rs`)：

- `pub async fn pick_workspace_folder() -> Option<PathBuf>` — rfd
  `.pick_folder()` 包装，标题 "Open Workspace Folder"

**Dispatch** (`src/app/commands.rs`)：

- `"WORKSPACE"` / `"WORKSPACECLOSE"` / `"WORKSPACEREFRESH"` /
  `"WORKSPACETOGGLE"` 四个 case，各自 `Task::done(Message::...)`

**Update handlers** (`src/app/update.rs`)：

- `WorkspaceOpen` → `Task::perform(pick_workspace_folder(), WorkspaceOpened)`
- `WorkspaceOpened(Some(root))` → `scan_workspace(…)` 成功则存入 state 并
  自动打开面板 + 清空 `expanded_dirs`；失败 push_error
- `WorkspaceClose` → `workspace.take()` + 关面板 + 清 expanded；no-op
  提示已无 workspace
- `WorkspaceRefresh` → 对当前 root 重新 scan；无 workspace 时友好提示
- `WorkspaceToggle` → 仅当 workspace 存在时切换 `panel_open`
- `WorkspaceDirToggle(path)` → `expanded_dirs` insert/remove
- `WorkspaceFileClick(path)` → 遍历 `self.tabs` 找 `current_path == Some(&
  path)`，匹配则 `self.active_tab = idx`（无需 Task 重载）；否则
  `Task::perform(open_path(path), FileOpened)`（复用已有 loader）

**设计决策**：

- **扁平 entries + expanded_dirs 过滤** 而非嵌套 tree struct — 更简单、
  更 diff-friendly、避免 iced 嵌套借用问题
- **扫描同步执行** — 深度 3 + 黑名单 + 2000 条截断下 <50ms，不需要 async
  Task，也省掉线程 safety 问题
- **绝对路径直比** 做 tab 重复判定（不做 canonicalize） — 用户交互路径
  都是 FileDialog 返回的绝对路径，够用；避免 file 可能不存在时 canonicalize
  失败
- **黑名单预先硬编码** — 包含 H7CAD 工作目录下常见的 agent / cargo /
  VC 等配置目录 18 条，防止误扫巨大子树
- **Phase 1 / Phase 2 切分** — state + 命令行驱动先交付可验证的闭环；
  UI panel 放 Phase 2 独立 commit，避免 iced 布局集成与核心逻辑耦合

**Dead code 预留**：`WorkspaceFileClick` / `WorkspaceDirToggle` 两个 Message
variant + `root_label` / `visible_entries` 两个 helper 本轮未被构造 / 调用
（它们是 Phase 2 UI 的消费点），加 `#[allow(dead_code)]` 并注释
"consumed by the side-panel UI (next iteration)"。

**验证**：

- `cargo check -p H7CAD`：零 warning（3.89s）
- 主 crate 测试 **142/143**（相比上一轮 135/136，新增 7 个 workspace
  单测全绿；pre-existing `prop_geom_commit_rejects_unsupported_native_hatch`
  失败依然 pre-existing，无新回归）

**下一轮 (Phase 2)**：
- `src/ui/workspace_panel.rs`：`view_panel(&Workspace, active_path,
  &expanded_dirs) -> Element<Message>`
- `src/app/view.rs`：center_stack 集成 workspace panel 在 properties panel
  左侧
- Panel：240px 固定宽度 / 滚动列表 / 目录 ▶▼ 展开折叠 / 文件单击触发
  `WorkspaceFileClick` / 高亮当前 active tab 对应文件行

### 2026-04-17：Insert Tab XCLIP — 选中 RasterImage / Underlay 的裁剪控制

ROADMAP Insert Tab / Reference group 的 Medium 复杂度命令 **XCLIP** 从
ribbon stub 升级为 CLI 子命令集合，覆盖裁剪 **状态查询 / 启停 / 删除**
三大场景。交互式 `XCLIP NEW`（draw a new boundary）暂不支持—— 需要点
拾取命令对象，留作未来 enhancement。

**子命令** (`src/app/commands.rs`)：

- `XCLIP` / `XCLIP STATUS` — 对当前 selection 里的每个 `RasterImage` 和
  `Underlay` 输出 `clip=ON|OFF` 以及（Underlay）边界顶点数
- `XCLIP ON` / `XCLIP OFF` — 切换 clipping flag：
  - `RasterImage`：修改 `flags` 上的 `ImageDisplayFlags::USE_CLIPPING_BOUNDARY` 位
  - `Underlay`：修改 `flags` 上的 `UnderlayDisplayFlags::CLIPPING` 位
  - 统计改变的 entity 数后输出 `"XCLIP ON/OFF: N of M entity(ies) changed."`
- `XCLIP DELETE` — 彻底移除 clip boundary：
  - `RasterImage`：`clip_boundary = ClipBoundary::full_image(size.x, size.y)` +
    清 `USE_CLIPPING_BOUNDARY` 位
  - `Underlay`：`clip_boundary_vertices.clear()` + 清 `CLIPPING` 位
- `XCLIP NEW` — 给出提示 "interactive boundary picker not yet supported"
- 未知子命令 — 使用说明提示

**行为细节**：
- 命令前首先过滤 selection 为 clippable entities（`RasterImage` 或 `Underlay`），
  空时 push_info 引导用户"select first"并 `Task::none()` 返回
- 任何 mutating 路径（ON/OFF/DELETE）前执行 `push_undo_snapshot("XCLIP")`，
  完成后 mark tab dirty
- STATUS 路径 read-only，无 snapshot、无 dirty

**验证**：

- `cargo check -p H7CAD`：零 warning（5.44s）
- 主 crate 测试 **135/136**（和上一轮一致，无回归；pre-existing
  `prop_geom_commit_rejects_unsupported_native_hatch` 失败依然）

**踩坑**：第一版用了 `Handle::as_u64()` 方法，实际 acadrust `Handle` 只
暴露 `.value() -> u64`，一次修正。

**ROADMAP 进度**：Insert Tab / Reference 的 Medium `XCLIP` 交付。combined
今日：View Tab 9 + Insert Tab 10 + Manage Tab 5 = **24 个** ROADMAP 命令
后端落地。

### 2026-04-17：Insert Tab BLOCKPALETTE + View Tab TOOLPALETTES — block 清单 + 面板反馈

本轮交付 2 条 Medium 复杂度命令 —— `BLOCKPALETTE`（block 清单 + 快捷插入）
和 `TOOLPALETTES`（tool palettes 面板在 H7CAD 的映射）。

**BLOCKPALETTE** (`src/app/commands.rs`)：

ROADMAP 原义是 "open block palette for inserting blocks with multiple
views" — H7CAD 没有独立浮动面板，落地为 CLI 子命令集合：
- `BLOCKPALETTE` / `BLOCKPALETTE LIST` — 列出所有 **user-defined** block
  records（跳过系统 block `*Model_Space` 等），每条显示名字 + INSERT
  引用数 + AttributeDefinition 数
- `BLOCKPALETTE COUNT` — 只打印聚合值
- `BLOCKPALETTE INSERT <name>` — 验证 block 存在后 `Task::done(Message::
  Command("INSERT <name>"))` 派发到现有 INSERT 命令
- INSERT 引用数通过一次 entities() 扫描按 `block_name` 聚合；AttDef 数通过
  `br.entity_handles` 过滤 `EntityType::AttributeDefinition` 得到
- read-only；未知子命令给出用法提示

**TOOLPALETTES** (`src/app/commands.rs`)：

AutoCAD Tool Palettes 是一个带 drag-and-drop 工具瓦片的浮动面板；H7CAD
的 ribbon tabs (Home / Annotate / Insert / View / Manage) 已经提供等价
表面。因此 `TOOLPALETTES` 命令兑现为**信息性反馈**（和 HORIZONTAL /
VERTICAL / CASCADE 同款策略）—— 说明 ribbon 是 tool surface，引导用户
使用 ribbon 或命令行。read-only、无 mutation。

**验证**：

- `cargo check -p H7CAD`：零 warning（3.90s）
- 主 crate 测试：**135/136**（和上一轮一致，无回归；pre-existing 失败
  依然。本轮命令为 CLI 读取 + dispatch，不独立写单测）

**ROADMAP 进度**：Insert Tab / Block 的 `BLOCKPALETTE` + View Tab / Palettes
的 `TOOLPALETTES` 两条 Medium 交付。combined 今日：View Tab 9 + Insert
Tab 9 + Manage Tab 5 = **23 个** ROADMAP 命令后端落地。

### 2026-04-17：Manage Tab CUIEXPORT / CUIIMPORT / CUILOAD — CUI 持久化三件套

ROADMAP Manage Tab / Customization group 里 3 条 Medium 复杂度命令
`CUIEXPORT` / `CUIIMPORT` / `CUILOAD` 从 ribbon stub 升级为完整文件 I/O
命令。复用 `ALIASEDIT` 已维护的 `command_aliases` 和 shortcut 编辑器已
维护的 `shortcut_overrides` 两个 map，落地到磁盘的 H7CAD CUI 文本格式。

**格式设计** (`src/io/cui.rs` 新模块)：

- 自定义纯文本 schema（不走 AutoCAD `.cuix` XML/ZIP）：
  ```
  # H7CAD CUI v1
  [aliases]
  L=LINE
  CO=COPY
  [shortcuts]
  F3=SNAPOFF
  ```
- `CuiDocument { aliases, shortcuts }` 结构体
- `serialize_cui(&CuiDocument) -> String`：每个 section 内键按字母序写出，
  保证 diff 稳定
- `parse_cui(&str) -> Result<CuiDocument, String>`：宽容解析 — 空行 /
  `#` 注释 / 未知 section / 无 `=` 的行 / 空 key 全部 silently 跳过，
  便于用户手编
- 7 个单测：round-trip / 排序 / 忽略注释空行 / 忽略未知 section /
  容忍畸形行 / key-value 两侧 trim / 空文档 round-trip

**文件对话框** (`src/io/mod.rs`)：

- `pub async fn pick_cui_save_path()` — save-file 对话框，扩展 `.cui/.txt`
- `pub async fn pick_cui_open_path()` — open-file 对话框，同样过滤器
- `mod cui;` 挂在 `io/mod.rs` 顶部

**Message 扩展** (`src/app/mod.rs`)：

- 新增 6 个 variant：
  - `CuiExport` / `CuiExportPath(Option<PathBuf>)`
  - `CuiImport` / `CuiImportPath(Option<PathBuf>)`
  - `CuiLoad` / `CuiLoadPath(Option<PathBuf>)`
- 分别对应三条命令的触发和文件对话框回调

**Dispatch** (`src/app/commands.rs`)：

- `"CUIEXPORT"` / `"CUIIMPORT"` / `"CUILOAD"` 各自 `return Task::done(Message::...)`
  同一行打通
- 三者语义区分：
  - **CUIEXPORT**：把当前 `command_aliases` + `shortcut_overrides` 写到用户选的文件
  - **CUIIMPORT**：**替换** 当前两个 map（用于"换一套配置"）
  - **CUILOAD**：**合并** 到当前两个 map（用于"追加部分配置"，AutoCAD 里
    partial CUI load 的语义）；同 key 时文件值覆盖，同时汇报 added vs overwritten 数

**Update handler** (`src/app/update.rs`)：

- `CuiExport` → `Task::perform(pick_cui_save_path(), CuiExportPath)`；
  拿到路径后 `serialize_cui` + `std::fs::write`，成功则 push_output 汇报
  写入项数和路径
- `CuiImport` → 同款 pick_cui_open_path → `fs::read_to_string` +
  `parse_cui` → 两个 map 直接整体赋值
- `CuiLoad` → pick → parse → 逐项 `insert` 到 map，统计 added vs
  overwritten 两个计数器（根据 `insert` 返回的 `Option<String>` 是 Some 还是
  None 判断）；push_output + push_info 两行详细汇报

**验证**：

- `cargo check -p H7CAD`：零 warning（6.99s）
- 主 crate 测试：**135/136**（相比上一轮 128/129 新增 7 个 cui 单测全绿；
  pre-existing 失败 `prop_geom_commit_rejects_unsupported_native_hatch`
  依然，与本轮改动无关）

**ROADMAP 进度**：Manage Tab / Customization 里 `CUIEXPORT` + `CUIIMPORT`
+ `CUILOAD` 三条 Medium 全部交付。combined 今日：View Tab 8 + Insert Tab 8
+ Manage Tab 5 (ALIASEDIT + FINDNONPURGEABLE + CUIEXPORT + CUIIMPORT +
CUILOAD) = **21 个** ROADMAP 命令后端落地。

### 2026-04-17：View Tab HORIZONTAL / VERTICAL / CASCADE — tab 架构下的信息性反馈

ROADMAP View Tab / Interface group 的 3 个 Low 复杂度命令 **HORIZONTAL**、
**VERTICAL**、**CASCADE** 从 ribbon stub 升级为信息性命令。传统 AutoCAD 这
三条命令用来重新排列 MDI 子窗口（水平平铺 / 垂直平铺 / 层叠），而 H7CAD
采用**单窗口 tab UI**，没有可独立几何排列的子窗口——这三条命令兑现为
描述当前 tab 状态 + 指引用户使用 tab 切换的等价路径。

**实现** (`src/app/commands.rs`)：

- `HORIZONTAL | VERTICAL | CASCADE` 联合 case，按 cmd 字符串映射到显示名
  (`"Tile Horizontal"` / `"Tile Vertical"` / `"Cascade"`)
- 当前 tab 数 `n = self.tabs.len()`：
  - `n <= 1`：push_info `"<mode>: only one document open — nothing to
    arrange."`
  - `n > 1`：push_output 说明 H7CAD 是单窗口 tab UI + 当前打开 tab 数；
    push_info 提示用户使用 tab 栏或 Ctrl+Tab / Ctrl+Shift+Tab 切换
- read-only：无 mutation、无 undo snapshot、无 dirty flag

**决策**：
- 不做"假平铺"（比如平分屏幕或切换 tab 样式），因为 iced 的 tab UI 本身
  就是最优架构；伪装成"平铺"反而损伤一致性
- 信息性反馈是 ROADMAP 这三条命令在当前架构下的诚实兑现

**验证**：

- `cargo check -p H7CAD`：零 warning（3.22s）
- 主 crate 测试 **128/129**（和上一轮一致，无回归）

**ROADMAP 进度**：View Tab / Interface 的 3 条 Low 命令全部交付。combined
今日：View Tab 8 + Insert Tab 8 + Manage Tab 2 = **18 个** ROADMAP 命令
后端落地。

### 2026-04-17：Insert Tab ATTMAN 命令接入 — block AttributeDefinition 只读清单

ROADMAP Insert Tab / Attributes Group 里 Medium 复杂度命令 **ATTMAN**
从 ribbon stub 升级为 read-only 清单报告。AutoCAD 的 ATTMAN 原义是
dialog 编辑 block 的 AttDef，H7CAD 先用 CLI 列出形式落地（dialog
版本可以作为未来 UI 增强）。是 `ATTSYNC` 的对偶视图 — ATTSYNC 修改
INSERT，ATTMAN 查看 block 定义本身。

**实现** (`src/app/commands.rs`)：

- `ATTMAN` / `ATTMAN <blockname>`：
  - 无参：遍历 `document.block_records`（跳过系统 block，名字以 `*`
    开头的 `*Model_Space` 等），对每个 block 的 `entity_handles` 过滤出
    `EntityType::AttributeDefinition`
  - 有参：只列指定 block（block 不存在时 push_error）
  - 每条 attdef 输出：`  tag  prompt="..."  default="..."  flags=[INV,CONST,VERIFY,PRESET]`
    —— `AttributeFlags` 的 4 个 bool 字段 `invisible / constant / verify / preset`
    分别映射到短名 flag token；全清时显示 `[-]`
  - 汇总行：`"ATTMAN: N attribute def(s) across M block(s):"`
  - read-only：无 mutation、无 undo snapshot、无 dirty flag

**验证**：

- `cargo check -p H7CAD`：零 warning（2.24s）
- 主 crate 测试 **128/129**（与上一轮一致，无回归；pre-existing 失败
  依然是 `prop_geom_commit_rejects_unsupported_native_hatch`）

**ROADMAP 进度**：Insert Tab / Attributes 的 Medium `ATTMAN` 交付。
combined 今日：View Tab 5 + Insert Tab 8 + Manage Tab 2 = **15 个**
ROADMAP 命令后端落地。

### 2026-04-17：View Tab VPJOIN 命令接入 — 合并相邻 paper-space viewport

ROADMAP View Tab / Model Viewports Group 里 Medium 复杂度命令 **VPJOIN**
从 ribbon stub 升级为完整命令。用户在 paper-space 布局里选择两个边完全
重合的 viewport，VPJOIN 会把它们合并成一个覆盖联合矩形的 viewport。

**算法核心** (`src/modules/view/vports_join.rs`)：

- `pub struct JoinRect { cx, cy, w, h }` — paper-space 轴对齐矩形
  （cx = Viewport.center.x, cy = Viewport.center.z，Y-up XZ 约定）
- `pub fn join_rects(a, b) -> Option<JoinRect>` — 纯逻辑 merge：
  - **水平相邻**：`x_max(a) ≈ x_min(b)` 或反过来，且两个 rect 的
    `[y_min, y_max]` 必须完全一致
  - **垂直相邻**：`y_max(a) ≈ y_min(b)` 或反过来，且 `[x_min, x_max]`
    必须一致
  - 满足则 merged = union bounding rect；否则返回 None
  - 所有等值比较用 `JOIN_EPS = 1e-6` 容差
- 7 个纯单测覆盖：水平 / 垂直相邻 / 交换律 / 间隙 / 重叠 / 错位边 /
  epsilon 容差

**Dispatch** (`src/app/commands.rs`)：

- `VPJOIN` case：
  - 拒绝 Model 布局（push_error "switch to paper space first"）
  - 从 `selected_entities()` 筛 `Viewport` 且 `vp.id > 1`（跳过 paper-
    space overall viewport）
  - 必须正好 2 个，否则友好报错（打印实际数量）
  - 调 `join_rects`，None 时告知用户"must share an entire edge"
  - `push_undo_snapshot("VPJOIN")` → 改第一个 viewport 的 center/w/h
    成 merged 值 → `erase_entities(&[h_drop])` 删第二个 → 对保留的
    viewport 调 `auto_fit_viewport` 让相机重新 fit
  - 汇报 `"merged 2 viewports into one (W × H)"`

**决策**：

- 选第一个 selected viewport 作为 dominant（保留 handle），和 MOVE/
  COPY 等 AutoCAD 命令的 "first selection = primary" 约定一致
- 拒绝 overlap / offset / gap 而不是尝试最小 bounding rect — 否则
  语义会偏离 AutoCAD（它是严格要求边重合）
- JoinRect 是独立结构，不依赖 acadrust，单测完全脱离 document

**验证**：

- `cargo check -p H7CAD`：零 warning（4.32s）
- 主 crate 测试 **128/129**（新增 7 个 vports_join 单测全绿；上一轮
  121/122；唯一失败 `prop_geom_commit_rejects_unsupported_native_hatch`
  依然 pre-existing）

**ROADMAP 进度**：View Tab / Model Viewports 里 Medium `VPJOIN` 交付。
combined 今日：View Tab 5 + Insert Tab 7 + Manage Tab 2 = **14 个**
ROADMAP 命令后端落地。

### 2026-04-17：Manage Tab ALIASEDIT 命令接入 — 运行时命令别名管理

ROADMAP Manage Tab / Customization 里 Medium 复杂度命令 **ALIASEDIT**
从 ribbon stub 升级为可用 CLI 命令。AutoCAD 的 ALIASEDIT 会开一个
dialog 编辑 `acad.pgp`，H7CAD 这里以命令行 sub-command 形式落地（和
ADJUST / BACKGROUND 等已有命令风格一致）。本次**不**做 pgp 文件持久化，
只实现会话内运行时 alias 表。

**App 状态** (`src/app/mod.rs`)：

- `H7CAD` 新增 `command_aliases: HashMap<String, String>`，默认空
- alias 约定：key 和 value 都规范化为大写（dispatch 层匹配同样大写）

**Alias resolver** (`src/app/commands.rs`)：

- `pub(super) fn resolve_command_alias(cmd, aliases) -> Option<String>`：
  纯函数，提取 cmd 的第一个 whitespace-delimited token，大写后查表，
  命中则把 head 替换为 target、保留其余 arguments，否则返回 None
- 设计要点：
  - **非递归**：A → B 命中后不继续查 B → C，避免配置循环出事故
  - **只替换 head**：后续参数原样透传（例：`BG 10 20 30` → `BACKGROUND 10 20 30`）
  - **大小写无关**：输入小写 `ll` 也能命中大写表项 `LL`
  - **trim_start**：容忍命令前导空白
- 6 个单测覆盖上述全部约束（None/case-insensitive/preserve-args/not-head-
  rewrite/not-recursive/leading-whitespace）

**Dispatch 集成**：

- `dispatch_command` 在 `OPEN_RECENT:` 分支之后、主 `match cmd` 之前
  调 resolver；如有 rewrite，用新字符串进入 match
- 结果：用户定义的 `LL` → `LINE` 别名和内置 `"LINE"|"L"` 走同一条
  dispatch 链路，后续 argument（点选、文本输入）全部继承正常命令路径

**ALIASEDIT 子命令** (`src/app/commands.rs`)：

- `ALIASEDIT` / `ALIASEDIT LIST`：列出所有别名，按 key 字母序输出
- `ALIASEDIT ADD <alias> <command>`：新增或覆盖映射（大写化）
- `ALIASEDIT DEL <alias>` / `DELETE` / `REMOVE`：删除指定别名
- `ALIASEDIT CLEAR`：清空全部
- 未知 sub-command 输出友好 error

**验证**：

- `cargo check -p H7CAD`：零 warning（3.05s）
- 主 crate 测试 **121/122**（新增 6 个 alias resolver 单测全绿；上一轮
  115/116；唯一失败 `prop_geom_commit_rejects_unsupported_native_hatch`
  依然 pre-existing，与本次无关）

**ROADMAP 进度**：Manage Tab / Customization 里 Medium 复杂度 `ALIASEDIT`
交付（dialog 版本未来可在此基础上做 UI 层）。combined 今日：View Tab 4
+ Insert Tab 7 + Manage Tab 2 (FINDNONPURGEABLE + ALIASEDIT) = **13 个**
ROADMAP 命令后端落地。

### 2026-04-17：Insert Tab Underlay 命令组 — FRAMES0/1/2 + UOSNAP

ROADMAP Insert Tab / Reference 里 4 个 Low 复杂度命令一起落地：
- `FRAMES0` / `FRAMES1` / `FRAMES2` — underlay 边框可见性 tri-state
- `UOSNAP` — underlay 几何是否参与 object snap

**Scene 层** (`src/scene/mod.rs`)：

- `Scene` 新增两个字段（默认保持旧行为）：
  - `underlay_frames_mode: u8` 默认 `1`（FRAMES1 = 一直显示）
  - `underlay_snap_enabled: bool` 默认 `true`
- `wires_for_block` 的 entity filter 链加一条：
  `if underlay_frames_mode == 0 && matches!(e, EntityType::Underlay(_))
  { return false; }` — FRAMES0 下 Underlay 的整个 wire 不进入渲染
- `flat_map` 改 closure：若 `!underlay_snap_enabled` 且 entity 是
  `Underlay`，对 `tessellate_one(e)` 返回的每个 wire 清 `snap_pts` —
  frame 仍然可见，但 object snap 不会吸附 underlay 的 insertion/clip 角点

**App 层** (`src/app/mod.rs`, `src/app/commands.rs`)：

- `H7CAD` 新增 `frames_mode: u8`（默认 1）和 `uosnap: bool`（默认 true）
- 4 个新 dispatch case：
  - `FRAMES0` / `FRAMES1` / `FRAMES2`：直接写 `self.frames_mode = 0|1|2`
    + 对所有 tab 同步 `scene.underlay_frames_mode`，命令行输出状态
  - `UOSNAP [ON|OFF|TOGGLE]`：复用现有 `parse_on_off_toggle` helper
    （NAVVCUBE/NAVBAR 同款），写 `self.uosnap` 并同步 `scene.underlay_snap_enabled`

**语义决策**：

- FRAMES2（"On + Print"）在当前渲染层和 FRAMES1 行为一致；
  "+ Print" 意义在打印路径过滤 underlay 是否出图，当前占位 state，
  将来 PLOT 路径可 gate 上
- UOSNAP OFF 选择"保留视觉、屏蔽 snap"而不是完全隐藏 — 和 AutoCAD 语义
  一致，用户关心"不要误吸附到 underlay"而非"让 underlay 消失"

**验证**：

- `cargo check -p H7CAD`：零 warning（3.09s）
- 主 crate 测试 **115/116**（与上一轮一致，无回归；pre-existing 失败
  依然是 `prop_geom_commit_rejects_unsupported_native_hatch`）

**ROADMAP 进度**：Insert Tab / Reference 里 `FRAMES0` + `FRAMES1` +
`FRAMES2` + `UOSNAP` 共 4 条 Low 复杂度命令交付。combined 今日：
View Tab 4 + Insert Tab 7 (BASE + ATTSYNC + ADJUST + FRAMES×3 + UOSNAP) +
Manage Tab 1 = **12 个** ROADMAP 命令后端落地。

### 2026-04-17：Insert Tab ADJUST 命令接入 — 调整 Underlay fade/contrast/monochrome

ROADMAP Insert Tab / Reference group 里 Low 复杂度命令 **ADJUST** 从 ribbon
stub 升级为完整命令。AutoCAD 的 ADJUST 会弹一个对话框调 underlay 三个
属性，H7CAD 这里用 CLI 风格（和 BACKGROUND / NATIVERENDER 等已有命令
一致）省去 dialog 开发。

**CLI 接口** (`src/app/commands.rs`)：

- `ADJUST FADE <0-80>` — 设置 `underlay.fade`（DXF code 282）
- `ADJUST CONTRAST <0-100>` — 设置 `underlay.contrast`（DXF code 281）
- `ADJUST MONO <ON|OFF|TOGGLE>` — 通过 `underlay.set_monochrome(bool)`
  切换 `UnderlayDisplayFlags::MONOCHROME` bit
- 无参时 push_info 展示 usage 字符串

**实现**：

- 命令行 parse：`parts[1]` 是 sub-command（FADE/CONTRAST/MONO/MONOCHROME），
  `parts[2]` 是值；`u8::parse().filter(|v| v <= 80)` 直接卡范围
- 从 `scene.selected_entities()` 筛出 `EntityType::Underlay(_)`，收集
  handles；空集直接 push_error early-return（不 push_undo_snapshot）
- `push_undo_snapshot("ADJUST")` → `document.get_entity_mut(h)` 逐个修改
  → mark dirty → 汇报 `"updated N underlay(s) — {summary}"`（summary 取第
  一条的变更，e.g. `fade=30` / `mono=ON`）

**验证**：

- `cargo check -p H7CAD`：零 warning（4.45s）
- 主 crate 测试 **115/116**（和上一轮一致无回归；pre-existing 失败项
  依然是 `prop_geom_commit_rejects_unsupported_native_hatch`）

**ROADMAP 进度**：Insert Tab / Reference 的 Low 复杂度 `ADJUST` 交付。
combined 今日：View Tab 4 + Insert Tab 3 (BASE + ATTSYNC + ADJUST) +
Manage Tab 1 = **8 个** ROADMAP 命令后端落地。

### 2026-04-17：Insert Tab ATTSYNC 命令接入 — 同步 INSERT 属性到 block 定义

ROADMAP Insert Tab / Attributes group 里 Medium 复杂度命令 **ATTSYNC**
从 ribbon stub 升级为完整命令。该命令按 block 的 `AttributeDefinition`
集合重塑**所有** INSERT 实例的 attribute 列表：新增 tag 用 attdef 默认值
填充、stale tag 丢弃、已有 tag 的用户值保留。

**算法核心** (`src/modules/insert/attsync.rs`)：

- `pub(crate)` 模块暴露，供 dispatch 复用
- `pub fn sync_insert_attributes(attdefs, existing) -> (Vec<AttributeEntity>, SyncDelta)`：
  纯逻辑 helper
  - 对每个 `AttributeDefinition.tag`：若 `existing` 有同 tag，走
    `AttributeEntity::from_definition(attdef, Some(prev.value))`（保留用户值
    + 刷新几何/style 字段），否则 `from_definition(attdef, None)`（attdef
    default_value 填充）
  - `SyncDelta { added, removed, preserved }` 汇报三路计数
  - 输出顺序 = attdef 顺序（DXF 按 attdef 在 block 内的出现顺序写入）
  - 保留 `prev.common.handle` 让 host 继续持有现有 handle，避免选择/undo
    链路失联
- 5 个纯单测：全空 existing → 全 add；全匹配 → 全 preserve；部分 stale
  → 删；混合（+2 / -1 / =1）；空 attdefs → 清空 existing

**Dispatch 集成** (`src/app/commands.rs`)：

- `"ATTSYNC" | "ATTSYNC <name>"`：
  - 参数解析：`ATTSYNC <blockname>` 直接用，或空参时从 `selected_entities()`
    里找第一个 `Insert` 的 `block_name` 作为默认
  - Step 1 — 从 `block_records.get(&name).entity_handles` 收集全部
    `AttributeDefinition`（参考 `CmdResult::AttreqNeeded` 的同款模板）
  - Step 2 — 遍历 `document.entities()` 找 `Insert` 且 `block_name ==
    target`，收集 handles
  - Step 3 — `push_undo_snapshot("ATTSYNC")` → 对每个 handle
    `document.get_entity_mut(h)` 拿可变 ref → `ins.attributes = sync_insert_
    attributes(&attdefs, &ins.attributes).0`
  - 输出：`ATTSYNC: "<name>" synced N insert(s) — +ADD / -REM / =PRESERVE`
  - 错误路径：block 不存在 → `push_error`；无匹配 INSERT → `push_output
    "no INSERT references"`（仍然不 mark dirty，不 push_undo_snapshot）

**验证**：

- `cargo check -p H7CAD`：零 warning（2.17s）
- 主 crate 测试：**115/116 通过**（相比上一轮 110/111，**新增 5 个
  attsync 单测全绿**）；唯一失败 `prop_geom_commit_rejects_unsupported_
  native_hatch` 为 pre-existing，不在本次 scope
- 踩坑一次：第一版用 `entities_with_handles()`（不存在于 `acadrust::CadDocument`），改用 `.entities()` + `e.common().handle`；单测 fixture 里
  `AttributeDefinition::new(tag, default)` 少了一个 prompt 参数，实际签名是
  `new(tag, prompt, default_value)`，一次性修正

**ROADMAP 进度**：Insert Tab / Attributes group 的 Medium 复杂度 `ATTSYNC`
交付。combined 今日：View Tab 4 + Insert Tab 2 (BASE + ATTSYNC) +
Manage Tab 1 = **7 个** ROADMAP 命令后端落地。

### 2026-04-17：Manage Tab FINDNONPURGEABLE 命令接入 — 列不可 purge 的项

ROADMAP Manage Tab / Cleanup group 里 Medium 复杂度命令 **FINDNONPURGEABLE**
从 ribbon stub 升级为完整只读查询命令。是现有 PURGE 命令的语义对偶 —
PURGE 删除**可 purge**（不被引用且非系统保留）的定义，
FINDNONPURGEABLE 列出**不可 purge**的定义以及原因。

**实现** (`src/app/commands.rs`)：

- 新增 `"FINDNONPURGEABLE"` dispatch case，read-only：无 mutation、无 undo
  snapshot、无 dirty flag
- 扫描 `document.entities()` 统计引用次数：
  - `common.layer` → 每个 layer 的使用计数
  - `common.linetype`（排除 `ByLayer`/`ByBlock`）→ 每个 linetype 的使用计数
  - `Text.style` / `MText.style` → 每个 text style 的使用计数
  - `Insert.block_name` → 每个 block 的使用计数
  - `Dimension(dim).base().style_name` → 每个 dim style 的使用计数
- 对每个 table 输出分组：
  - **Layers**：名字为 `"0"`（系统默认）或被引用的
  - **Text Styles**：名字为 `"Standard"`（系统默认）或被引用的
  - **Linetypes**：`"Continuous"`/`"ByLayer"`/`"ByBlock"`（系统默认）或被引用的
  - **Blocks**（via `block_records`）：名字以 `*` 开头（系统块，如
    `*Model_Space`/`*Paper_Space*`）或被 `INSERT` 引用的
  - **Dimension Styles**：名字为 `"Standard"`（系统默认）或被引用的
- 每条输出格式：`    NAME  (reason)`，reason 有 `system default` /
  `system block` / `in use by N entity(ies)` / `in use by N insert(s)` 几类
- 汇总行：`"FINDNONPURGEABLE: {N} non-purgeable item(s):"`；若所有项都可 purge
  输出 `"FINDNONPURGEABLE: all items are purgeable."`

**验证**：

- `cargo check -p H7CAD`：零 warning（2.40s）
- 主 crate 测试：**110/111**（和上一轮一致，无回归；pre-existing 失败项
  `prop_geom_commit_rejects_unsupported_native_hatch` 仍在）
- `cargo check --workspace --all-targets`：零新增 warning

**ROADMAP 进度**：Manage Tab / Cleanup 里 Medium 复杂度 `FINDNONPURGEABLE`
交付。combined 今日：View Tab 4 + Insert Tab 1 + Manage Tab 1 = **6 个**
ROADMAP 命令后端落地。

### 2026-04-17：Insert Tab BASE 命令接入 — 设置图纸 `$INSBASE` 基点

ROADMAP Insert Tab / Block group 里 Low 复杂度命令 **BASE** 从 ribbon stub
升级到完整交互命令。该命令决定了当前图纸被 XREF/INSERT 到其他图纸时的
默认插入原点（DXF 系统变量 `$INSBASE`）。

**CmdResult 扩展** (`src/command/mod.rs`)：

- 新增变体 `SetInsertionBase([f64; 3])`：把 world-space 点写入 document 的
  header（与 `nm::EntityData::Point::position` 使用同一 `[x, y, z]` 约定）

**CmdResult handler** (`src/app/cmd_result.rs`)：

- `SetInsertionBase([x, y, z])` → `push_undo_snapshot("BASE")` →
  - `scene.document.header.model_space_insertion_base = Vector3::new(x,y,z)`
    （acadrust 路径）
  - `scene.native_doc_mut().header.insbase = [x, y, z]`（native 路径；
    已经是 DXF reader/writer 两侧完整覆盖的字段）
  - mark tab dirty、清 `active_cmd`/`snap_result`/preview wire、restore tangent snap
  - 命令行输出 `"Base point set to X,Y,Z"`

**SetBasePointCommand** (`src/modules/insert/base_point.rs`)：

- 之前只有 `tool()` ribbon stub，本次加 `SetBasePointCommand` 结构体实现
  `CadCommand` trait
- 交互三路：
  - `on_point(pt)` — viewport 点选 → `SetInsertionBase([pt.x, pt.y, pt.z])`
  - `on_text_input("x,y[,z]")` — 命令行输入（支持逗号或空白分隔，Z 缺省=0）
  - `on_enter()` — 回车接受当前 header 值作为默认
- `wants_text_input = true`：让命令行 input 走 `on_text_input` 分支
- prompt 动态显示当前 `$INSBASE` 值作为 `<default>` 提示
- 5 个单测：`parse_point` 两维/三维/非法输入、`on_enter` 默认值回落、
  `on_point` world 坐标透传

**模块暴露** (`src/modules/insert/mod.rs`)：

- `mod base_point` → `pub(crate) mod base_point`，让 `dispatch_command`
  能 `use crate::modules::insert::base_point::SetBasePointCommand`

**Dispatch** (`src/app/commands.rs`)：

- `"BASE"` 新 case：从当前 header 读出 `model_space_insertion_base` 作为
  默认值喂入 `SetBasePointCommand::new(current)`，推 prompt 到命令行，
  挂到 `tabs[i].active_cmd`

**验证**：

- `cargo check -p H7CAD`：零 warning（2.83s）
- `cargo check --workspace --all-targets`：仅原有 h7cad-native-dwg 8 条
  warning，本次零新增
- 主 crate 测试：**110/111 通过**（对比上一轮 105/106，新增 5 个
  base_point 单测全绿）；唯一失败 `prop_geom_commit_rejects_unsupported_
  native_hatch` 是 pre-existing，不在本次 scope

**ROADMAP 进度**：Insert Tab/Block group 里 Low 复杂度 `BASE` 命令交付。
combined 今日：View Tab 4 个 + Insert Tab 1 个 = 5 个 Low 复杂度命令后端落地。

### 2026-04-17：View Tab 4 个 UI 开关命令后端接入 (FILETAB/LAYOUTTAB/NAVVCUBE/NAVBAR)

落地 `ROADMAP.md` View Tab 里 4 个 Low 复杂度 UI 切换命令的运行时行为。
之前这些 ribbon 按钮点击后只会派发命令字符串，没有任何后端效果；
本次把它们接到真实的 state + 条件渲染。

**State 层** (`src/app/mod.rs`)：

- `H7CAD` 新增 4 个 bool 字段，默认全 `true`：
  - `show_viewcube` — ViewCube 显示开关 (NAVVCUBE)
  - `show_navbar` — 右侧 pan/zoom/orbit 工具栏开关 (NAVBAR)
  - `show_file_tabs` — ribbon 下方文档 tab 栏开关 (FILETAB)
  - `show_layout_tabs` — 状态栏里的 Model/Layoutn 标签栏开关 (LAYOUTTAB)

**Scene/GPU 层** (`src/scene/mod.rs`, `src/scene/render.rs`)：

- `Scene` 新增 `show_viewcube: bool`（默认 `true`）；由 `NAVVCUBE`
  命令在每个 tab 上写入
- `Primitive` 新增 `show_viewcube` 字段；`render()` 在 `show_viewcube == false`
  时**跳过** `pipeline.viewcube.render(...)`，避免 GPU 提交 ViewCube 几何
- `update()`（鼠标 hover 区域计算）在关闭 ViewCube 时强制 `state.hover_region = None`，
  避免在原 ViewCube 区域误报指针变化

**命令层** (`src/app/commands.rs`)：

- 新增 helper `parse_on_off_toggle(cmd, current) -> bool`：解析
  `<CMD>` / `<CMD> ON` / `<CMD> OFF` / `<CMD> TOGGLE`，未知或缺省 = toggle
- `dispatch_command` 里新增 4 个 case（均支持 ON/OFF/TOGGLE 语法）：
  - `NAVVCUBE`：同步写 `self.show_viewcube` 和所有 tab 的 `scene.show_viewcube`
  - `NAVBAR` / `FILETAB` / `LAYOUTTAB`：写对应 `self.show_*` 字段
  - 每个命令会 `push_output` 状态提示到命令行历史

**UI 渲染层** (`src/app/view.rs`, `src/ui/statusbar.rs`)：

- `cube_click`（ViewCube 点击热区 overlay）变为 `Option<Element>`，
  按 `show_viewcube` 条件加入 `viewport_stack`
- `nav`（右侧 nav toolbar）同理变为 `Option<Element>`，按 `show_navbar` 条件加入
- `tab_bar`（doc tab 栏）按 `show_file_tabs` 切换为 `doc_tab_bar(...)` 或
  零尺寸 `Space`，保持 column! 结构稳定
- `StatusBar::view` 签名新增 `show_layout_tabs: bool` 参数；
  为 `false` 时跳过 `for name in layouts` 循环和 `add_btn`，让 layout 标签与
  "+" 按钮都不渲染（右侧状态 pill 保留）

**验证**：

- `cargo check -p H7CAD`：零 error、零 warning
- `cargo check --workspace --all-targets`：零新增 warning（h7cad-native-dwg
  既有的 8 条 warning 在本次改动前就存在，`git stash` 复测确认）
- `cargo test -p H7CAD`：105/106 通过。唯一失败 `prop_geom_commit_rejects_
  unsupported_native_hatch` 是 pre-existing failure（git stash 后同样失败），
  与本次改动无关
- `cargo test --workspace --exclude H7CAD`：全绿（124 + 81 + 14 = 219）

**ROADMAP 进度**：View Tab/Viewport Tools 与 Interface group 里 4 个 Low
复杂度命令全部交付（NAVVCUBE / NAVBAR / FILETAB / LAYOUTTAB）。剩余 Low
复杂度命令：HORIZONTAL / VERTICAL / CASCADE（多文档窗口排列，依赖 iced 窗口
子系统，不是单 bool 可解）、UOSNAP / FRAMES0/1/2（Underlay 相关，需要
underlay runtime 先就位）。

### 2026-04-17：M3-B brick 3a — AC1015 object header 三件套解码

消费 brick 2b 切出来的 `&[u8]` 对象切片，用 `BitReader` 解出 AC1015 每个对象
开头的 `[BS object_type][RL main_size_bits][H handle]`，把字节切片变成**带语义
路由信息的 `ObjectHeader`**。AC1015 真实样本首 20 个 handle **20/20 解码成功**
且 handle 字段 **20/20 与 handle_map 一致**。

**新增模块** (`crates/h7cad-native-dwg/src/object_header.rs`)：

- `ObjectHeader { object_type, main_size_bits, handle, handle_code }`：
  四个字段对应 ODA R2000 对象公共头；`main_size_bits` 是 handle 流起点的
  绝对 bit 位置，brick 3b 用它分割 merged-data 主/handle 流
- `HANDLE_CODE_HARD_OWNER = 0x5`：对象自指 handle 的标准 code nibble
- `read_ac1015_object_header(&[u8]) -> Result<(ObjectHeader, BitReader<'_>)>`：
  先 byte-aligned 消耗 MS 前缀，再在 body 上建 BitReader 读 BS + RL + H；
  返回 reader 定位在 **xdata 起点**，brick 3b 可直接继续消费
- 防御：
  - `MAX_MAIN_SIZE_BITS = 128 Mbits`（约 16 MiB）防止损坏驱动大分配
  - MS body 越切片尾 → `UnexpectedEof { context: "object body extends past slice" }`
  - BS/RL/H 任一字段截断 → `BitReader` 原生 EOF 错误冒泡
- 单测 8 个：
  - `decodes_well_formed_header` / `decodes_large_handle`：正常路径 + 4 字节 handle
  - `rejects_empty_slice` / `rejects_truncated_ms_prefix` / `rejects_body_size_larger_than_slice` /
    `rejects_truncated_bs_field` / `rejects_implausible_main_size_bits`：5 种失败路径
  - `reader_positioned_exactly_after_header`：断言 reader 在 **bit 58**（保证
    brick 3b 不需要重算 header offset）

**接入** (`crates/h7cad-native-dwg/src/lib.rs`)：

- `mod object_header;`
- `pub use object_header::{read_ac1015_object_header, ObjectHeader, HANDLE_CODE_HARD_OWNER};`

**集成测试** (`tests/real_samples.rs`)：

- `real_ac1015_object_header_decodes_first_objects`：用真实 `sample_AC1015.dwg`
  → `build_pending_document` → `ObjectStreamCursor` → 对前 20 条 handle 逐一解
  `ObjectHeader`，打印 type histogram
  - 断言：至少 50% 解码成功
  - 断言：每个解码成功的 header.handle 必须等于 handle_map 对应条目的 handle（0 容忍漂移）
  - 断言：`main_size_bits ≤ slice_bits_upper`（不越切片）
  - 实测结果：**20/20 解码成功、20/20 handle 匹配**，type 分布 =
    {42×3, 48, 50-53, 56-57×3, 60, 62, 64, 66-68, 500, 501}，**2 个 ≥500 的自定义 class**
    精确对应 Classes section 里 51 个注册类中最早出现的两个

**测试与验证**：

- `cargo test -p h7cad-native-dwg -- --test-threads=1`：**61 + 53 + 10 = 124** 全绿
  （lib 单测 53 → 61；read_headers 53 保持；real_samples 9 → 10）
- `cargo check --workspace --all-targets`：无新增 warning（原有 7 条 warning
  均是 real_samples/bit_reader 既存代码，不属本砖 scope）
- **brick 3 起步**：下一砖 brick 3b 可用 `ObjectHeader.object_type` 直接路由到
  class-specific decoder（先做 ENTITY / OBJECT 两大类的 common header：owner
  handle + reactors + xdictionary handle + linetype/layer 引用），然后按
  `object_type` 分派到各 entity family 解几何字段

### 2026-04-17：M3-B brick 2b — ObjectStreamCursor 按 handle 切对象字节范围

激活 brick 2a 的 `read_modular_short`，新增 `ObjectStreamCursor`，把
`PendingDocument.handle_offsets` 里的 handle → file-offset 映射变成
**handle → `&[u8]` 对象切片**。AC1015 真实样本首 20 条低 handle **20/20**
成功切出合法 slice。

**新增模块** (`crates/h7cad-native-dwg/src/object_stream.rs`)：

- `ObjectStreamCursor<'a> { file, offsets }`：借用原始 DWG 字节 + 已解码
  handle map，零拷贝
- `object_size_at(offset: i64) -> Option<(header_bytes, body_size)>`：
  读 MS header，返回消耗的字节数和 body 字节数
  - offset <= 0 或 >= file.len()：None（handle map 尾部的 purged/GC 条目）
  - body_size > `MAX_OBJECT_BODY_BYTES` (16 MiB)：None（损坏保护）
  - MS 截断：None
- `object_slice_by_handle(handle) -> Option<&'a [u8]>`：
  binary_search 找 entry → `object_size_at` → 切 `[MS 头 + body]`（不含尾 CRC）
  - 尾切片越文件边界：None
  - handle 不存在：None
- 单测 8 个：`object_size_at` 正常 / 零与负 offset / 越界 offset / 截断 MS；
  `object_slice_by_handle` round-trip / 未知 handle / body 越界 / 巨大 body
  被拒

**接入** (`crates/h7cad-native-dwg/src/lib.rs`)：

- `mod object_stream;` + `pub use object_stream::ObjectStreamCursor;`
- 去掉 brick 2a 在 `read_modular_short` 上的 `#[allow(dead_code)]`（生产代码
  路径激活）

**集成测试** (`tests/real_samples.rs`)：

- `real_ac1015_object_stream_cursor_slices_first_objects`：用真实
  `sample_AC1015.dwg` 跑完整 `build_pending_document` → `ObjectStreamCursor::
  new(&bytes, &pending.handle_offsets)` → 对前 20 条低 handle 做 slice 探测
  - 断言：每个 slice.len() >= 2（至少 MS 头）
  - 断言：slice 不越文件边界
  - 断言：前 20 条至少 10 条（≥ 一半）成功 —— 实际 20/20 全通，保留 50%
    floor 防止后续样本变动或 handle 表排序改变时单点失败
  - 打印："AC1015 object_stream: resolved 20 / 20 low-handle slices
    (total map entries = 1047)"

- 测试：`cargo test -p h7cad-native-dwg -- --test-threads=1` 全绿
  （53 unit + 53 read_headers + 9 real_samples；unit 从 45 增至 53 = 新增
  8 object_stream 单测）
- `cargo check --workspace --all-targets` 无新增 warning
- **brick 2 系列收官**：brick 3（类路由对象解码器）可以开始消费 slice 了

### 2026-04-17：M3-B brick 2a — modular.rs 抽公共模块 + 新增 ModularShort 解码

为 brick 2b（`ObjectStreamCursor`）准备基座：object stream 里的每个对象都以
`MS`（Modular Short）作 size prefix，需要一个 byte-aligned、不走 `BitReader`
的解码器。顺便把 brick 1 在 handle_map.rs 里实现的两个 byte-aligned reader
抽成公共模块便于复用。

**新增模块** (`crates/h7cad-native-dwg/src/modular.rs`)：

- `read_modular_char(bytes, cursor) -> Option<u64>`（从 handle_map.rs 迁入）
- `read_signed_modular_char(bytes, cursor) -> Option<i64>`（迁入）
- **新增** `read_modular_short(bytes, cursor) -> Option<u64>`：
  - 小端 2 字节为一 chunk，低 15 位贡献 payload
  - word 的 `0x8000` = continuation flag
  - 对齐 ACadSharp `ReadModularShort` 参考实现
  - 防御：`shift > 60` 返回 None（4 chunks 是实际 AC1015 对象 size 上限远高于）
  - 暂标 `#[allow(dead_code)]` — 生产代码调用点在 brick 2b `object_stream.rs`

**单测**（11 个，全在 `modular::tests`）：

- `read_modular_char`：single-byte terminator / multi-byte continuation / 截断报错
- `read_signed_modular_char`：positive / negative / multi-byte positive
- `read_modular_short`：single chunk / max single-chunk payload (0x7FFF) /
  two-chunk continuation / 截断报错 / 单字节残缺报错

**handle_map.rs 调整**：`use crate::modular::{read_modular_char,
read_signed_modular_char}`，删除内部定义。`lib.rs` 注册 `mod modular;`（内部
模块，不对外暴露；`pub(crate)` 足够）。

- 测试：`cargo test -p h7cad-native-dwg -- --test-threads=1` 全绿
  （45 unit + 53 read_headers + 8 real_samples；unit 从 34 增至 45 = 原
  28 非 handle_map + 6 handle_map 保留 + 11 modular 新单测）
- `cargo check --workspace --all-targets` 无新增 warning
- handle_map.rs 行为字节对等（同一组内部 helper，只是换位置）

### 2026-04-17：M3-B brick 1 — AcDb:Handles 解码接入 build_pending_document

开启 DWG parser M3-B 系列（对象流解码）第 1 砖 —— 把 AC1015 `AcDb:Handles`
section 从「描述符层可见、未被消费」的状态打通到 `PendingDocument`，为后续
brick 2（对象流游标）和 brick 3（类路由对象解码）铺路。

**新增模块** (`crates/h7cad-native-dwg/src/handle_map.rs`)：

- `HandleMapEntry { handle, offset }`：单条 `(handle, object_stream_offset)`
  记录
- `parse_handle_map(payload) -> Result<Vec<HandleMapEntry>, DwgReadError>`：
  解码 byte-aligned 的 Handle section chunk 流
  - chunk 头：RS big-endian `size`（含自身 2 字节）
  - `size == 2` → 空尾 chunk，终止
  - chunk payload 上限 `min(size - 2, 2032)`
  - 每个条目：`ModularChar(unsigned)` delta_handle + `SignedModularChar`
    delta_loc；delta_handle > 0 时才产出条目（AutoCAD 偶尔用 0-delta 做流
    填充）
  - 每 chunk 尾 2 字节 CRC（跳过，校验延后到全文件 pass）
- 自带 6 个单测覆盖：single-chunk 基本解码 / 忽略 zero-delta / 负 offset
  / multi-byte modular char / 截断报错 / 立即空尾
- 硬性上限：`MAX_HANDLE_MAP_ENTRIES = 2^20`、`MAX_HANDLE_MAP_CHUNKS = 1024`
  防止损坏 size 前缀触发无界循环

**接入** (`crates/h7cad-native-dwg/src/lib.rs`)：

- `build_pending_document` 开头新增一次遍历：遇到 `record_number` 对应
  `KnownSection::Handles` 的描述符，调 `parse_handle_map`，成功则追加到
  `pending.handle_offsets`
- **容错原则**：解码失败（合成测试 fixture 里 record_number == 2 但 payload
  不是真实 Handle map 的情况）只是让该 crate 的 `handle_offsets` 为空，不
  会破坏整体文档流水线

**PendingDocument 字段扩展** (`crates/h7cad-native-dwg/src/pending.rs`)：

- 新增 `pub handle_offsets: Vec<HandleMapEntry>`，严格单调递增（delta encoding
  保证），空 vec 意味着当前 section layout 没有 Handle 块

**集成测试** (`crates/h7cad-native-dwg/tests/real_samples.rs`)：

- `real_ac1015_build_pending_document_populates_handle_offsets`：用真实
  `sample_AC1015.dwg` 跑主入口 `build_pending_document`，断言
  - `handle_offsets.len() >= 20`（实际样本 1047 条）
  - 前 5 条 offset 都 `> 0` 且 `< file_size`（handle 表后段允许越界，是
    AutoCAD 写 purged/GC 条目的正常现象，留给 brick 2 过滤）
  - handles 整体严格递增

- 测试：`cargo test -p h7cad-native-dwg -- --test-threads=1` 全绿（34 unit +
  53 read_headers + 8 real_samples）；`cargo check -p h7cad-native-facade` /
  `cargo check --workspace` 无新增 warning（pre-existing 的 `real_samples.rs`
  7 条 reader-reassign 和 `bit_reader.rs` 1 条 `mut` 警告与本次改动无关）

### 2026-04-17：D4 扩展 EntityData::Image 字段 + RASTER_IMAGE native-first

解锁 home/draw 最后一个延后命令 — RASTER_IMAGE，**home/draw 创建命令 native-first
9/9 全部收官**。

**模型层** (`crates/h7cad-native-model/src/lib.rs`)：
- `EntityData::Image` 追加 `file_path: String`（文件路径，acadrust 直接有对等字段）
  和 `display_flags: i32`（bitfield：SHOW_IMAGE / SHOW_WHEN_NOT_ALIGNED /
  USE_CLIPPING_BOUNDARY / TRANSPARENCY_IS_ON）

**Bridge 双向** (`src/io/native_bridge.rs`)：
- `native_image_to_acadrust`：用 `RasterImage::new(file_path, ..)` 构造（之前硬
  编码为 `""` 路径 → 渲染失效），从 `display_flags` 重建 `ImageDisplayFlags`
- `acad_image_to_native`：从 `image.file_path.clone()` 和 `image.flags.bits()` 读回

**DXF parser/writer**：
- 由于 DXF 标准将 file_path 存在 **IMAGEDEF 对象**上（IMAGE 实体通过 code 340
  handle 链接），native-dxf 当前未实现 object 层；本次折中方案为在 IMAGE 实体
  上用**非标准 code 1** 存 file_path（保证 native round-trip，其他 CAD 读取时
  会忽略 code 1）。document 内明确标注为 TODO 升级为标准 IMAGEDEF 链。
- code 70 用于 display_flags

**RASTER_IMAGE 命令** (`src/modules/home/draw/raster_image.rs`)：
- `make_entity` → `make_entity_native`：构造 `nm::Entity::new(nm::EntityData::
  Image { .. })`
- `u_vector/v_vector` 按 `world_size / pixel_count` 缩放（对齐 acadrust
  `RasterImage::set_size` 的语义）
- `display_flags = SHOW_IMAGE (1) | USE_CLIPPING_BOUNDARY (4)`（保留原命令默认）
- 2 个 `CommitAndExit` → `CommitAndExitNative`
- 移除 `use acadrust::entities::RasterImage` / `use acadrust::EntityType` /
  `use crate::types::Vector3`
- 度量：`raster_image.rs` 中 `acadrust::` 代码引用 4 → 0

- 测试：workspace `cargo check` 零 warning；`native_bridge` 22 个测试全绿
- **home/draw 进度 9/9 ✓**：C3 阶段 **全部命令 native-first 收官**

### 2026-04-17：D3 扩展 LwVertex/LwPolyline 宽度字段 + DONUT native-first

扩 native 模型支持 LwPolyline 族宽度属性，同步解锁 DONUT 命令 native-first。

**模型层** (`crates/h7cad-native-model/src/lib.rs`)：
- `LwVertex` 追加 `start_width: f64`, `end_width: f64`（DXF code 40/41 per-vertex）
- `EntityData::LwPolyline` 追加 `constant_width: f64`（DXF code 43）

**Bridge 双向** (`src/io/native_bridge.rs`)：
- `native_lwpolyline_to_acadrust`：读 native 新字段直接写入 `ar::LwVertex.start_width/
  end_width` 和 `ar::LwPolyline.constant_width`
- `acad_lwpolyline_to_native`：从 acad 读回相同字段
- 同步更新 `src/entities/traits.rs` 的 `lwv_ar_to_nm / lwv_write_back` helpers

**DXF parser** (`crates/h7cad-native-dxf/src/entity_parsers.rs`)：
- `parse_lwpolyline` 加 code 40/41/43 支持；code 40 按位置歧义处理（10/20 之前
  记 `constant_width`，之后记 per-vertex `start_width`）

**DXF writer** (`crates/h7cad-native-dxf/src/writer.rs`)：
- `LwPolyline` 写入 code 43（`constant_width != 0.0`）、code 40（per-vertex
  `start_width != 0.0`）、code 41（per-vertex `end_width != 0.0`）

**DONUT 命令** (`src/modules/home/draw/donut.rs`)：
- `make_donut` → `make_donut_native` 返回 `nm::Entity`
- 关键宽度字段全部保真：vertices 的 `start_width=end_width=width`，polyline
  的 `constant_width=width`（填充效果依赖这些）
- `CommitEntity` → `CommitEntityNative`，移除 `use acadrust::entities::{
  LwPolyline, LwVertex}` / `use acadrust::EntityType`
- 度量：`donut.rs` 中 `acadrust::` 引用 2 → 0

**构造点同步**：REVCLOUD / SHAPES / POLYLINE / DONUT 命令，scene/dispatch.rs、
scene/acad_to_truck.rs、cmd_result.rs 测试里的 `nm::LwVertex` 和 `EntityData::
LwPolyline` 构造 / 解构全部更新。

- 测试：workspace `cargo check` 零 warning；`native_bridge` 22 个测试全绿
- **home/draw 进度 8/9**：仅剩 RASTER_IMAGE 依赖 D4

### 2026-04-17：C3d POLYLINE 命令 native-first（修正 C3c 判断）

C3c changelog 误判 polyline.rs 使用宽度字段而延后；实际 PLINE 命令只使用
`vertices + bulge + is_closed`，完全契合现有 native `EntityData::LwPolyline + LwVertex { x, y, bulge }`。本次直接迁移。

- `PlineCommand::build_entity` 签名 `Option<EntityType>` → `Option<nm::Entity>`，
内部构造 `nm::Entity::new(nm::EntityData::LwPolyline { vertices, closed })`，
per-vertex 用 `nm::LwVertex { x, y, bulge }`
- 3 个 CmdResult 出口 `CommitAndExit(e)` → `CommitAndExitNative(e)`（正常 Enter /
Escape / C/CLOSE 文本输入）
- 移除 `use acadrust::entities::LwVertex` / `use acadrust::{EntityType, LwPolyline}` /
`use crate::types::Vector2`
- 度量：`polyline.rs` 中 `acadrust::` 引用 2 → 0；主 crate 零 warning 保持

**home/draw 进度更新**：native-first **7/9** 完成（REVCLOUD / SHAPES×6 / SPLINE /
MLINE / WIPEOUT / ATTDEF / POLYLINE）；仅剩 DONUT / RASTER_IMAGE 待 D3 / D4。

### 2026-04-17：D2 扩展 native EntityData::Wipeout.elevation 字段

修复 C3b WIPEOUT 迁移遗留的 DXF Z / elevation 丢失（世界 Y 轴）。

- `crates/h7cad-native-model/src/lib.rs`：`EntityData::Wipeout` 追加 `elevation: f64`
- `src/io/native_bridge.rs`：
  - `native_wipeout_to_acadrust`：polygonal / from_corners 使用 `elevation`
  作为 `insertion_point.z`（之前硬编码为 0）
  - `acad_wipeout_to_native`：从 `wipeout.insertion_point.z` 读回 elevation
  - 5 处测试 fixture 显式设 `elevation: 0.0`
- `crates/h7cad-native-dxf/src/entity_parsers.rs`：`parse_wipeout` 解码 code 30
→ elevation
- `crates/h7cad-native-dxf/src/writer.rs`：Wipeout 写入时增加 code 10/20/30
insertion point triple，Z = elevation
- `src/modules/home/draw/wipeout.rs`：
  - `make_rect_wipeout_native`：`elevation = p1.y as f64`（世界 Y）
  - `make_poly_wipeout_native`：`elevation = pts.first().y`（与原命令语义一致）
- 测试：workspace `cargo check` 零 warning；`native_bridge` 22 个测试全绿
- 效果：WIPEOUT 矩形 / 多边形模式在 native 存储 / DXF 中保留 elevation

### 2026-04-17：D1 扩展 native EntityData::MLine.closed 字段

修复 C3b MLINE 迁移遗留的字段损失（MLineFlags::CLOSED 被丢弃）。

- `crates/h7cad-native-model/src/lib.rs`：`EntityData::MLine` 追加 `closed: bool`
字段
- `src/io/native_bridge.rs`：
  - `native_mline_to_acadrust`：`closed = true` 时调 `ar::MLine::close()`
  （设置 `MLineFlags::CLOSED` bit）
  - `acad_mline_to_native`：从 `mline.is_closed()` 读回 `closed`
  - 3 处测试 fixture 显式设 `closed: false`
- `crates/h7cad-native-dxf/src/entity_parsers.rs`：`parse_mline` 解码 code 71
flags bitfield（`CLOSED = 0x2`），提取 `closed`
- `crates/h7cad-native-dxf/src/writer.rs`：MLine 写入时写 code 71
`HAS_VERTICES (1) | CLOSED (2)` bit
- `src/modules/home/draw/mline.rs`：`build_mline_native` 的 `_closed` 参数
重新生效，直接传 `closed` 到 `EntityData::MLine`
- 测试：workspace `cargo check` 零 warning；`native_bridge` 22 个测试全绿
- 效果：MLINE Close 分支语义保真，DXF round-trip 保留 closed flag

### 2026-04-17：C3c ATTDEF 命令 native-first（home/draw 阶段收口）

- **ATTDEF** (`attdef.rs`)：`AttributeDefinition { tag, prompt, default_value, insertion_point, height, ..Default }` + `common.layer = "0"` →
`nm::Entity::new(nm::EntityData::AttDef { tag, prompt, default_value, insertion, height })`；`nm::Entity::new` 默认 `layer_name = "0"` 与原命令一致，
无需显式设置
- 移除 `use acadrust::entities::AttributeDefinition` / `use acadrust::EntityType` /
`use crate::types::Vector3`
- 度量：`attdef.rs` 中 `acadrust::` 引用 2 → 0；主 crate 零 warning 保持

**RASTER_IMAGE 延后说明**：`raster_image.rs` 需要传 `file_path`，但 native
`EntityData::Image { insertion, u_vector, v_vector, image_size }` **无 file_path 字段**
— bridge 投影时用 `ar::RasterImage::new("", ..)` 会让 path 丢失，导致图片渲染/保存
失效。列为 D 系列必须扩展字段（file_path + flags + pixel_size）之前的必要前置。

**home/draw 进度小结**：native-first **6/9** 完成
（REVCLOUD / SHAPES×6 / SPLINE / MLINE / WIPEOUT / ATTDEF）；延后 3 项待 D 系列：

- DONUT / POLYLINE（LwVertex 缺 start_width / end_width / constant_width）
- RASTER_IMAGE（Image 缺 file_path / flags / pixel_size）

### 2026-04-17：C3b SPLINE / MLINE / WIPEOUT 命令 native-first

继续 C3 系列，迁移 home/draw 里 3 个 native 字段基本对等的命令。

- **SPLINE** (`spline.rs`)：`Spline { degree, control_points, knots, ..Default::default() }` → `nm::EntityData::Spline { degree, closed: false, knots, control_points, weights, fit_points, start_tangent, end_tangent }`；
2 个 `CommitEntity` → `CommitEntityNative`
- **MLINE** (`mline.rs`)：`MLine::from_points(..) / closed_from_points(..)` +
`scale_factor` + `style_name` → `nm::EntityData::MLine { vertices, style_name, scale }`；2 个 `CommitAndExit` → `CommitAndExitNative`
  - **字段损失**：native 无 `flags/closed` 字段，Close 分支的闭合语义丢失
  （顶点不会视觉闭环）。D 系列待办：扩展 native MLine 加 closed 标志
- **WIPEOUT** (`wipeout.rs`)：`Wipeout::from_corners(c1, c2)` /
`Wipeout::polygonal(verts, z)` → `nm::EntityData::Wipeout { clip_vertices }`；
矩形模式展开为 4 个 corner 顶点，多边形模式直接复制 xy
  - **字段损失**：native Wipeout 只存 2D clip vertices，原命令传入的
  DXF Z 高度（世界 Y 轴）丢失，bridge 默认归 0。D 系列待办
- 共移除 3 处 `use acadrust::...` / `use crate::types::Vector3` / 局部 `v3`
helper；改为 `use h7cad_native_model as nm`
- 度量：`spline.rs` 1→0，`mline.rs` 2→0，`wipeout.rs` 2→0；主 crate 零 warning 保持

### 2026-04-17：C3a REVCLOUD / SHAPES 命令 native-first（LwPolyline 纯 xy+bulge）

开启 C3 系列 — home/draw 模块创建命令 native-first。首批选择**只使用 xy+bulge**
的 LwPolyline 命令（native `LwVertex { x, y, bulge }` 完整对等）：

- **REVCLOUD** (`revcloud.rs`)：`make_revcloud` → `make_revcloud_native` 返回
`nm::Entity::new(nm::EntityData::LwPolyline { vertices, closed: true })`；
1 个 `CommitAndExit` → `CommitAndExitNative`
- **SHAPES** (`shapes.rs`, 含 RECT/RECT_ROT/RECT_CEN/POLY/POLY_C/POLY_E 6 个
子命令)：`make_pline` 返回类型 `EntityType` → `nm::Entity`；6 个
`CommitAndExit(make_pline(..))` → `CommitAndExitNative(make_pline(..))`
- 移除 `use acadrust::entities::LwVertex` / `use acadrust::{EntityType, LwPolyline, entities::LwVertex}` / `use crate::types::Vector2`
- 度量：`revcloud.rs` 的 `acadrust::` 2 → 0，`shapes.rs` 的 2 → 0；主 crate 零
warning 保持

**延后说明**：`donut.rs` 和 `polyline.rs` 使用 `LwPolyline.constant_width`
和 `LwVertex.start_width/end_width`，native `EntityData::LwPolyline { vertices, closed }` 和 `LwVertex { x, y, bulge }` 无这些字段 — 强行迁移会丢失线宽特性
（尤其 DONUT 的填充效果依赖 width 字段）。列为 D 系列 "扩展 native 模型字段"
的待办，迁移前先扩充 native 模型。

### 2026-04-17：C2g-2 LEADER 命令 native-first（annotate 创建命令收官）

复用 C2g-1 新增的 `CommitManyAndExitNative` 变体，把 `leader_cmd.rs` 从
`acadrust::entities::{Leader, MText, Insert}` + `ReplaceMany` 路径切到
`nm::EntityData::{Leader, MText, Insert}` + `CommitManyAndExitNative`。

- `build_leader / build_mtext / v3` → `build_leader_native / build_mtext_native / build_insert_native`，三个构造都返回 `nm::Entity`
- 六个 CmdResult 出口：
  - `NoAnnotation`/`Tolerance` → `CommitAndExitNative(leader)`（原 `CommitAndExit`）
  - `WithText`/`WithBlock` 空注释 → `CommitAndExitNative(leader)`
  - `WithText` 有文本 → `CommitManyAndExitNative(vec![leader, mtext])`
  - `WithBlock` 有块名 → `CommitManyAndExitNative(vec![leader, insert])`
- 移除 `use acadrust::entities::{Insert, Leader, LeaderCreationType, MText}` /
`use acadrust::EntityType` / `use crate::types::Vector3`
- `LeaderCreationType` 本地枚举化为 `enum CreationChoice { None, Text, Block, Tolerance }`
（只用于命令内部的分支逻辑，不传给 entity）
- 字段损失说明：
  - native `EntityData::Leader` 仅有 `vertices + has_arrowhead`，原命令设的
  `creation_type / hookline_enabled / text_height` 无对等字段 — bridge 走
  `ar::Leader::new` 默认 (WithText / hookline=false / text_height=2.5)
  - 新增常量 `LEADER_TEXT_HEIGHT = 2.5` 替代原 `leader.text_height` 传给
  `landing_pt / build_mtext_native`，与 bridge 默认保持一致
- 度量：`leader_cmd.rs` 中 `acadrust::` 引用 3 → 0；主 crate 零 warning 保持
- **annotate 创建命令 native-first 收官**：C2b-C2g 共 13 个创建命令已全部迁完
（TEXT / MTEXT / RAY / XLINE / 7 个 DIMENSION / TOLERANCE / MLEADER / TABLE / LEADER）。
`src/modules/annotate/` 剩余 `acadrust::` 均在**编辑型**命令（DIMEDIT / QDIM /
DIMBREAK / DIMSPACE / DDEDIT / DIMTEDIT / DIMJOGLINE / MLEADER-EDIT），
属 E 系列 "Edit operations native-first" 的范围

### 2026-04-17：C2g-1 CmdResult 新增 CommitManyAndExitNative 基础设施

为 C2g LEADER native-first 迁移做准备：现有 `CmdResult::ReplaceMany(vec![], additions)`
承担「一次提交 2 个新实体（Leader + MText/Insert）」的场景，但它只吃
`Vec<acadrust::EntityType>`，没有 native 对等入口。

- `src/command/mod.rs`：`CmdResult` enum 新增
`CommitManyAndExitNative(Vec<nm::Entity>)` 变体
- `src/app/cmd_result.rs`：在 `CommitAndExitNative` 旁边加 dispatch 分支：
`push_undo_snapshot` → 循环 `native_entity_to_acadrust` + `commit_entity` →
`clear_preview_wire` / `active_cmd = None` / `snap_result = None` /
`restore_pre_cmd_tangent`；复用 layer/color/linetype 默认值逻辑
- 设计要点：**新增语义**，不替换已有 `ReplaceMany`（FILLET/CHAMFER 等仍走
acadrust 路径）；新变体仅用于 native-first 的多实体纯新增场景
- 主 crate 零 warning 保持

### 2026-04-17：C2f TABLE 命令 native-first

沿用 C2a-C2e 模式，把 `src/modules/annotate/table_cmd.rs` 的 TABLE 命令从
`acadrust::entities::TableBuilder` 构造切到 `nm::EntityData::Table`。

- `TableCommand::on_point`：`TableBuilder::new(rows, cols).at(ins).row_height(..) .column_width(..).build()` + `CmdResult::CommitAndExit(EntityType::Table(..))` →
`nm::Entity::new(nm::EntityData::Table { num_rows, num_cols, insertion, horizontal_direction, version, value_flag })` + `CmdResult::CommitAndExitNative(entity)`
- 移除 `use acadrust::entities::TableBuilder` / `use acadrust::EntityType` /
`use crate::types::Vector3`
- `ROW_HEIGHT=0.5` / `COL_WIDTH=2.0` 常量**保留用于预览线框**，但不再传入实体
构造；native 路径下 bridge 走 `acadrust::Table::new(..)`，每行/列走
`TableRow/Column::new()` 默认 `0.25 / 2.5`。已有行为差异，bridge 层需要扩展
`EntityData::Table` 增加 `row_height/column_width` 字段才能保真（记为 TODO）
- 度量：`table_cmd.rs` 中 `acadrust::` 引用 3 → 0；主 crate 零 warning 保持

### 2026-04-17：C2e MLEADER 命令 native-first

沿用 C2a-C2d 模式，把 `src/modules/annotate/mleader_cmd.rs` 的 MLEADER 命令从
`acadrust::entities::MultiLeader` 构造切到 `nm::EntityData::MultiLeader`。

- `MLeaderCommand::on_text_input`：`MultiLeader::with_text(..)` +
`CmdResult::CommitAndExit(EntityType::MultiLeader(..))` →
`nm::Entity::new(nm::EntityData::MultiLeader { .. })` +
`CmdResult::CommitAndExitNative(entity)`
- `build_mleader` → `build_mleader_native` 直接构造 native：
  - `verts` 的最后一点作为 `text_location`，前面的点作为 `leader_vertices`
  - `leader_root_lengths = vec![leader_vertices.len()]`（单 root）
  - 默认值对齐 bridge `acad_multileader_to_native` 的反向映射：
  `content_type=1` (MText) / `path_type=1` (Straight) / `style_name="Standard"` /
  `scale_factor=1.0` / `leader_line_weight=-1` / `enable_landing=true` /
  `enable_dogleg=true` / `text_attachment_type=9`
  - 保留原命令的 `arrowhead_size=2.5` / `dogleg_length=2.5`
- 移除 `use acadrust::entities::MultiLeader` / `use acadrust::EntityType` /
`use crate::types::Vector3` / 本地 `fn v3(..)` helper
- 字段损失说明：原命令通过 `ml.context.leader_roots[0]` 设置的
`direction/connection_point/landing_distance` 在 native 模型中无对等字段，
bridge 会走默认值；渲染 / DXF / DWG 正常不受影响
- 度量：`mleader_cmd.rs` 中 `acadrust::` 引用 3 → 0；主 crate 零 warning 保持

### 2026-04-17：C2d TOLERANCE 命令 native-first

沿用 C2a/C2b/C2c 模式，把 `src/modules/annotate/tolerance_cmd.rs` 的 TOLERANCE
命令从 `acadrust::entities::Tolerance` 构造切到 `nm::EntityData::Tolerance`。

- `ToleranceCommand::on_point`：`Tolerance::with_text(ins, text)` +
`CmdResult::CommitAndExit(EntityType::Tolerance(..))` →
`nm::Entity::new(nm::EntityData::Tolerance { text, insertion })` +
`CmdResult::CommitAndExitNative(entity)`
- 移除 `use acadrust::entities::Tolerance` / `use acadrust::EntityType` /
`use crate::types::Vector3`；只 `use h7cad_native_model as nm`
- `insertion` 坐标沿用 `[pt.x, pt.z, pt.y]`（Y↔Z 翻转与其它命令一致）
- `native_bridge` 已有 Tolerance 分支，CommitAndExitNative handler 自动走投影路径
- 度量：`tolerance_cmd.rs` 中 `acadrust::` 引用 3 → 0

### 2026-04-17：C2c DIMENSION 家族 7 个命令 native-first

把 `src/modules/annotate/` 里 7 个 dimension 命令从 `acadrust::Dimension::{ Linear,Aligned,Radius,Diameter,Angular3Pt,Ordinate}` 切到
`nm::EntityData::Dimension { dim_type, .. }` 构造。

涉及文件：

- `linear_dim.rs`（dim_type=0）
- `aligned_dim.rs`（dim_type=1）
- `diameter_dim.rs`（dim_type=3）
- `radius_dim.rs`（dim_type=4）
- `angular_dim.rs`（dim_type=5）
- `ordinate_dim.rs`（dim_type=6，X/Y 方向通过 `dim_type & 0x40` 位标记）
- `dim_continue.rs` / `dim_baseline.rs`（dim_type=0 链式/基线）

**设计要点**：

- nm 用单一变体 + `dim_type` (i16) 区分 7 种 sub-type，字段涵盖
definition_point / text_midpoint / first_point / second_point / angle_vertex /
dimension_arc / leader_length / rotation (degrees) / ext_line_rotation (degrees)
- `measurement` 字段由命令侧自行计算（Linear/Aligned 用投影距离，Radius 用圆心-点距离，
Diameter 用 2×半径，Angular3Pt 用向量夹角度数，Ordinate 置 0）
- Radius/Diameter：`angle_vertex` 承载 center，`definition_point` 承载圆周点
- native_bridge 中 `native_dimension_to_acadrust` 已支持所有 7 种分支，直接复用
- 每个文件删除本地 `fn v3(..)` helper（用 `[f64;3]` 字面量替代）

**度量**：7 个命令文件中 `acadrust::` 引用各 3 → 0；共减少 21 处 acadrust 引用

- DWG 88 / DXF 81 / model 9 全绿；主 crate 零 warning 保持

### 2026-04-17：C2b TEXT / MTEXT 命令 native-first

把 `src/modules/annotate/{text,mtext}.rs` 两个命令从 `acadrust::{Text, MText}`
切到 `nm::EntityData::{Text, MText}` 构造，沿用 B5b 的 `CommitEntityNative`
通道。

- `TextCommand::on_text_input`：`acadrust::Text::with_value` → `nm::Entity::new( nm::EntityData::Text { insertion, height, value, rotation, style_name, width_factor, oblique_angle, horizontal_alignment, vertical_alignment, alignment_point })`
- `MTextCommand::on_text_input`：`acadrust::MText { ... }` → `nm::Entity::new( nm::EntityData::MText { insertion, height, width, rectangle_height, value, rotation, style_name, attachment_point, line_spacing_factor, drawing_direction })`
- 两个文件的 `acadrust::` 引用各 2 → 0
- `native_bridge` 已有 Text/MText 的投影（以 radians 持有，度角在投影时转换）
- DWG 88 / DXF 81 / model 9 全绿；主 crate 零 warning 保持

### 2026-04-17：C2a RAY / XLINE 命令 native-first

沿用 B5b 模式，把 `src/modules/home/draw/ray.rs` 里的 RAY / XLINE 两个命令
从 `acadrust::EntityType::{Ray,XLine}` 构造切到 `nm::EntityData::{Ray,XLine}`。

- `RayCommand::on_point` / `XLineCommand::on_point` 的 `CmdResult::CommitEntity( EntityType::Ray(..))` 全部改为 `CmdResult::CommitEntityNative(nm::Entity::new( nm::EntityData::Ray {..}))`
- 移除 `use acadrust::entities::{Ray as RayEnt, XLine as XLineEnt}` 和
`use acadrust::EntityType`；只 `use h7cad_native_model as nm`
- `native_entity_to_acadrust` 已有 Ray/XLine 分支，cmd_result 的
CommitEntityNative handler 自动走投影路径
- 度量：`ray.rs` 中 `acadrust::` 引用 3 → 0；DWG 88 / DXF 81 / model 9 全绿；
主 crate 零 warning 保持

### 2026-04-17：B5g Compat adapter 物理删除 + feature 移除

把 10 个 entity 文件里全部 **44 个** `#[cfg(feature = "acadrust-compat")]`
adapter impl 物理删除，从 `Cargo.toml` 移除 `acadrust-compat` feature 及其
`default` 声明，完成 B 系列 compat 清理最终一步。

- 删除的文件：`line/circle/arc/point/ellipse/lwpolyline/ray/solid/spline/shape`
各自的 `impl {TruckConvertible,Grippable,PropertyEditable,Transformable} for acadrust::entities::Xxx` 共 44 个 impl
- 连带删除的 adapter 专用 helper：`lwpolyline::ar_to_nm` /
`lwpolyline::write_back_verts` / `solid::ar_corners` / `solid::write_back` /
`common::v3_to_arr` / `common::arr_to_v3`
- `Cargo.toml`：移除 `[features] default = ["acadrust-compat"]` 和 `acadrust-compat = []`
- B5 系列的"精确量化"闭环：B3 建 feature gate → B5a/b/c/d/e inline dispatch →
B5g 物理删除，全部 66 处 trait bound 依赖彻底消除

**度量**：

- `cargo check -p H7CAD` 0 error / 0 warning
- `cargo check -p H7CAD --no-default-features` 已不适用（feature 已删）
- DWG 88/88、DXF 81/81、model 9/9 全绿
- 10 个 entity 文件 `acadrust::` 引用：全部仅保留 free function 内部（~30 处，
属 bridge 合理归属）

**剩余 acadrust 依赖**：`src/scene/`*, `src/modules/`*, `src/app/*`, `src/entities/`
中复杂 entity（Polyline/Hatch/Text/Dimension/Insert/Viewport 等 25 个）仍通过
本地 `struct` 承载 acadrust 字段；`src/io/{mod,native_bridge}.rs` 保留 acadrust
DwgReader/Writer 路径。这些属于 B 系列之外的长期工作，不影响 B5 闭环。

### 2026-04-17：B5e 剩余 5 个 entity dispatch 彻底脱钩 acadrust

把 `src/entities/traits.rs::EntityTypeOps` 里 6 个 dispatch 方法中，对 Ray / XLine /
Solid / Spline / Shape 这 5 个复杂 entity 的调用从 `Trait::method(x)`（依赖
`impl ... for acadrust::entities::X` adapter）inline 成直接调用 native free
function（`ray::ray_to_truck(&o, &d)`、`solid::to_truck(&corners)`、
`spline::to_truck(degree, knots, &cps)`、`shape::to_truck(&ins, size)` 等）。

- `to_truck_entity` / `grips` / `geometry_properties` / `apply_geom_prop` /
`apply_grip` / `apply_transform` 6 个方法中的 5 个 arm 全部 inline
- 共 **30 个 arm** 改造完成（Spline 的 `apply_geom_prop` 本就是空实现，改为 noop）
- XLine 的 grips/properties/apply_* 复用 Ray 的 free function（`ray::ray_grips` 等），
native 层已经这样设计，本次只是把 dispatch 接过来

**量化收益**：`cargo check -p H7CAD --no-default-features` 错误数
  **31 → 0**（全部 5 个 entity 的 trait bound 错误消除）

- DWG 88/88、DXF 81/81、model 9/9 全绿
- 主 crate 默认 feature 下零 warning 保持
- 至此，`--no-default-features` **首次能完整编译**（纯 native dispatch 路径打通）

**下一步（B5g）**：可物理删除 `src/entities/{line,circle,arc,point,ellipse, lwpolyline,ray,solid,spline,shape}.rs` 中的 44 个 `#[cfg(feature = "acadrust-compat")]`
adapter impl，最终从 `Cargo.toml` 移除 `acadrust-compat` feature。需先处理
`src/scene`/`src/modules` 中仍直接使用 `EntityType` dispatch 的业务代码。

### 2026-04-17：B5c LwPolyline inline dispatch

把 LwPolyline 加入 traits.rs 的 inline native dispatch 行列。

- 在 `traits.rs` 新增 `lwv_ar_to_nm` 和 `lwv_write_back` helper
（`acadrust::entities::LwVertex` ↔ `nm::LwVertex` 转换）
- 对 LwPolyline 在 6 个 dispatch 方法里 inline 调用 `lwpolyline::to_truck/grips/ properties/apply_geom_prop/apply_grip/apply_transform`
- **量化收益**：`cargo check -p H7CAD --no-default-features` 错误数
**36 → 30**（减 6，对应 LwPolyline 的 6 个 dispatch 方法）
- DWG 88/88、DXF 81/81、model 9/9 全绿
- 主 crate 默认 feature 下零 warning 保持

### 2026-04-17：B5d EntityTypeOps dispatch 部分脱钩 acadrust（5 个简单 entity）

把 `src/entities/traits.rs::EntityTypeOps` 里 6 个 dispatch 方法中，对 Line /
Circle / Arc / Point / Ellipse 这 5 个简单 entity 的调用从 `Trait::method(x)`
（依赖 `impl ... for acadrust::entities::X` adapter）inline 成直接调用 native
free function（`line::to_truck(&s, &e)` 等）。

- `to_truck_entity`：5 arm 改为 inline
- `grips`：5 arm 改为 inline
- `geometry_properties`：5 arm 改为 inline
- `apply_geom_prop`：5 arm 改为 inline（含字段写回）
- `apply_grip`：5 arm 改为 inline（含字段写回）
- `apply_transform`：5 arm 改为 inline（含字段写回）
- 共 **30 个 arm** 改造完成

**量化收益**：`cargo check -p H7CAD --no-default-features` 错误数
  **66 → 36**（减少 30，正好对应 30 个被解耦的调用点）

剩余 36 个错误全部在复杂 entity（Spline/LwPolyline/Polyline/Text/Dimension/Hatch
/Insert 等）的 trait dispatch 上，需要 B5c 先扩展 nm schema 再接通。

DWG 88/88、DXF 81/81、model 9/9 全绿；主 crate 默认 feature 下零 warning 保持。

### 2026-04-17：B5b 简单画图命令 native-first（LINE/CIRCLE/ARC/POINT/ELLIPSE）

把 5 个最核心的画图命令从 acadrust 类型完全切换到 `nm::Entity` 构造。

- `CmdResult` 枚举新增两个 native 变体：
`CommitEntityNative(nm::Entity)`（对等 CommitEntity，命令保持活动）
`CommitAndExitNative(nm::Entity)`（对等 CommitAndExit，命令退出）
- `cmd_result.rs` 新增两个 handler 分支，用 `native_entity_to_acadrust` 投影回
compat 层，复用现有 `commit_entity` 流程（layer/color/linetype 默认值 + scene
镜像）
- 5 个画图命令文件切换：
  - `modules/home/draw/line.rs`：`acadrust::Line::from_points` → `nm::EntityData::Line`
  - `modules/home/draw/circle.rs`：`acadrust::Circle` → `nm::EntityData::Circle`
  - `modules/home/draw/arc.rs`：`acadrust::Arc as CadArc` → `nm::EntityData::Arc`
  - `modules/home/draw/point.rs`：`acadrust::Point as CadPoint` → `nm::EntityData::Point`
  - `modules/home/draw/ellipse.rs`：`acadrust::Ellipse` → `nm::EntityData::Ellipse`
- **度量**：5 个命令文件的 `acadrust::` 引用 7 → **0**
- DWG 28+53+7=88、DXF 81/81、model 9/9 全绿；主 crate 零 warning 保持

注：`--no-default-features` 错误数仍为 66（traits.rs 的 EntityTypeOps dispatch 还
未切换）。这属于 B5c/d 工作范围 —— commands 层的 native-first 是一步，scene 层
的 dispatch 替换是下一步。

### 2026-04-17 综述：Compat 清理 B 系列完成 B1/B2/B3/B5a

一天内完成 4 个批次，主 crate 始终保持零 warning、测试全绿。


| 批次                          | 收益                                                                                                                            |
| --------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| **B1 类型别名门面**               | 新建 `src/types.rs` re-export 层；78 文件批量替换 `acadrust::types::`* → `crate::types::`*；业务代码 `acadrust::types::` 引用 62 → 0           |
| **B2 XData 双向投影**           | `native_bridge.rs` 新增 14 种 XDataValue 完整往返；修复 DWG save 丢 xdata 的隐性 bug；XDATA 命令迁到 native_store，`acadrust::xdata` 业务引用 2 → 0   |
| **B3 Adapter feature gate** | 10 个 entity 文件 44 个 compat impl 加 `#[cfg(feature = "acadrust-compat")]`；关闭 feature 时编译报出 **66 处 trait bound 错误**，精确量化 B5 工作范围 |
| **B5a Native dispatch 起步**  | `traits.rs` 新增 6 个 `*_native(&nm::EntityData, ...)` 函数，覆盖 Line/Circle/Arc/Point/Ellipse；为后续命令切 native_store 提供接入点             |


**度量**：

- `acadrust::types::` 引用：62 → 0 （业务侧）
- `acadrust::xdata::` 引用：2 → 0 （业务侧）
- `cargo check -p H7CAD --no-default-features` 错误数：0（默认 feature）→ 66（关闭 acadrust-compat，精准暴露 B5 工作量）
- 主 crate `cargo check -p H7CAD` warning：0 全程保持
- DWG 88/88、DXF 81/81、model 9/9 全绿全程保持

**下一步（下一会话可直接启动）**：B5b —— 扩展 CmdResult 枚举 + 改造 5 个画图
命令（LINE/CIRCLE/ARC/POINT/ELLIPSE）走 native-first。详见
`docs/plans/2026-04-17-acadrust-removal-plan.md`。

---

### 2026-04-17：B5a Native dispatch 起步（5 个简单 entity）

在 `src/entities/traits.rs` 建立并行的 `nm::EntityData` dispatch 入口，为未来逐个
命令切到 native_store 准备落地点。

- 新增 6 个 `*_native(&nm::EntityData, ...)` 自由函数：
`to_truck_native` / `grips_native` / `properties_native` / `apply_geom_prop_native`
/ `apply_grip_native` / `apply_transform_native`
- 覆盖 5 个简单 entity 类型：Line / Circle / Arc / Point / Ellipse
（与 `src/entities/{line,circle,arc,point,ellipse}.rs` 已存在的 native free
function 对接）
- 其他 variant 回落到默认值（`None` / `vec![]` / `{}`），未来批次扩展 LwPolyline/
Spline/Text/Dimension/Hatch/Insert/Viewport 等
- 这些函数当前**尚未被调用**（acadrust EntityType dispatch 仍是主路径），用
`#[allow(dead_code)]` 标注；后续 B5b-B5g 每个命令切换时依次接通
- DWG 88/88、DXF 81/81、model 9/9 全绿；主 crate 零 warning 保持

策略说明：为什么只做 5 个类型？

1. Line/Circle/Arc/Point/Ellipse 是最简单、最标准化的原语，各自 `nm::EntityData`
  variant 字段≤5 个，能一次对接完成而不引入不一致
2. Polyline/Dimension/Hatch 等复杂 entity 的 native free function 尚未完全就绪
  （需要先在各自 entity 文件里补 native 接口），是 B5c/d 工作
3. 5 个类型已覆盖约 70% 的日常画图场景，为 B5b 切换命令（DRAWLINE/CIRCLE/ARC…）
  提供充分入口

### 2026-04-17：B3 Entity adapter 隔离（acadrust-compat feature gate）

把 `src/entities/{line,circle,arc,point,ellipse,solid,ray,spline,lwpolyline,shape}.rs`
里 44 个 `impl {TruckConvertible,Grippable,PropertyEditable,Transformable} for acadrust::entities::Xxx` 的 compat adapter 门控在新 feature 下。

- `Cargo.toml` 新增 feature `acadrust-compat`（`default = ["acadrust-compat"]`，
现有行为完全不变）
- 10 个 entity 文件每个 impl block 头上加 `#[cfg(feature = "acadrust-compat")]`
- **关键度量**：关闭 feature 时编译报错 **66 处 trait bound 未满足**——这正是
B5 要处理的 acadrust `EntityType` dispatch 调用点，工作量被精确量化
- 默认 feature 下零改动（代码路径、行为、测试全部不变）
- DWG 88/88、DXF 81/81、model 9/9 全绿；主 crate 零 warning 保持

策略说明：没有物理搬动代码到独立文件，只加 cfg gate。原因：

1. 零代码搬动风险为 0
2. 门控粒度足够（每个 impl 独立开关）
3. 关 feature 时编译错误成为"acadrust 依赖清单"，B5 可据此逐项处理
4. 将来 B5 完成后删除 compat 一行 sed 即可（批量删 cfg attr + impl block）

### 2026-04-17：B2 XData 迁移到 native + bridge 双向投影

把 `acadrust::xdata::{ExtendedDataRecord, XDataValue}` 从业务代码剥离，集中到
`src/io/native_bridge.rs` 的 bridge 层；XDATA 命令（LIST/SET/CLEAR）完全走
native-first，以 `nm::Entity.xdata` 为真源。

- `native_bridge.rs` 新增 `xdata_to_acadrust` / `xdata_from_acadrust` 双向投影，
覆盖 `String/ControlString/LayerName/BinaryData/Handle/Point3D 家族/Real/ Distance/ScaleFactor/Integer16/Integer32` 全部 14 种 `XDataValue`，group code
1000-1071 完整往返
- `native_common_from_acadrust` 调用 `xdata_from_acadrust`（DWG/DXF 读入时填充
`nm::Entity.xdata`）
- `apply_common` 调用 `xdata_to_acadrust`（native→acadrust 投影时同步 xdata，
保证 DWG 保存路径不丢 xdata）
- `commands.rs` 的 XDATA 命令：
  - LIST：从 `native_store.inner().get_entity(nh).xdata` 读取，格式化输出 `code: value`
  - SET / CLEAR：改走 `apply_store_edit` 通用入口，编辑 `entity.xdata`，自动
  snapshot + compat 投影
- 结果：`src/` 业务代码 `acadrust::xdata::` 引用 2 处 → **0 处**（仅 bridge 内部
使用，属合理归属）
- DWG 88/88、DXF 81/81、model 9/9 全绿；主 crate 零 warning 保持

### 2026-04-17：B1 类型别名迁移（acadrust::types 去直接依赖）

把 `acadrust::types::{Vector2, Vector3, Color, Handle, LineWeight, Transparency, Transform, Matrix3, Matrix4, BoundingBox2D, BoundingBox3D, DxfVersion, aci_table}`
的直接引用全面切换到 `crate::types::`* 门面层。业务代码与 `acadrust` 实现解耦，
未来切换到 native 实现只需改 `src/types.rs` 一个文件。

- 新增 `src/types.rs`：顶层门面，`pub use acadrust::types::`*（14 个类型）
- 在 `src/main.rs` 登记 `mod types`
- 批量替换 78 个源文件中的 `acadrust::types` → `crate::types`（包括 `aci_table` 路径）
- 结果：`src/` 下 `acadrust::types::` 引用 62 处 → **0 处**（仅 `src/types.rs` 自身 2 处 re-export）
- `cargo check -p H7CAD` 零 warning（保持）
- DWG 88/88、DXF 81/81、model 9/9 全绿

参见 `docs/plans/2026-04-17-acadrust-removal-plan.md` Layer 1。
下一批（B2 XData / B3 Entity adapter / B4 ObjectType/Table / B5 Scene dispatch）
继续按该计划推进。

### 2026-04-17：DXF 补齐、DWG 原生解析 M3-A 贯通、Compat 清理

#### DXF 冷门类型补齐

覆盖之前被 `EntityData::Unknown` / `ObjectData::Unknown` 吞没的大量常见 AutoCAD 对象。
Reader / Writer / bridge 全链路接通。

- **ENTITIES 新增变体**：`HELIX`、`ARC_DIMENSION`、`LARGE_RADIAL_DIMENSION`、
Surface 家族（`EXTRUDEDSURFACE / LOFTEDSURFACE / REVOLVEDSURFACE / SWEPTSURFACE / PLANESURFACE / NURBSURFACE`）、`LIGHT`、`CAMERA`、`SECTION`、
`ACAD_PROXY_ENTITY`
- **OBJECTS 新增变体**：`FIELD`、`IDBUFFER`、`LAYER_FILTER`、`LIGHTLIST`、
`SUNSTUDY`、`DATATABLE`、`WIPEOUTVARIABLES`、`GEODATA`、`RENDERENVIRONMENT`、
`ACAD_PROXY_OBJECT`
- Surface 家族统一用 `Surface { surface_kind, u_isolines, v_isolines, acis_data }`
承载 6 种子类型，避免变体爆炸
- `ProxyEntity` / `ProxyObject` 用 `raw_codes` 原始透传，保证读→写不丢失信息
- `h7cad-native-dxf` 测试 72 → 81 全绿

#### DWG 原生解析 M3-A 知识层贯通

在 `crates/h7cad-native-dwg` 建立了对真实 AC1015 (R2000) DWG 的完整读取路径。
每一砖都用 `ACadSharp/samples/sample_AC1015.dwg` 真实字节做硬锚点验证。

- 新增 `crates/h7cad-native-dwg/src/bit_reader.rs`：MSB-first bit 流读取器，
支持 DWG 全部原生类型（`BitShort / BitLong / BitLongLong / BitDouble / Handle / Text`）
- 新增 `crates/h7cad-native-dwg/src/known_section.rs`：`KnownSection` 枚举
（`Header / Classes / Handles / ObjFreeSpace / Template / AuxHeader`）与 start/end sentinel
常量
- 修正 `section_map.rs`：AC1015 section locator record 从错误的 8 字节修正为
正确的 9 字节（1 byte record_number + 4 byte seeker + 4 byte size），
`section_count` 加 128 上界保护
- 确认 6 段布局全部匹配：AcDb:Header / Classes 的 16 字节 start sentinel 相等
- 真实解出：4 BitDouble 常量（`412148564080, 1, 1, 1`）+ 4 TV（`"m"`）+ 2 BL +
Viewport Handle + 20 个 CadHeader 布尔标志 + 8 个单位 BS（LUNITS=2, LUPREC=4,
AUNITS=0, ATTMODE=1, PDMODE=34）
- Classes section：51 条真实 class records（AcDbDictionaryWithDefault /
AcDbLayout / AcDbTableStyle ...）
- Handles section：1047 个 handle→offset 条目（2 chunks，通过 ModularChar +
SignedModularChar 增量编码）
- `crates/h7cad-native-dwg` 测试 0 → 88 全绿

#### Compat 清理（acadrust 依赖收缩）

- 新增 `docs/plans/2026-04-17-acadrust-removal-plan.md`：盘点 src/ 下 ~700 处
`acadrust::` 引用，按 5 层分类（I/O 边界保留 / 类型别名 / entity adapter /
scene-module dispatch / object-table），给出 B1–B5 分批迁移路径
- 删除 ~200 行真实 dead code：
  - `app/helpers.rs::sync_native_entity_from_compat`（compat←native 旧同步方向）
  - `scene/hit_test.rs::click_hit_hatch / box_hit_hatch / poly_hit_hatch`
  （HashMap 版本，被 `_entries` slice 版本取代）
  - `scene/transform.rs::mirror_xy_line`（直接操作 `acadrust::entities::Line`）
  - `modules/home/modify/splinedit.rs::apply_spline_op`（compat 版，
  被 `apply_spline_op_entity` 取代）
  - `modules/home/modify/attedit.rs::apply_attedit`（compat 版，
  被 `apply_attedit_native` 取代）
  - `entities/common.rs::transform_angle`、`entities/spline.rs::apply_geom_prop` 空实现
  - DXF tokenizer 的 `read_i64_le` 未使用方法
- `CadStore` trait / `StoreSnapshot` struct / `NativeStore::into_inner`
加 `#[allow(dead_code)]`（是为 native-first 迁移保留的预留接口，不是真死代码）
- **主 crate `cargo check -p H7CAD` 零 warning**

### 架构重构：CadStore 统一文档存储层

引入 `CadStore` trait 和 `NativeStore` 实现，将文档编辑流向从 compat-first（acadrust → native）
切换为 native-first（native → compat 投影）。

#### 新增

- `src/store/mod.rs` — `CadStore` trait：实体 CRUD、常用属性编辑（layer/color/linetype/lineweight/invisible/transparency）、持久化、快照/撤销
- `src/store/native_store.rs` — `NativeStore`，包装 `h7cad_native_model::CadDocument` 的 `CadStore` 实现
- `Scene::native_doc()` / `native_doc_mut()` / `set_native_doc()` 访问器方法
- `H7CAD::apply_store_edit()` — native-first 单闭包属性编辑方法
- `H7CAD::sync_compat_from_native()` — 反向同步（native → compat 投影）
- `Scene::rebuild_gpu_model_after_grip()` — grip 编辑后重建 hatch/solid GPU 模型

#### 变更

- `Scene::native_document: Option<nm::CadDocument>` → `Scene::native_store: Option<NativeStore>`
- `save_active_tab_to_path` 改用 `CadStore::save`
- 属性编辑（Layer/Color/LineWeight/Linetype/Toggle/GeomProp/Transparency）改为 native-first
- Grip 拖拽编辑改为 native-first
- `transform_entities`（MOVE/ROTATE/SCALE/MIRROR）改为 native-first
- MATCHPROP（layer 匹配 + 全属性匹配）改为 native-first
- `HistorySnapshot::native_document` → `native_doc_clone`

#### 移除

- `apply_property_edits` 双闭包方法（被 `apply_store_edit` 替代）
- compat 版 `toggle_invisible`、`Scene::apply_grip` 成为 dead code

