# 更新日志

## 2026-05-14

### DXF / PID 保真与运行时显示

- 扩展 native DXF 模型与 writer，补齐多类保真对象的读写/回写覆盖，包括 block 动态参数、book color、raster 变量、raw/unknown object、spatial filter、table object、underlay definition 等 round-trip 场景。
- 强化 PID 导入路径：提升真实 PID 包中的几何恢复、属性诊断与缓存可观察性，并让 UI/状态栏能更清楚地反馈导入后的 preserved-only / fallback 内容。
- 扩展实体公共属性和 scene 命中/渲染链路，覆盖更多实体的 grip、显示属性、PDF/SVG 导出与 native bridge 转换。
- 完成 AC1015 DWG writer 阶段性闭环：LINE / CIRCLE / ARC / POINT / LWPOLYLINE / TEXT / ATTRIB / INSERT / MTEXT / SPLINE / ELLIPSE / RAY / XLINE / SOLID / 3DFACE / VIEWPORT / HATCH 等实体 writer 均有 round-trip 防回归记录。

### 验证

- `cargo build --release` 通过。
- 当前构建仅剩 11 个未使用代码相关 warning，未阻断 release binary 生成。
- `target/release/H7CAD.exe` 已在本机启动验证。

## 2026-05-06

### DXF / Native 渲染保真

- 合并并修复上游更新后的构建兼容问题，恢复 workspace 级检查与测试通过。
- 打开 DXF 时新增 native diagnostics：对 Unknown / Proxy 等 preserved-only 内容生成中文可读告警入口。
- 新增 native render stats，可区分 `native-rendered`、`compat-fallback`、`preserved-only` 三类实体。
- 扩展 native 渲染支持：Ellipse、Spline 进入 native conversion/render path。
- 修复 tilted Ellipse：native Ellipse 现在使用 `entity.extrusion` 计算 minor axis，quadrant snap 点也正确计算 Z 坐标。
- 修复 DIMSTYLE round-trip：native model 新增 `dimexe`，DXF DIMSTYLE 表按标准写读 `44=DIMEXE`、`147=DIMGAP`。
- native Dimension 渲染现在使用 `dimasz/dimexo/dimexe/dimtxt/dimscale` 等关键 dimstyle 参数。
- nested INSERT 的 ByBlock 继承增强：wire 与 hatch 都能继承最近有效 INSERT 的 color / linetype / lineweight。
- Hatch fill 增加 INSERT 平移、缩放、旋转的 boundary regression 覆盖。

### 验证

- `cargo check --workspace` 通过。
- `cargo test --workspace` 通过。
- `git diff --check` 通过。

### 开发文档

- 新增 DXF/native 集成后续开发计划、下一切片 brainstorm 方案、diff 盘点与提交切片建议。
