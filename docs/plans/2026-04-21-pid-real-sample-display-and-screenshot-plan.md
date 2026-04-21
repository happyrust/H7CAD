# PID Real Sample Display and Screenshot Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 让 H7CAD 以 `D:\work\plant-code\cad\pid-parse\test-file\工艺管道及仪表流程-1.pid` 为真实验收样本，做到可稳定打开、画面接近原始 P&ID 外观，并同时支持应用内 PNG 截图命令与自动化截图回归验证。

**Architecture:** 当前 PID 打开链路已经存在：`io::open_path` 调 `pid_import::open_pid`，再把 `PidOpenBundle.native_preview` 注入 `scene` 并在 `Message::FileOpened` 里 `fit_all()`。本计划不推翻现有架构，而是在这条链路上分层补齐：先增加真实样本可观测性，再修 `derive_layout -> pid_document_to_bundle -> scene fit/render` 的显示质量，最后在稳定画面之上增加截图导出与截图回归。

**Tech Stack:** Rust 2021, H7CAD workspace, `pid-parse`, `image`, 现有 `scene`/`app::update`/`command_line` 系统，crate 内真实样本测试，必要时用 `chrome-devtools`/桌面自动化做第二层截图验证。

---

## Feature Scope

验收样本固定为：

- `D:\work\plant-code\cad\pid-parse\test-file\工艺管道及仪表流程-1.pid`

最终必须满足：

1. 该 `.pid` 文件可在 H7CAD 中稳定打开
2. 图面显示接近原始 P&ID 外观，而不是仅有稀疏调试图元
3. 用户可通过命令导出 PNG 截图
4. 项目内有自动化截图验证链路，可用于确认显示并防回退

## Current Findings

- `src/io/pid_import.rs` 已有真实样本测试入口：`open_pid_real_sample_builds_layout_when_sample_present`
- `src/io/pid_import.rs::open_pid()` 会：
  - `parse_package`
  - `merge_publish_sidecars`
  - `derive_layout`
  - `pid_document_to_bundle`
  - `cache_package`
- `src/app/update.rs::Message::FileOpened` 在打开 `OpenedDocument::Pid(bundle)` 后会：
  - `tab_mode = Pid`
  - `scene.document = compat_preview`
  - `scene.set_native_doc(Some(bundle.native_preview))`
  - `scene.fit_all()`
- 代码库里还没有现成的 PID 视图截图命令
- 代码库里已有 `image` 依赖与图像读写路径，可作为 PNG 导出基础
- 当前最大风险不是“打不开”，而是“打开后显示质量不足以接近原始 P&ID 外观”

## Recommended Delivery Order

1. 先做真实样本基线测试
2. 再修显示质量
3. 再做 `PIDSHOT` 命令
4. 最后做自动化截图回归

不要先做截图命令。否则只会把错误画面稳定导出来。

---

## Task 1: Add a real-sample acceptance baseline for `工艺管道及仪表流程-1.pid`

**Files:**
- Modify: `src/io/pid_import.rs:3632-3728`
- Test: `src/io/pid_import.rs`

**Step 1: Write the failing real-sample test**

在现有 `real_sample_pid_path()` 与 `open_pid_real_sample_builds_layout_when_sample_present()` 旁边，新增一个明确绑定该文件名的测试。  
最小建议测试：

```rust
#[test]
fn open_target_pid_sample_builds_dense_preview() {
    let path = std::path::PathBuf::from(
        r"D:\work\plant-code\cad\pid-parse\test-file\工艺管道及仪表流程-1.pid"
    );
    if !path.exists() {
        eprintln!("SKIP: target pid sample not found");
        return;
    }

    let bundle = open_pid(&path).expect("open target pid sample");
    let layout = bundle.pid_doc.layout.as_ref().expect("layout should exist");
    assert!(layout.items.len() >= 20, "expected dense layout items");
    assert!(layout.segments.len() >= 10, "expected dense layout segments");
    assert!(bundle.summary.object_count >= 10, "expected visible objects");
}
```

阈值不要一开始定太激进；先把“明显不是空预览”的基线立起来。

**Step 2: Run the new test to verify current behavior**

Run:

```bash
cargo test open_target_pid_sample_builds_dense_preview -- --nocapture
```

Expected:

- 如果当前已经通过，记录当前计数作为后续显示优化基线
- 如果失败，保留失败输出，作为后续 Task 2/3 的输入

**Step 3: Add optional diagnostics if needed**

