# H7CAD CLI 批处理 PDF 导出（三十六轮）

> **起稿**：2026-04-25（第三十六轮）
> **前置**：三十二-三十五轮 PDF 导出已做到文字 / hatch / image /
> 原生曲线 / 原生样条 / 对话框的完整能力。所有能力目前只能通过 GUI
> 触发——对自动化管线 / CI / 批处理不友好。
> **目标**：给 `h7cad` 可执行程序加一个 headless CLI 路径，
> `h7cad drawing.dxf --export-pdf out.pdf` 就能无 GUI 转成 PDF，
> 退出码 0 / 1 对应成功 / 失败。

---

## 1. 现状

`src/main.rs`：

```rust
fn main() -> iced::Result {
    app::run()
}
```

`app::run()` 内部读 `std::env::args().nth(1)` 作为要在 GUI 里打开的
文件路径。完全没有 headless / batch 路径，即使只想做 DXF→PDF 转换
也必须启动窗口。

---

## 2. 范围

| 纳入 | 优先级 | 预估 |
|------|-------|------|
| T1 `main.rs` 加 CLI 前置处理：识别 `--export-pdf <out>` flag | P0 | 0.2 h |
| T2 `src/cli.rs` 新模块：`fn run_batch_export(input, output) -> Result` | P0 | 0.8 h |
| T3 Headless 管线：open DXF → 构 Scene → `export_pdf_full` → 写文件 | P0 | 1.0 h |
| T4 3 个集成测试（成功 / 缺输入 / 无写权限模拟） | P0 | 0.4 h |
| T5 `--help` 文本 + README 段落（简短） | P1 | 0.2 h |
| T6 CHANGELOG + commit + push | P0 | 0.1 h |

**不纳入**：
- `--export-svg`（对称但下一轮做，避免本轮过大）
- 任何外部依赖（`clap` 等）——就手写 args 解析，保持零新依赖
- 多 layout 批处理（也延到下一轮）

---

## 3. 设计

### 3.1 main.rs 前置分派

```rust
fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if let Some(cli_args) = cli::parse_batch_args(&args[1..]) {
        match cli::run_batch_export(cli_args) {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("h7cad: {e}");
                std::process::ExitCode::from(1)
            }
        }
    } else {
        // GUI 路径保持原样（iced::Result → ExitCode）
        match app::run() {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("h7cad (GUI): {e:?}");
                std::process::ExitCode::from(1)
            }
        }
    }
}
```

### 3.2 CLI 约定

```
h7cad INPUT.dxf --export-pdf OUTPUT.pdf
h7cad INPUT.dxf --export-pdf            # 自动推导 OUTPUT = INPUT + ".pdf"
h7cad --help
```

- `INPUT.dxf` 必须是存在的 DXF 文件；后续扩展可以支持 DWG
- `--export-pdf` 后面跟 PDF 路径（可选）

### 3.3 Headless 管线（cli::run_batch_export）

1. 读文件 bytes → `h7cad_native_dxf::read_dxf_bytes` → `native_doc`
2. 创建 `Scene::new(native_doc_to_acadrust(&native_doc))`（不需要 GPU）
3. 绑定 `scene.set_native_document(native_doc)` 让 native 路径可用
4. 取纸张尺寸（走和 GUI 导出一样的 `paper_limits` / fallback）
5. 计算 offset / rotation（和 `PlotExportPath` 逻辑对齐，不含
   PlotSettings 的 centering——CLI 约定用默认配置）
6. `export_pdf_full(&scene.entity_wires(), &scene.hatches,
   scene.native_doc(), …, &PdfExportOptions::default())`

Scene 初始化不触发 GPU 上下文的部分已在多处被 `#[cfg(test)]` 的
`display_scene()` 验证过（R31-R35 全部 fixture 都这么跑），沿用。

---

## 4. 测试

### 集成测试位置

`tests/cli_batch_export.rs`（新文件，使用 `std::process::Command` 跑
build 出来的 `h7cad` 二进制）。

### 测试用例

- `cli_batch_export_writes_pdf_for_minimal_dxf`：用 `tempfile` 生成
  一个最小 DXF（只含一条 LINE），跑 `h7cad input.dxf --export-pdf
  out.pdf`，断言 `out.pdf` 存在且以 `%PDF-` 开头
- `cli_help_flag_returns_zero`：`h7cad --help` 退出码 0，stdout 非空
- `cli_missing_input_returns_nonzero`：`h7cad nonexistent.dxf
  --export-pdf out.pdf` 退出码非 0，stderr 含 "cannot open" 字样

### 单元测试

`cli::parse_batch_args` 是纯函数，可在 lib 层加普通单元测试
（不需要跑二进制）。

---

## 5. 验收

```bash
cargo check -p H7CAD                  # 零新 warning
cargo test --bin H7CAD                # 399 → 402+ 全绿（+ parse_batch_args unit tests）
cargo test --test cli_batch_export    # 3 / 3
cargo build -p H7CAD --release        # 二进制产出, 手工跑 CLI 验证
```

---

## 6. 状态

- [x] 计划定稿（2026-04-25）
- [ ] T1 main.rs 前置分派
- [ ] T2 src/cli.rs 模块
- [ ] T3 headless 管线
- [ ] T4 集成测试
- [ ] T5 --help 文本
- [ ] T6 CHANGELOG + commit + push
