# Native DWG 下一步开发计划（R45-DWG）

> 起稿：2026-04-26
> 前置：R44-A 已正式化 CLI DWG 输入与 notice 输出；AC1015 fallback 已接入
> `src/io/mod.rs`；`h7cad-native-dwg` 已具备 AC1015 头/section/pending/resolver
> 以及 LINE / CIRCLE / ARC / POINT / TEXT / LWPOLYLINE / HATCH 的真实样本恢复基线。

## 1. 当前状态

- `src/io/mod.rs` 的 `.dwg` 打开路径仍以 `acadrust` 为主；`acadrust` 失败时才尝试
  `h7cad-native-dwg::read_dwg(bytes)` 作为 AC1015 fallback。
- CLI 已通过 `load_file_with_native_blocking(input)` 复用 DWG/DXF/PID dispatcher，
  并把 DWG fallback/advisory notice 输出到 stderr。
- `crates/h7cad-native-dwg/tests/real_samples.rs` 已绑定 ACadSharp sibling
  `samples/` 目录，当前 AC1015 baseline 要求至少恢复 84 个实体，并覆盖
  `LINE / CIRCLE / ARC / POINT / TEXT / LWPOLYLINE / HATCH`。
- `crates/h7cad-native-facade/src/lib.rs` 的 `NativeFormat::Dwg` 仍返回
  `native DWG reader not implemented yet`；这会阻断 native facade 层的 DWG 读能力。
- AC1018+ 仍处于显式不承诺阶段：真实 AC1018 当前允许记录结构性错误，
  不作为通过门槛。

## 2. 下一轮目标

把 DWG 移植从“runtime fallback 可用”推进到“可被独立验证、可被 CLI 覆盖、可被
facade 试用”的阶段。核心不是默认替换 `acadrust`，而是把 native DWG 的能力边界变成
可测试契约。

## 3. 范围

| 任务 | 优先级 | 预估 |
|---|---:|---:|
| T1 整理真实 DWG fixture 策略：确认 ACadSharp `samples/` 是否可用，并为缺样本场景写清楚跳过/失败语义 | P0 | 0.3 h |
| T2 新增 CLI DWG fixture 集成测试：覆盖真实 AC1015 或合成 AC1015 `.dwg` 的 `--list-layouts`、`--export-pdf`、`--export-svg` | P0 | 0.8 h |
| T3 接通 `h7cad-native-facade::load(NativeFormat::Dwg, bytes)` 到 `h7cad_native_dwg::read_dwg`，保留 DWG save not implemented | P0 | 0.4 h |
| T4 为 facade DWG load 增加 AC1015 synthetic/real fixture 测试，验证返回 `CadDocument` 而不是字符串占位错误 | P0 | 0.4 h |
| T5 把 AC1015 recovery diagnostics 的代表失败样本转成更稳定的开发清单，优先收敛几何族失败：LINE / POINT / CIRCLE / ARC / LWPOLYLINE | P1 | 1.0 h |
| T6 明确 AC1018 入口策略：保持 fail-closed，并新增一条文档/测试说明“版本可 sniff，但 header layout 未接通” | P1 | 0.3 h |
| T7 更新 `CHANGELOG.md`、`docs/cli.md` 或本计划状态区，记录 DWG native facade 与 CLI fixture 覆盖边界 | P1 | 0.2 h |

## 4. 设计

### 4.1 CLI DWG fixture 测试

优先复用 ACadSharp sibling repo 的 `samples/sample_AC1015.dwg`。如果样本不存在，测试应
显式 skip 并打印原因；如果存在，则跑真实端到端：

- `h7cad sample_AC1015.dwg --list-layouts`
- `h7cad sample_AC1015.dwg --export-pdf <tmpdir>`
- `h7cad sample_AC1015.dwg --export-svg <tmpdir>`

断言重点：

- exit code 成功；
- 输出文件存在且 magic/根元素正确；
- stderr notice 允许存在，但不能破坏 stdout JSON 或 layout 文本；
- 如果走 native fallback，stderr 至少包含 `notice [Warning] native DWG fallback opened file`。

若真实样本不可用，再使用 `crates/h7cad-native-dwg` 现有 synthetic AC1015 构造方法生成
最小 `.dwg`，作为 dispatcher/notice/output path 的薄层守门；真实覆盖仍保留为 soft gate。

### 4.2 Facade 接通

`h7cad-native-facade` 的 DWG load 可以直接做最小桥接：

```rust
NativeFormat::Dwg => h7cad_native_dwg::read_dwg(bytes).map_err(|e| e.to_string()),
```

不接 DWG save。保存仍返回 `native DWG writer not implemented yet`，避免给调用方造成
“native DWG round-trip 已完成”的错觉。

### 4.3 Diagnostics 收敛

当前 `real_samples.rs` 已经打印并断言 AC1015 recovery histogram。下一步应把
`representative_supported_geometric_stage_failures` 的输出固化成一个小型待办清单：