若仅靠断言不足以定位问题，则在测试中临时打印：

- `summary.object_count`
- `summary.relationship_count`
- `layout.items.len()`
- `layout.segments.len()`
- fallback 相关数量（若可从 preview/index 中拿到）

不要把临时 `eprintln!` 漫无边际铺开，只打印与显示密度相关的核心计数。

**Step 4: Re-run the targeted test**

Run:

```bash
cargo test open_target_pid_sample_builds_dense_preview -- --nocapture
```

Expected: 获得一份稳定可复现的真实样本基线。

**Step 5: Commit**

```bash
git add src/io/pid_import.rs
git commit -m "test: add target pid sample acceptance baseline"
```

---

## Task 2: Diagnose why the real sample is not “close to original P&ID”

**Files:**
- Modify: `src/io/pid_import.rs`
- Check: `src/app/update.rs:354-391`
- Check: `src/scene/mod.rs:2972-2989`
- Test: `src/io/pid_import.rs`

**Step 1: Write one failing test that captures the most visible deficiency**

从以下几个方向中只选一个最关键的先测试：

1. 主要 layout item 没进入 native preview
2. segments 太少 / 连接关系没有画出来
3. text/symbol/process point 变成了过于简陋的占位图元
4. `fit_all()` 后图面仍然落在不可视区域

建议优先选 1 或 2，因为它们最影响“像不像原图”。

示例（如果可统计预览实体）：

```rust
#[test]
fn target_pid_preview_contains_lines_text_and_markers() {
    let path = std::path::PathBuf::from(
        r"D:\work\plant-code\cad\pid-parse\test-file\工艺管道及仪表流程-1.pid"
    );
    if !path.exists() {
        eprintln!("SKIP: target pid sample not found");
        return;
    }

    let bundle = open_pid(&path).expect("open target pid sample");
    let entity_count = bundle.native_preview.entities().count();
    assert!(entity_count >= 30, "preview should not be sparse");
}
```

**Step 2: Run the test and inspect the failing symptom**

Run:

```bash
cargo test target_pid_preview_contains_ -- --nocapture
```

Expected: 失败信息应能把问题收敛到 preview density / entity mapping / bounds 之一。

**Step 3: Write the minimal implementation in the narrowest layer**

优先修改 `src/io/pid_import.rs`，因为显示质量大概率源于这里的 preview 构建，而不是 UI 层。

优先检查这些点：

- `pid_document_to_bundle()`
- layout item 到 native entity 的映射是否丢失了文本、节点、线段
- fallback 布局对象是否压过了真实 layout
- process point / symbol / stream / relationship 面板是否占据过多视觉空间

若问题明显是打开后 camera/fit 的问题，再转到：

- `src/app/update.rs`
- `src/scene/mod.rs::fit_all`

保持 YAGNI：先修最影响该样本观感的一个点，不一口气做全量美化。

**Step 4: Run the focused test again**

Run:

```bash
cargo test open_target_pid_sample_builds_dense_preview -- --nocapture
cargo test target_pid_preview_contains_ -- --nocapture
```

Expected: 至少证明真实样本比当前更完整、更密集、更接近原图。

**Step 5: Commit**

```bash
git add src/io/pid_import.rs src/app/update.rs src/scene/mod.rs
git commit -m "fix: improve target pid sample preview fidelity"
```

只提交真正改过的文件。

---

## Task 3: Ensure the opened PID auto-fits into a useful viewport

**Files:**
- Modify: `src/app/update.rs:354-391`
- Modify: `src/scene/mod.rs:2972-2989`
- Test: `src/app/update.rs`

**Step 1: Write a failing test for open-and-fit behavior**

已有打开路径在 `Message::FileOpened` 后调用 `scene.fit_all()`。  
增加一个应用层测试，证明打开 PID 后不会出现“数据有了但视图没居中/没缩放”的情况。

测试方向：

- 打开 PID 后 camera bounds 覆盖主要实体
- `fit_all()` 后 scene 不保持默认空视图

如果当前测试设施不方便直接检查 camera，可退而求其次检查：

- `scene.native_store.is_some()`
- `scene.document.entities().count() > 0`
- 打开后没有再丢失 layout/current state

**Step 2: Run the new test**

Run:

```bash
cargo test file_opened_pid -- --nocapture
```

Expected: 如果失败，定位是 `FileOpened` 分支还是 `fit_all` 本身。

**Step 3: Write the minimal implementation**

