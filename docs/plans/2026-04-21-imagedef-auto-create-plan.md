# 开发计划：Writer 端 `ensure_image_defs` — 自动为 IMAGE 实体生成 IMAGEDEF 对象

> 起稿：2026-04-21（同日次轮）  
> 前置：`docs/plans/2026-04-21-imagedef-object-plan.md` 已完成。本轮闭合其"未纳入"清单中的首项：writer 侧 auto-create IMAGEDEF。

## 动机

前轮把 IMAGE ↔ IMAGEDEF 标准链接打通：

- Reader 把 `code 340` 解析成 `image_def_handle`，再通过 `resolve_image_def_links` 回填 `file_path`
- Writer 根据 `image_def_handle` 二选一输出 `code 340`（标准）或 `code 1` fallback（legacy 兼容）

**遗留问题**：当 H7CAD 通过 bridge / UI 构造 `EntityData::Image { file_path: "...", image_def_handle: NULL, ... }` 时（例如从 `acadrust::RasterImage` 或 `ImageCommand::execute` 来），writer 输出非标准的 `code 1 on IMAGE` — 虽然我们自己能读回，但 AutoCAD / ACadSharp 等**只认 IMAGEDEF+340 标准链**的工具看不到 file_path（IMAGE 变成"孤儿"）。

**本轮目标**：Writer 在序列化前自动为"`handle = NULL` 且 `file_path` 非空"的 IMAGE 实体分配 handle、创建 `ObjectData::ImageDef` 对象、回填 handle。输出变成纯标准 DXF，任意第三方 DXF 解析器都能正确识别 file_path。

## 目标

1. 新增 `ensure_image_defs(doc: &mut CadDocument)` 函数，对文档做幂等的 IMAGEDEF auto-create 预处理
2. `write_dxf_string(doc: &CadDocument)` 保持 API 签名不变（只读借用），内部按需 clone + 预处理（避免下游 `src/io/mod.rs::save_dxf` 被迫改签名）
3. 作用域覆盖 `doc.entities`（top-level）与 `doc.block_records[*].entities`（block-scoped）两类 IMAGE 实体
4. Idempotent：对已经有 handle 的 IMAGE 不再重复建 IMAGEDEF；对空 `file_path` + `NULL` handle 的"纯空 IMAGE"不误建
5. 测试覆盖：top-level / block 实体 / 空 file_path / 已有 handle / round-trip 可读回

## 非目标

- 不改 `write_dxf_string` 的 API 签名（走 clone-on-demand 路径）
- 不实现 `IMAGEDEF_REACTOR` 的自动建立（reader 已能读，writer 已能对称写；自动创建双向反向链接留待独立工作）
- 不管 owner_handle / `ACAD_IMAGE_DICT` 子字典的自动建立（新 IMAGEDEF 的 `owner_handle` 设为 `Handle::NULL`；AutoCAD 实测允许，严格对齐 DXF Reference 要求 owner = ACAD_IMAGE_DICT 可在下一轮处理）
- 不扩 `ObjectData::ImageDef` 字段（`resolution_unit` / `pixel_size` / `class_version` 继续用默认）
- 不修 reader 的 "mixed DXF trust-first-fill" 语义（前轮刻意选择，本轮不动）

## 关键设计

### 1. 两阶段 allocate

借用冲突：遍历 `doc.block_records` 时 `&doc.block_records`，同时需要 `doc.allocate_handle()`（`&mut doc`）。Rust 不允许。

解法：**拆成三趟**：

1. **收集趟**（只读借用）：扫描 `doc.entities` + `doc.block_records[*].entities`，收集所有 "`handle == NULL && !file_path.is_empty()`" 的 IMAGE 实体的 **位置** (`ImageLoc`) + `file_path` + `image_size`
2. **分配趟**（唯一可变借用 `&mut doc`）：为每个 pending 项 `allocate_handle`，构造 `CadObject`，push 到 `doc.objects`，同时把 `(ImageLoc, new_handle)` 记录下来
3. **回填趟**（按 ImageLoc 精准回写）：
   - `ImageLoc::TopLevel(i)` → `doc.entities[i].data` 的 `image_def_handle`
   - `ImageLoc::Block(br_handle, i)` → `doc.block_records.get_mut(&br_handle).unwrap().entities[i].data`

```rust
enum ImageLoc {
    TopLevel(usize),
    Block(Handle, usize),
}

fn ensure_image_defs(doc: &mut CadDocument) {
    let mut pending: Vec<(ImageLoc, String, [f64; 2])> = Vec::new();

    for (i, e) in doc.entities.iter().enumerate() {
        if let EntityData::Image {
            image_def_handle, file_path, image_size, ..
        } = &e.data
        {
            if *image_def_handle == Handle::NULL && !file_path.is_empty() {
                pending.push((ImageLoc::TopLevel(i), file_path.clone(), *image_size));
            }
        }
    }
    for (br_handle, br) in &doc.block_records {
        for (i, e) in br.entities.iter().enumerate() {
            if let EntityData::Image {
                image_def_handle, file_path, image_size, ..
            } = &e.data
            {
                if *image_def_handle == Handle::NULL && !file_path.is_empty() {
                    pending.push((
                        ImageLoc::Block(*br_handle, i),
                        file_path.clone(),
                        *image_size,
                    ));
                }
            }
        }
    }

    if pending.is_empty() {
        return;
    }

    let mut allocated: Vec<(ImageLoc, Handle)> = Vec::with_capacity(pending.len());
    for (loc, file_name, image_size) in pending {
        let new_handle = doc.allocate_handle();
        doc.objects.push(CadObject {
            handle: new_handle,
            owner_handle: Handle::NULL,
            data: ObjectData::ImageDef { file_name, image_size },
        });
        allocated.push((loc, new_handle));
    }

    for (loc, new_handle) in allocated {
        let ent_data = match loc {
            ImageLoc::TopLevel(i) => &mut doc.entities[i].data,
            ImageLoc::Block(br_handle, i) => {
                &mut doc.block_records.get_mut(&br_handle).unwrap().entities[i].data
            }
        };
        if let EntityData::Image { image_def_handle, .. } = ent_data {
            *image_def_handle = new_handle;
        }
    }
}
```

