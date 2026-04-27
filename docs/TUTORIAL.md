# H7CAD 上手教程

| 字段 | 内容 |
|------|------|
| 版本 | v1.0（首版） |
| 对应代码 | v0.1.3，`main` @ `9540b2a`（round 39） |
| 读者 | **终端用户** —— 工程师、绘图员、CI / 出图工程师、SPPID 用户 |
| 配套阅读 | [`README.md`](../README.md)、[`docs/PRD.md`](PRD.md)、[`docs/ARCHITECTURE-TUTORIAL.md`](ARCHITECTURE-TUTORIAL.md)（开发者向）、[`COMMANDS.md`](../COMMANDS.md) |
| 维护方式 | 跟随 `CHANGELOG.md` 重大变更同步更新；命令列表以 [`COMMANDS.md`](../COMMANDS.md) 为单一真源 |

> 本教程是**用户向**的 Quick Start。开发者扩展、源码分层、迁移路线请看
> [`docs/ARCHITECTURE-TUTORIAL.md`](ARCHITECTURE-TUTORIAL.md)；阶段计划见
> [`docs/DEVELOPMENT-PLAN.md`](DEVELOPMENT-PLAN.md)；专项 SPPID BRAN 教程见
> [`docs/plans/2026-04-20-sppid-bran-tutorial.md`](plans/2026-04-20-sppid-bran-tutorial.md)。

---

## 1. 安装与启动

### 1.1 Linux（Flatpak，推荐）

```bash
flatpak remote-add --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
flatpak install H7CAD.flatpak             # 从 latest release 下载
flatpak run io.github.HakanSeven12.H7CAD
```

### 1.2 macOS / Windows / Linux（源码构建）

要求：**Rust 1.75+**。

```bash
git clone https://github.com/HakanSeven12/H7CAD.git
cd H7CAD
cargo build --release
./target/release/H7CAD
```

> Windows 用户启动后若未看到字体或 Ribbon 图标，请先执行一次 `cargo build --release`
> 让 `build.rs` 把 `assets/` 安装到 `target/release/`。

### 1.3 启动模式

```bash
h7cad                       # 进入 GUI
h7cad drawing.dxf           # 启动 GUI 并打开文件
h7cad drawing.dxf \
      --export-pdf out.pdf  # 不启窗，做批处理（详见第 11 节）
h7cad --help                # 命令行帮助
```

---

## 2. 第一张图：界面巡礼

启动后的窗口由四块组成：

```
┌──────────────────────────────────────────────┐
│  Ribbon (Home / Annotate / Insert / View / Manage)│
├──────────────────────────────────────────────┤
│                                              │
│           Viewport （Iced + WebGPU）         │
│                                              │
├──────────────────────────────────────────────┤
│  Command:  _                                 │  ← 命令行
├──────────────────────────────────────────────┤
│  Status: SNAP / ORTHO / GRID / OSNAP …       │  ← 状态栏
└──────────────────────────────────────────────┘
```

- **Ribbon Tab**：和源码 `src/modules/{home,annotate,insert,view,manage}` 一一对应。
- **命令行**：接受 AutoCAD 风格命令，**大小写不敏感**，支持别名（`L` = `LINE`，`PL` = `PLINE`）。
- **状态栏**：捕捉、正交、极轴、对象捕捉的开关；F8 切正交，F3 切对象捕捉。

> 所有命令的实现状态请查 [`COMMANDS.md`](../COMMANDS.md)：✅ 已实现，🔶 接受但行为为 stub，❌ 未实现。

---

## 3. 基础绘图：画一条线、一个矩形、一段弧

打开 H7CAD，确认右上角是 `Drawing1` / `Untitled`，然后在命令行依次输入：

```text
LINE
0,0
100,0
100,80
0,80
C                # 闭合
```

该序列会画出一个 100×80 的矩形（用 LINE 串成）。等价的更快做法：

```text
RECTANGLE
0,0
100,80
```

再在矩形里画一段弧：

```text
ARC                  # 默认 3 点 ARC
20,20
50,40
80,20
```