优先检查：

```rust
self.tabs[i].scene.document = compat_preview;
self.tabs[i].scene.set_native_doc(Some(bundle.native_preview));
self.tabs[i].scene.fit_all();
```

若 PID 特殊场景需要额外处理，可在 `OpenedDocument::Pid(bundle)` 分支中补：

- 更早/更晚的 `fit_all()`
- 打开 PID 后清理旧选中与视图残留
- PID tab 初始 layout/state 的专门设置

不要在 CAD 和 PID 共用路径里硬编码过多 PID 特例，除非确有必要。

**Step 4: Re-run tests**

Run:

```bash
cargo test file_opened_pid -- --nocapture
cargo test open_target_pid_sample_builds_dense_preview -- --nocapture
```

Expected: 打开后视图初始化行为稳定。

**Step 5: Commit**

```bash
git add src/app/update.rs src/scene/mod.rs
git commit -m "fix: auto-fit opened pid sample into viewport"
```

---

## Task 4: Add the `PIDSHOT` command for PNG export

**Files:**
- Modify: `src/app/commands.rs`
- Modify: `src/io/mod.rs` or a new dedicated screenshot helper module
- Create: `src/io/pid_screenshot.rs` (recommended if logic exceeds ~60 lines)
- Test: command tests or helper tests

**Step 1: Write the failing test for PNG export**

如果现有命令测试基础允许，先加一个最小 helper 测试，不要一开始就强耦合 UI：

```rust
#[test]
fn export_pid_preview_png_writes_file() {
    let out = std::env::temp_dir().join("h7cad-pidshot-test.png");
    let path = std::path::PathBuf::from(
        r"D:\work\plant-code\cad\pid-parse\test-file\工艺管道及仪表流程-1.pid"
    );
    if !path.exists() {
        eprintln!("SKIP: target pid sample not found");
        return;
    }

    let bundle = crate::io::pid_import::open_pid(&path).expect("open pid");
    export_pid_preview_png(&bundle.native_preview, &out).expect("export png");
    assert!(out.exists());
}
```

这里的关键是：**截图导出能力先做成 helper，再挂命令。**

**Step 2: Run the failing test**

Run:

```bash
cargo test export_pid_preview_png_writes_file -- --nocapture
```

Expected: 先失败，因为 helper / command 尚不存在。

**Step 3: Write minimal implementation**

推荐拆两层：

1. helper 层：`export_pid_preview_png(...)`
2. command 层：`PIDSHOT <path.png>`

最小命令契约：

- 只允许活动 PID tab
- 目标路径必须以 `.png` 结尾
- 成功输出：`PIDSHOT  saved screenshot to ...`
- 失败输出：`PIDSHOT: ...`

如果没有现成 framebuffer 抓图能力，第一版可以实现为：

- 从当前 PID preview/native doc 生成一个**确定性的 2D 导出图**到 PNG
- 不强求“截整个桌面窗口像素”，但要确保是用户可确认的 PNG 输出

这是本计划里最重要的 YAGNI 决策：  
先做 **稳定、可回归的 PID 图面 PNG 导出**，不要一开始就做复杂 GPU 读回。

**Step 4: Re-run the test**

Run:

```bash
cargo test export_pid_preview_png_writes_file -- --nocapture
```

Expected: PNG 文件成功落盘。

**Step 5: Commit**

```bash
git add src/app/commands.rs src/io/mod.rs src/io/pid_screenshot.rs
git commit -m "feat: add pid screenshot export command"
```

仅在新 helper 被创建时提交新文件。

---

## Task 5: Add a deterministic screenshot regression for the target sample

**Files:**
- Create: `tests/pid_screenshot_regression.rs` or inline test in `src/io/pid_import.rs`
- Create: `tests/fixtures/pidshots/工艺管道及仪表流程-1.png` (if snapshot baseline is committed)
- Modify: screenshot helper module

**Step 1: Write the failing regression test**

推荐先做 **进程内导图回归**，不要先做窗口级 UI 自动化。

测试结构建议：

```rust
#[test]
fn target_pid_sample_screenshot_matches_baseline() {
    let sample = std::path::PathBuf::from(
        r"D:\work\plant-code\cad\pid-parse\test-file\工艺管道及仪表流程-1.pid"
    );
    if !sample.exists() {
        eprintln!("SKIP: target pid sample not found");
        return;
    }

    let actual = std::env::temp_dir().join("target-pid-actual.png");
    export_target_pid_sample_png(&sample, &actual).expect("export actual");
    assert_png_close_to_baseline(&actual, "tests/fixtures/pidshots/工艺管道及仪表流程-1.png");
}
```

