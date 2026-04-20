# `pid_writer_validate --edit` 模拟编辑模式落地计划

> 起稿：2026-04-19  
> 依赖：`pid-parse` v0.4.1 metadata_helpers + `pid_writer_validate` CLI（同日早些时候完成）
>
> **目标**：把 `pid_writer_validate` 从只能"原样 round-trip"扩展到"模拟 metadata 编辑后再 round-trip 验证"。
>
> ```
> pid_writer_validate input.pid \
>     --edit SP_DRAWINGNUMBER=NEW-001 \
>     --edit SP_REVISION=2 \
>     --general-edit FilePath=D:/issued/x.pid \
>     --out edited.pid --keep
> ```
>
> 让用户能在不打开 H7CAD GUI 的情况下，用同一个 CLI 工具完整验证：
> 1. metadata_helpers 的 splice 行为正确
> 2. PidWriter 的写出对**未编辑的流**字节级保真
> 3. 输出文件能被 SmartPlant 重新打开（人工二次确认）

---

## 现状盘点

* `pid_writer_validate` 已能：parse → write (passthrough) → re-parse → per-stream byte diff
* `pid_parse::writer::set_drawing_attribute` / `set_element_text` 已能：byte-level XML splice
* 缺：把"上述 splice 应用到 PidPackage" 然后跑 round-trip 验证的胶水

## CLI 扩展

```
pid_writer_validate <input.pid> [既有 flags…]
                                [--edit ATTR=VALUE]+ 
                                [--general-edit ELEMENT=VALUE]+
```

- `--edit` 可重复，每次替换 Drawing 流里一个属性
- `--general-edit` 可重复，每次替换 General 流里一个 leaf-text 元素
- ATTR / ELEMENT / VALUE 通过 `=` 分隔；VALUE 含 `=` 时取**第一个**等号之前为 key
- 任一 edit 解析失败 / metadata_helpers 拒绝 → exit 2，不写出文件

## 报告语义升级

新增"per-stream 状态"三态（替代之前的 OK / DIFF 二态）：

| 状态 | 含义 |
|---|---|
| `OK` | 流未被编辑请求覆盖；source 与 round-trip 字节完全相等 |
| `EDITED` | 流在编辑请求覆盖范围内；round-trip 字节 == 应用 edit 后的 expected |
| `DIFF` | 意外差异：流没被请求编辑但字节变了，或编辑后字节与预期不符 |

`ok = no DIFF`（key set 也无差异）。

报告字段补 `edited` 计数 + `edits_applied: Vec<EditOp>`。

## 实施步骤

### Step 1 · CLI 参数解析扩展

`parse_args`：
- 收 `--edit` / `--general-edit` 多次到 `Vec<EditOp>`
- `EditOp { kind: Drawing|General, key: String, value: String }`
- key/value 切分错误 → 返回 `argument error: --edit must be ATTR=VALUE`

### Step 2 · 把 edits 串到 round-trip

修改 `run_validate` 接受 `edits: &[EditOp]`：
```rust
fn run_validate(input, output, max_diff_bytes, edits) -> Result<ValidateReport, ValidateError>
```

新流程：
1. parse_package(input) → `original`
2. clone `original` → `edited`，对每个 EditOp：
   - Drawing：取 `/TaggedTxtData/Drawing` 字节 → utf8 → `set_drawing_attribute` → replace_stream
   - General：取 `/TaggedTxtData/General` 字节 → utf8 → `set_element_text` → replace_stream
   - 任意失败：`ValidateError::Edit(msg)`（新错误类型）
3. PidWriter::write_to(&edited, &WritePlan::default(), output)
4. parse_package(output) → `roundtrip`
5. 比 `edited.streams` vs `roundtrip.streams`（这是关键变化：以"应用过 edit 的 expected"为对照基准，而非原始 source）
6. 同时报告"哪些流被显式 edit 标记"，让用户区分 EDITED vs OK vs DIFF

### Step 3 · 报告 + 输出

`StreamStatus` enum 加进去：
```rust
#[derive(Serialize)]
enum StreamStatus { Ok, Edited, Diff }

struct StreamRecord { path: String, status: StreamStatus, source_len: Option<usize>, expected_len: usize, roundtrip_len: usize }
```

human format 在 per-stream 段每行带状态前缀：
```
OK     /TaggedTxtData/General        133 B
EDITED /TaggedTxtData/Drawing         (101 B → 105 B)  attr=SP_DRAWINGNUMBER
DIFF   /SomeOther/Stream              source=128 B roundtrip=129 B first diff @ 42
```

JSON 加 `edits_applied` 数组。

### Step 4 · 测试

`tests/writer_validate_cli.rs` 新增：
- `validate_with_edit_drawing_attribute_passes`：fixture → 调 `--edit SP_DRAWINGNUMBER=NEW-001 --json` → JSON 解析后 `ok=true`、`mismatched=0`、有 `EDITED` 状态的流出现
- `validate_with_general_edit_passes`：同上但走 `--general-edit FilePath=D:/x.pid`
- `validate_edit_with_unknown_attr_exits_with_edit_error`：`--edit SP_NOSUCH=v` → exit 2 + stderr 含 "metadata edit failed" / "AttributeNotFound"
- `validate_edit_argument_malformed_exits_with_argument_error`：`--edit foo` 无 `=` → exit 1 + stderr 含 "ATTR=VALUE"

`run_validate` 单测（lib 端不可达，因为 binary module；放在 binary 内部 `#[cfg(test)] mod tests`）：
- `apply_edits_returns_edit_error_on_unknown_attribute`

### Step 5 · 落地

- `cargo build --bin pid_writer_validate` 全绿
- `cargo test --test writer_validate_cli` 全绿
- `cargo test` 全集（fixture-dependent 除外）保持原状
- `pid-parse/CHANGELOG.md` 在 `pid_writer_validate` 行后追加 `--edit` / `--general-edit` 描述
- 不 bump 版本

## 公共 API 增量

### pid-parse binary
- `EditOp { kind, key, value }`、`StreamStatus`、`StreamRecord` 暴露在 binary module（用于 tests + Serialize）
- `run_validate(input, output, max_diff_bytes, edits)` 签名扩展

### pid-parse lib
- 无改动

## 显式不做（留给下下迭代）

1. **目录批量**（`<dir>` 递归）：第一版只单文件
2. **`--read-only` flag** 验证模式跳过 write 步骤：用户场景未明
3. **edit 顺序约束**：第一版按 args 顺序应用；如果两个 edit 触碰同 attr，后一个覆盖前一个

## 风险

- **R1**：`--edit SP_X=val=other`：value 含 `=` → 用 `splitn(2, '=')` 让 value 拿全部剩余
- **R2**：edit 触发 `MetadataEditError::DuplicateAttribute` → 透传错误，exit 2，明确告诉用户范围太宽
- **R3**：edit 把流改大或改小 → write_to 写新长度；roundtrip 解析后应仍然 utf8。第一版假设 ASCII 替换；非 ASCII 留给下一迭代

## 工作量预估

- Step 1：10 min
- Step 2：20 min
- Step 3：20 min
- Step 4：20 min
- Step 5：10 min

合计 ~80 min。