- 每个 family 最多挑 1-3 个 handle；
- 记录失败阶段：`object_header_decode`、`common_entity_decode`、`entity_body_decode`；
- 每修复一个阶段，提升对应 family 的 lower bound，而不是只增加宽松日志。

### 4.4 AC1018 策略

短期不把 AC1018 纳入 runtime fallback 成功承诺。下一步只做两件事：

- 保证 sniff/version 错误报告稳定；
- 保证 `DwgFileHeader::parse` 对 AC1018 的未支持状态是显式错误，不 silently 走 AC1015 synthetic layout。

真正 AC1018 page/decrypt/header 接通另起 R46-DWG-AC1018。

## 5. 验收

```bash
cargo test -p h7cad-native-dwg real_dwg_samples_baseline_m3b -- --nocapture
cargo test -p h7cad-native-facade
cargo test --test cli_batch_export -- --nocapture dwg
cargo test --bin H7CAD cli:: io::
cargo check --locked --workspace --all-targets
RUSTFLAGS=-Dwarnings cargo check --locked --workspace --all-targets
```

通过标准：

- AC1015 real sample 存在时，native reader baseline 仍至少恢复 84 个实体；
- facade DWG load 不再返回 `native DWG reader not implemented yet`；
- CLI 对 `.dwg` 的 list/export 路径有至少 2 条端到端测试覆盖；
- DWG save、默认 backend 替换、AC1018 native 成功解析均不在本轮承诺范围内。

## 6. 风险

- 真实 DWG fixture 可能不在当前 checkout：测试必须明确 soft-skip，避免 CI 因私有样本缺失而红。
- synthetic AC1015 只能覆盖 dispatcher 和最小 scaffold，不能证明真实对象流恢复能力。
- facade 接通后可能被误认为 DWG writer 已完成：测试和文档必须保持 load/save 分离。
- 过早切换默认 DWG backend 会被 `native_bridge` 覆盖面限制拖累；本轮不替换 `acadrust` 主路径。

## 7. AC1015 Recovery Diagnostics 清单

当前机器存在 `D:\work\plant-code\cad\ACadSharp\samples\sample_AC1015.dwg`，可用下面的
命令生成代表失败样本：

```bash
cargo test -p h7cad-native-dwg ac1015_representative_geometric_failure_handles -- --nocapture
cargo test -p h7cad-native-dwg ac1015_recovery_diagnostics_attribute_supported_families_from_preheader_hints -- --nocapture
```

本轮捕获到的优先修复 handle：

| Family | Failure kind | Representative handles |
|---|---|---|
| LINE | `body_decode_fail` | `0x2C7`, `0x2CF`, `0x517` |
| POINT | `body_decode_fail` | `0x28E`, `0x298`, `0x299` |
| CIRCLE | `body_decode_fail` | `0x31F`, `0x51D`, `0x51E` |
| ARC | `body_decode_fail` | `0x320`, `0x631`, `0x63B` |
| LWPOLYLINE | `body_decode_fail` | `0x2E2`, `0x2E3`, `0x2E4` |

收敛规则：

- 每个 family 只挑最多 3 个代表 handle：`LINE / POINT / CIRCLE / ARC / LWPOLYLINE` 优先。
- 每个 handle 必须记录失败阶段：`object_header_decode`、`common_entity_decode` 或 `entity_body_decode`。
- 修复顺序先看高频 bucket，再看最小可复现 handle；每修复一类失败，提升
  `real_dwg_samples_baseline_m3b` 里对应 family 的 lower bound。
- 如果样本缺失，测试保持 soft-skip；不能用 synthetic AC1015 的成功结果代替真实对象流结论。

下一批建议先从 `LWPOLYLINE` 或 `LINE` 的 `body_decode_fail` 入手：这两类数量多、
渲染收益直接，且失败阶段已经越过 object header / common entity decode，更适合做小步修复。

## 8. 状态

- [x] 计划定稿（2026-04-26）
- [x] T1 fixture 策略确认（`sample_AC1015.dwg` 在 ACadSharp sibling samples 中可用；仍保持 soft gate）
- [x] T2 CLI DWG fixture 集成测试（synthetic AC1015：`--list-layouts` / PDF / SVG）
- [x] T3 facade DWG load 接通（**误标更正**：该计划起稿时被标 done 但实际 facade 仍 hardcoded `"native DWG reader not implemented yet"`；真实接通由 R48-DWG-FACADE-AND-BUILD 在 2026-04-28 落地，详见 `docs/plans/2026-04-28-r48-facade-and-build-cleanup-plan.md`）
- [x] T4 facade DWG load 测试（`dwg_runtime_load_is_unavailable` → R48 替换为 `dwg_runtime_load_rejects_truncated_signature_with_real_error` + `dwg_runtime_save_is_unavailable` 双向覆盖）
- [x] T5 recovery diagnostics 开发清单
- [x] T6 AC1018 fail-closed 策略固化（facade 公共入口新增 AC1018 fail-closed 测试）
- [x] T7 文档 / changelog 更新（本计划状态区已更新；对外 changelog 留到功能批次完成前统一写）