> 命令行随时可按 `Esc` 取消，或输入 `U` 撤销上一步。

### 3.1 多段线 / 圆 / 椭圆 / 样条

```text
PLINE          0,0  → 50,0  → 50,30 → ENTER       # 折线
CIRCLE         100,100   30                        # 圆，30 半径
ELLIPSE        80,40    150,40   30               # 椭圆
SPLINE         0,0  → 30,40 → 60,10 → 90,50 → ENTER
```

### 3.2 填充与渐变（HATCH / GRADIENT）

```text
HATCH          # 进入 HATCH 模式后选择内部点 / 边界
GRADIENT       # 渐变填充
```

H7CAD 的 PDF / SVG 导出在 **round 39** 已支持真实 gradient 输出（PDF 用 strip-fill，SVG 用 `<linearGradient>`），细节见 `CHANGELOG.md` 三十九轮条目。

---

## 4. 修改：MOVE / COPY / OFFSET / FILLET …

CAD 的核心一半在于**修改**而不是绘制。常用：

| 操作 | 命令 | 关键步骤 |
|------|------|----------|
| 移动 | `MOVE` (`M`) | 选对象 → 基点 → 第二点 |
| 复制 | `COPY` (`CO`) | 选对象 → 基点 → 多点连续粘贴 |
| 旋转 | `ROTATE` (`RO`) | 选对象 → 基点 → 角度 |
| 缩放 | `SCALE` (`SC`) | 选对象 → 基点 → 比例 |
| 镜像 | `MIRROR` (`MI`) | 选对象 → 镜像两点 → 是否删原 |
| 偏移 | `OFFSET` (`O`) | 距离 → 选对象 → 偏移侧 |
| 修剪 | `TRIM` (`TR`) | 选边界 → 选裁剪段 |
| 延伸 | `EXTEND` (`EX`) | 选边界 → 选延伸段 |
| 圆角 | `FILLET` (`F`) | 半径 → 两条边 |
| 倒角 | `CHAMFER` (`CHA`) | 距离 → 两条边 |
| 阵列 | `ARRAY` (`AR`) | 选对象 → 矩形 / 极坐标 / 路径子菜单 |
| 删除 | `ERASE` (`E`) | 选对象 → 回车 |

> 选择对象时支持 **窗口** 选（左→右）与 **跨选**（右→左），与 AutoCAD 同义。

---

## 5. 图层、线型、颜色

### 5.1 图层管理器

```text
LAYER           # 打开图层管理器
```

每个图层的 7 个状态都是独立切换：开 / 关、冻结 / 解冻、锁定 / 解锁、当前。常用快捷命令：

| 目标 | 命令 |
|------|------|
| 把选中对象的图层设为当前 | `LAYMCUR` |
| 关闭某图层 | `LAYOFF` |
| 冻结某图层 | `LAYFRZ` |
| 锁定某图层 | `LAYLCK` |

### 5.2 线型与缩放

```text
LINETYPE        # 线型管理（加载、删除、设当前）
LTSCALE 1.5     # 全局线型缩放
```

### 5.3 颜色

颜色挂在图层或 ByLayer，新对象继承当前层属性。颜色面板由 Ribbon Home 直接打开。

---

## 6. 标注与文本

```text
DIMLINEAR    # 线性标注
DIMALIGNED   # 对齐标注
DIMANGULAR   # 角度
DIMRADIUS    # 半径
DIMDIAMETER  # 直径
DIMCONTINUE  # 连续标注
DIMBASELINE  # 基线标注
QDIM         # 快速标注
DIMSTYLE     # 风格管理
MTEXT        # 多行文本
TEXT         # 单行文本
MLEADER      # 多重引线
```

文本风格管理：

```text
STYLE          # 文本风格
TABLESTYLE     # 表格风格
MLEADERSTYLE   # 多重引线风格
```

---

## 7. 块与外参

### 7.1 定义并插入块

```text
BLOCK         # 选实体 → 取名 → 基点 → 完成
INSERT        # 插入已定义的块或外部 .dwg
WBLOCK        # 把块写出为独立 .dwg / .dxf
```

