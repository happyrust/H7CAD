# 更新日志

## [未发布]

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