### 2. Clone-on-demand 入口

`write_dxf_string` 保持 `&CadDocument`，只在 **检测到有 pending IMAGE** 时 clone：

```rust
pub fn write_dxf_string(doc: &CadDocument) -> Result<String, String> {
    if needs_ensure_image_defs(doc) {
        let mut owned = doc.clone();
        ensure_image_defs(&mut owned);
        write_dxf_string_impl(&owned)
    } else {
        write_dxf_string_impl(doc)
    }
}
```

`needs_ensure_image_defs` 做一遍只读扫描判断，O(entities) 成本；没有 pending 就零 clone 开销（覆盖大多数 AutoCAD 源 DXF 的 round-trip 路径）。

原 `write_dxf_string` body 抽出成 `fn write_dxf_string_impl(doc: &CadDocument)`。

### 3. Idempotency

对已走过 `ensure_image_defs` 的 doc 再次调用：所有 IMAGE 的 `image_def_handle` 已非 NULL → `needs_ensure_image_defs` 返回 false → 直接走 impl，零副作用。

## 实施步骤

### M1 — writer 新增函数（20 min）

1. 在 `crates/h7cad-native-dxf/src/writer.rs` 顶部补 imports：`use h7cad_native_model::{CadObject, EntityData, Handle, ObjectData};` 已由 `use h7cad_native_model::*;` 覆盖
2. 新增 `ensure_image_defs` + `needs_ensure_image_defs` 函数
3. 内部辅助枚举 `ImageLoc`

### M2 — 入口改造（10 min）

1. 原 `write_dxf_string` 内部 body → `write_dxf_string_impl(doc: &CadDocument)`
2. 新 `write_dxf_string(doc: &CadDocument)` 做 needs-check + clone + call-impl
3. `cargo check -p h7cad-native-dxf` 过

### M3 — 集成测试（20 min）

新建 `crates/h7cad-native-dxf/tests/imagedef_ensure.rs`，至少 5 条：

1. `ensure_creates_imagedef_for_top_level_image_with_file_path_only`：构造 top-level IMAGE `{ handle=NULL, file_path="solo.png" }` → `write_dxf` → 验证 text 含 340 + IMAGEDEF object，且 IMAGEDEF.file_name = "solo.png"
2. `ensure_skips_image_with_empty_file_path`：IMAGE `{ handle=NULL, file_path="" }` → write → doc 中无 IMAGEDEF 新增（compare objects count before/after）
3. `ensure_skips_image_that_already_has_handle`：预先构造 IMAGE `{ handle=0xABC, file_path="pre.png" }` + IMAGEDEF { handle=0xABC, file_name="pre.png" } → write → IMAGEDEF 对象数不变、handle 不变
4. `ensure_handles_image_inside_block_record`：IMAGE 放在 `doc.block_records[*].entities` → write → 同样 auto-create
5. `ensure_auto_created_imagedef_is_readable_after_roundtrip`：构造 `{ IMAGE file_path only }` → `write_dxf` → `read_dxf` → 断言读回的 IMAGE 有非 NULL handle + 正确 file_path，且 `doc.objects` 含 ImageDef

### M4 — 全量测试 + CHANGELOG（10 min）

1. `cargo test -p h7cad-native-dxf` 确认 88 → 93（+5 新测）
2. `cargo test --bin H7CAD io::native_bridge` 无回归（IMAGE bridge 已解耦 image_def_handle，不受 writer 改动影响）
3. 追写 CHANGELOG 2026-04-21 条目（归并到同日 IMAGEDEF 章节或独立小节）

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| Clone 成本（大 CadDocument，几万实体）| 只在需要时 clone；95%+ 读 AutoCAD 源 DXF 的 round-trip 不触发；auto-create 场景通常是 UI 构造小 doc |
| `allocate_handle` 碰撞：新 handle 与某些冷门对象（scale / sunstudy）handle 撞车 | `allocate_handle` 已从 `next_handle`（max + 1）开始递增，理论无碰撞；防御性验证在测试 3 中体现 |
| `block_records.get_mut(&br_handle).unwrap()` panic | 回填阶段的 br_handle 来自同 doc 的 keys 快照，一定存在；但仍加 debug_assert 兜底 |
| 二次 write 同 doc 副作用（idempotency） | `needs_ensure_image_defs` 只看 `handle == NULL && !file_path.is_empty()`，auto-create 后 handle 非 NULL，第二次返回 false，走 impl 零副作用 |
| Clone 不保留 `next_handle` 内部计数 | `CadDocument` derive Clone 包含私有 `next_handle` 字段，OK；验证可通过观察 `allocate_handle` 在 clone 后仍返回单调递增值 |

## 验收

- `cargo test -p h7cad-native-dxf` ≥ **93** passed（88 基线 + 5 新）
- `cargo test --bin H7CAD io::native_bridge` 仍 20/20
- `cargo check -p H7CAD` 零新 warning
- CHANGELOG 追加小节

## 执行顺序

M1 → M2 → M3 → M4（串行；每步过 compile + 自测）