### 7.2 外部参考（XREF）

```text
XATTACH       # 附着外参
XREF          # 外参管理器
XRELOAD       # 重载
REFEDIT       # 进入就地编辑
REFCLOSE      # 完成 / 放弃就地编辑
```

> 外参管理 / 属性管理目前覆盖度可看 [`COMMANDS.md` § Block & Reference](../COMMANDS.md)；
> `XCLIP` / `BEDIT` / `BLOCKPALETTE` / `ATTMAN` / `ATTSYNC` 当前为 🔶 stub，请先用主路径命令。

---

## 8. 3D 快速入门

```text
BOX 0,0,0     50,50,50         # 建一个 50³ 立方体
ORBIT                            # 进入 3D 轨道，左键拖动
SPHERE 100,0,0  20               # 球
CYLINDER 200,0,0 15  60          # 圆柱（半径 15，高 60）
EXTRUDE                          # 选一个闭合 2D → 拉伸高度
REVOLVE                          # 选 2D + 旋转轴 + 角度
SWEEP                            # 沿路径扫掠
LOFT                             # 多截面放样
```

### 8.1 物性与导出

```text
MASSPROP         # 体积、面积、惯性矩等
EXPORTSTEP       # 导出 STEP（适合下游 CAM）
EXPORTSTL        # 导出 STL（适合 3D 打印）
```

> 3D 布尔（`UNION` / `SUBTRACT` / `INTERSECT`）和 `SLICE` 在 v0.1.x **未实现**，
> 计划随 PRD § FR-3D-BOOL 立项推进。

---

## 9. 出图（PDF / SVG）

### 9.1 PDF（GUI）

1. 切到 Ribbon **Manage** 或在命令行输 `PLOT`；
2. 在打印对话框里选 **PDF** 导出；
3. 在 **PDF Export Options** 调整：
   - 单色 / 彩色（`monochrome`）
   - HATCH 是否走 pattern 填充（`pattern_hatches`）
   - HATCH gradient（`gradient_hatches`，round 39 起默认 on）
   - SPLINE 原生贝塞尔（`native_splines`）
   - 字体族（`font_family`）

### 9.2 SVG（GUI）

```text
SVGEXPORT                    # 默认：单色、内嵌图像、原生曲线
SVGEXPORT COLOR              # 保留 ACI 颜色
SVGEXPORT MONO               # 强制黑白
SVGEXPORT TEXTGEOM           # 文本转几何路径
SVGEXPORT NOHATCH            # 跳过 hatch
SVGEXPORT NOIMAGE / EXTIMG   # 不内嵌 / 外链图像
SVGEXPORT NOCURVES / NOSPLINES
SVGEXPORTDIALOG              # 打开图形选项对话框
```

完整字段语义见 [`docs/svg_export.md`](svg_export.md)。

---

## 10. 批处理 CLI（CI / 自动化）

H7CAD 的同一个二进制可作为**无窗导出器**：

```bash
# 单输入，显式输出
h7cad input.dxf --export-pdf out.pdf

# 单输入，省略输出（生成同 stem .pdf）
h7cad input.dxf --export-pdf

# 多输入到目录
h7cad A.dxf B.dxf C.dxf --export-pdf out_dir/

# SVG 用法相同
h7cad input.dxf --export-svg out.svg

# 用 JSON 覆盖默认 export options
echo '{"gradient_hatches": false}' > opts.json
h7cad drawing.dxf --export-pdf out.pdf --options opts.json

# 帮助
h7cad --help
```

退出码：**所有输入成功 = 0**；任一失败 = 1，但其它输入仍会尝试导出，
失败诊断打到 stderr。CI 里建议把 stderr 也收集为 artifact。

`--options <PATH>` 接收的是 `PdfExportOptions` / `SvgExportOptions` 的
**部分覆盖**（缺失字段回退到默认），二者字段名同源，可在同一个 JSON
混用。

---

## 11. P&ID / SPPID 入门