第一版可以不用做复杂 perceptual diff。  
若图面是确定性的，可先：

- 比较尺寸
- 比较像素差阈值
- 或比较少量统计特征（非空像素占比、bounding box、颜色计数）

**Step 2: Run the regression test**

Run:

```bash
cargo test target_pid_sample_screenshot_matches_baseline -- --nocapture
```

Expected: 初次失败，因为 baseline 或对比 helper 还没准备好。

**Step 3: Write the minimal implementation**

实现内容：

- 固定输出尺寸（例如 `1600x900`）
- 固定视图范围 / fit 逻辑
- 固定配色/线宽策略
- 添加一个简单且稳定的 PNG 对比 helper

若像素级完全一致过于脆弱，则采用阈值比较：

- 总差异像素数 < N
- 或差异比例 < 1%

**Step 4: Re-run the regression test**

Run:

```bash
cargo test target_pid_sample_screenshot_matches_baseline -- --nocapture
```

Expected: PASS，得到稳定截图基线。

**Step 5: Commit**

```bash
git add tests/pid_screenshot_regression.rs tests/fixtures/pidshots/工艺管道及仪表流程-1.png src/io/pid_screenshot.rs
git commit -m "test: add pid screenshot regression baseline"
```

---

## Task 6: Add a second-layer automated confirmation path (optional but planned)

**Files:**
- Create: `docs/plans/...` notes only if needed
- Create: test automation script if repository already has a UI automation pattern

**Step 1: Decide if UI-level automation is actually needed now**

只有在进程内导图回归已经稳定通过后，才做这一层。

目标不是替代 Task 5，而是补一条“像用户一样打开并截图”的确认路径。

**Step 2: If needed, write the smallest failing automation**

流程：

1. 启动 H7CAD
2. 打开目标 PID
3. 等待视图稳定
4. 触发 `PIDSHOT`
5. 检查 PNG 是否生成

优先验证命令链路，而不是做复杂视觉 diff。

**Step 3: Implement the automation**

只在仓库现有自动化基础存在时做；若没有，则把这一层明确列为后续阶段，不要强行加入本轮。

**Step 4: Run the automation**

记录：

- 打开成功
- 命令成功
- PNG 成功

**Step 5: Commit**

```bash
git add <automation files>
git commit -m "test: automate pid screenshot confirmation flow"
```

---

## Validator Sequence

每个阶段完成后至少运行以下命令：

```bash
cargo test open_target_pid_sample_builds_dense_preview -- --nocapture
cargo test pid_import -- --nocapture
cargo check
```

在截图功能完成后再增加：

```bash
cargo test export_pid_preview_png_writes_file -- --nocapture
cargo test target_pid_sample_screenshot_matches_baseline -- --nocapture
```

如果命令分发有改动，追加：

```bash
cargo test commands -- --nocapture
```

如果没有现成命令测试，就用 `cargo check` + helper tests 作为最低验证门槛。

## Recommended Commit Order

1. `test: add target pid sample acceptance baseline`
2. `fix: improve target pid sample preview fidelity`
3. `fix: auto-fit opened pid sample into viewport`
4. `feat: add pid screenshot export command`
5. `test: add pid screenshot regression baseline`
6. `test: automate pid screenshot confirmation flow`

## Explicit Non-Goals

本计划不做：

1. 通用所有 PID 文件的完美视觉还原
2. GPU 级高保真屏幕截图系统
3. PID 编辑器 GUI
4. DWG / DXF 路径优化
5. 任意格式图片导出（第一版只做 PNG）

## Success Checklist

- [ ] `工艺管道及仪表流程-1.pid` 可稳定打开
- [ ] 图面显示接近原始 P&ID 外观
- [ ] `PIDSHOT <path.png>` 可导出 PNG
- [ ] 自动化截图回归可验证该样本
- [ ] `cargo check` 通过
- [ ] 相关 targeted tests 通过

Plan complete and saved to `docs/plans/2026-04-21-pid-real-sample-display-and-screenshot-plan.md`. Two execution options:

**1. Subagent-Driven (this session)** - I dispatch fresh subagent per task, review between tasks, fast iteration

**2. Parallel Session (separate)** - Open new session with executing-plans, batch execution with checkpoints

Which approach?
