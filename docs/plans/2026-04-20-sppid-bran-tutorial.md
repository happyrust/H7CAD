# H7CAD SPPID BRAN 教程

起稿：2026-04-20

## 目标

以真实样例 `DWG-0202GP06-01.pid / DWG-0202GP06-01_Data.xml / DWG-0202GP06-01_Meta.xml` 的发布口径为参照，在 H7CAD 中完成一条首期只读作者闭环：

1. 载入最小 SPPID 元件库
2. 放置一个简单 `BRAN`
3. 填好关键属性
4. 导出 `.pid + _Data.xml + _Meta.xml`
5. 再次回开导出的 `.pid`

首期范围刻意收窄为一个最小 BRAN 子图，不追求 SmartPlant 原始图形字节级复刻。

## 真实样例

本轮实现参照的真样例目录：

- `C:/Users/Administrator/Documents/xwechat_files/happydpc_b2ec/msg/file/2026-04/XML文件(1)/DWG-0202GP06-01.pid`
- `C:/Users/Administrator/Documents/xwechat_files/happydpc_b2ec/msg/file/2026-04/XML文件(1)/DWG-0202GP06-01_Data.xml`
- `C:/Users/Administrator/Documents/xwechat_files/happydpc_b2ec/msg/file/2026-04/XML文件(1)/DWG-0202GP06-01_Meta.xml`

样例表明：一个真实发布包里，`BRAN` 语义并不只是一个图形点，而是至少同时包含：

- `PIDPipingBranchPoint`
- `PIDBranchPoint`
- `PIDPipeline`
- `PIDPipingConnector`
- `PIDRepresentation`
- 若干 `Rel`

因此 H7CAD 首期作者态把 `BRAN` 简化为一个块条目，但导出时会展开为双支点语义。

## 第一步：装入最小 BRAN 块库

在 H7CAD 命令行输入：

```text
SPPIDLOADLIB
```

命令会向当前 CAD 图纸注入一个块定义：

- 块名：`SPPID_BRAN`
- 图形：一条主管、一条支管、一个中心圆点
- 隐藏属性：
  - `DRAWING_NO`
  - `DOC_TITLE`
  - `PIPELINE`
  - `CONNECTOR`
  - `PIPING_CLASS`
  - `NOMINAL_DIAMETER`
  - `BRANCH_NAME`

若当前已经有 `SPPID_BRAN`，命令会直接复用，不重复创建。

## 第二步：放置一个简单 BRAN

有两种走法。

### 走法 A：直接放 demo

```text
SPPIDBRANDEMO
```

这会自动做三件事：

1. 确保 `SPPID_BRAN` 块库存在
2. 在当前图里放置一个 `SPPID_BRAN` 插入
3. 补两条引导线，形成一个最小 BRAN 示意

默认属性如下：

- `DRAWING_NO = DWG-0202GP06-01`
- `DOC_TITLE = H7CAD BRAN Tutorial`
- `PIPELINE = A3jqz0101-OD`
- `CONNECTOR = A3jqz0101-OD-50 mm-1.6AR12-WE-50mm`
- `PIPING_CLASS = 1.6AR12`
- `NOMINAL_DIAMETER = 50 mm`
- `BRANCH_NAME = BRAN-1`

### 走法 B：手工 `INSERT`

如果你想自己控制插入点，可以先执行：

```text
INSERT
```

然后选择块名 `SPPID_BRAN`。H7CAD 会按块内 `ATTDEF` 逐项提示输入属性值。

## 第三步：导出发布包

在当前图只有一个 `SPPID_BRAN` 的前提下，输入：

```text
SPPIDEXPORT D:/tmp/bran-demo.pid
```

命令会同时写出三类产物：

- `D:/tmp/bran-demo.pid`
- `D:/tmp/bran-demo_Data.xml`
- `D:/tmp/bran-demo_Meta.xml`

首期导出对象集固定为一个最小 BRAN 子图：

- `PIDDrawing`
- `PIDPipeline`
- `PIDPipingConnector`
- `PIDPipingBranchPoint`
- `PIDBranchPoint`
- `PIDProcessPoint`
- `PIDRepresentation`

关系集固定覆盖以下发布证据：

- `DrawingItems`
- `DwgRepresentationComposition`
- `PipingEnd1Conn`
- `PipingEnd2Conn`
- `PipingTapOrFitting`
- `ProcessPointCollection`

`Meta.xml` 至少会写出：

- `DocumentVersion`
- `DocumentRevision`
- `File`
- `VersionedDoc`
- `RevisedDocument`
- `FileComposition`

### 当前导出限制

首期限制非常明确：

- 当前一张图里只允许导出一个 `SPPID_BRAN`
- 没有 `SPPID_BRAN` 时拒绝导出
- 多于一个 `SPPID_BRAN` 时拒绝导出
- 目标是 H7CAD 闭环与样例结构等价，不是外部 SmartPlant 字节级等价

## 第四步：核对 sidecar XML

导出后先看两个 sidecar：

- `*_Data.xml` 应能看到 `PIDPipingBranchPoint` 与 `PIDBranchPoint`
- `*_Meta.xml` 应能看到 `DocumentVersion / DocumentRevision / File`

若只是想粗看结构，可直接全文搜索以下关键词：

- `PIDPipingBranchPoint`
- `PIDBranchPoint`
- `PipingTapOrFitting`
- `ProcessPointCollection`
- `DocumentVersion`
- `FileComposition`

## 第五步：再次回开验证

重新在 H7CAD 中打开刚导出的 `.pid`：

```text
OPEN
```

选择 `D:/tmp/bran-demo.pid` 后，H7CAD 会自动寻找同名 sidecar：

- `bran-demo_Data.xml`
- `bran-demo_Meta.xml`

只要两者都在，PID 工作台就会把 sidecar 解析结果并入 `PidDocument`，从而在左侧浏览器与右侧属性栏里恢复最小 BRAN 语义图。

验收点：

1. 左栏能看到 `PIDPipingBranchPoint` 与 `PIDBranchPoint`
2. 中栏预览仍然可定位
3. 右栏属性里能看到 `PIPELINE / PIPING_CLASS / NOMINAL_DIAMETER / BRANCH_NAME` 等证据

## 推荐命令链

第一次走通时，建议直接按下面这条命令链操作：

```text
SPPIDLOADLIB
SPPIDBRANDEMO
SPPIDEXPORT D:/tmp/bran-demo.pid
OPEN
```

然后选择 `D:/tmp/bran-demo.pid` 复开。

## 首期边界

本教程对应的是 2026-04-20 这一版首期实现，边界如下：

- 只覆盖一个简单 BRAN
- 不做整张工艺图重建
- 不做 SmartPlant 原图几何还原
- 不做多 BRAN 拼装导出
- 不做 `.pid` 回写编辑器

后续若要扩到多支路、多元件库、或 SmartPlant 兼容性专项，应以本教程的单 BRAN 闭环为基线继续扩展。