H7CAD 通过同仓兄弟 crate `pid-parse` 提供首期作者闭环。常用命令：

```text
PIDSETDRAWNO     # 编辑 PID Drawing XML 中的 SP_DRAWINGNUMBER
PIDSETPROP       # 编辑任意 SP_* 属性
PIDGETPROP       # 读当前缓存中的 SP_* 属性
PIDSETGENERAL    # 编辑 PID General XML 中的 <element>text</element>
SPPIDLOADLIB     # 加载首期最小 BRAN 元件库
SPPIDBRANDEMO    # 放置一个简单 BRAN 演示
SPPIDEXPORT      # 输出 .pid + _Data.xml + _Meta.xml 三件套
```

完整端到端用例（载库 → 放 BRAN → 填属性 → 出包 → 回开）见
[`docs/plans/2026-04-20-sppid-bran-tutorial.md`](plans/2026-04-20-sppid-bran-tutorial.md)。

> 真正的 SmartPlant `.dwg` **打开**仍受限于 P2 阶段的 DWG 运行时，
> facade 当前会显式返回 `native DWG reader not implemented yet`。
> P&ID 链路使用的是 `.pid` 而非 `.dwg`，不受该限制。

---

## 12. 配置与排错

### 12.1 偏好与选项

```text
OPTIONS         # 应用设置（部分项当前 🔶）
DRAWING         # 当前文档信息
PURGE           # 清理未使用项
RENAME          # 重命名命名对象
```

### 12.2 常见问题

| 现象 | 原因 / 处理 |
|------|------------|
| 启动报 `native DWG reader not implemented yet` | 当前刻意行为，参见 PRD § 2.2；请改用 `.dxf` |
| Ribbon 按钮按下没反应 | 该命令在 [`COMMANDS.md`](../COMMANDS.md) 标 🔶 / ❌；可在 `ROADMAP.md` 查复杂度 |
| PDF 中文字体丢失 | 在 `--options` JSON 里指定 `"font_family": "TimesRoman"` 等内置字体 |
| HATCH gradient 在打印边缘处溢出 | round 39 实现裁到边界 AABB，与真实 ACAD 同 fidelity tier；如需关闭，`--options` 设 `"gradient_hatches": false` |
| 批处理某文件失败但流水线没退出 | 多输入下单文件失败不致命，stderr 有诊断；用 `set -o pipefail` + grep 自行判断 |
| `cargo build` 在 macOS 上 link 慢 | 默认 lld 不开，可在 `~/.cargo/config.toml` 加 `[target.aarch64-apple-darwin] linker = "ld"` |

### 12.3 收集诊断

- GUI：`File → Show Logs`，或运行时设置 `RUST_LOG=info`。
- CLI：把 stderr 与 `--options` JSON 一起打包给上游。
- 内部模块：`src/io/diagnostics.rs` 是错误流向状态栏的中转点。

---

## 13. 下一步阅读

- 命令支持矩阵：[`COMMANDS.md`](../COMMANDS.md)（256 条 / 141 ✅）。
- 缺失能力清单与复杂度：[`ROADMAP.md`](../ROADMAP.md)。
- 每轮工作的实现笔记与 trade-off：[`CHANGELOG.md`](../CHANGELOG.md)。
- SVG 导出选项细节：[`docs/svg_export.md`](svg_export.md)。
- SPPID BRAN 端到端教程：[`docs/plans/2026-04-20-sppid-bran-tutorial.md`](plans/2026-04-20-sppid-bran-tutorial.md)。
- 想读源码 / 写新命令 / 加新模块：开发者教程 [`docs/ARCHITECTURE-TUTORIAL.md`](ARCHITECTURE-TUTORIAL.md)。
- 想了解产品边界与里程碑：[`docs/PRD.md`](PRD.md)。

---

*本教程随代码同次维护。命令名以 [`COMMANDS.md`](../COMMANDS.md) 为单一真源；
若教程与 `COMMANDS.md` 出现分歧，**以代码与 `COMMANDS.md` 为准**，并提交 PR
更新本文档。*
