# 开发计划：IMAGE 实体 ↔ IMAGEDEF 对象标准化链接

> 起稿：2026-04-21  
> 背景：H7CAD DXF 解析进度盘点中识别的首号待办项。`ObjectData::ImageDef` 已在 model 层定义，DXF reader/writer 已能独立读写 IMAGEDEF object，但 `EntityData::Image` 仍通过**非标准 code 1** 直接携带 `file_path`，没有走 DXF 标准的 **code 340 → IMAGEDEF handle** 链接机制。结果：读真实 AutoCAD 输出的 DXF 时，IMAGE 实体的 `file_path` 字段会丢失。

## 动机

**当前链路（hack）**：
- Reader 端（`parse_image`，`entity_parsers.rs:287`）：把 IMAGE 实体上的 `code 1` 直接读作 `file_path`
- Writer 端（`write_entity_data::Image`，`writer.rs:878`）：把 `file_path` 直接写成 IMAGE 实体的 `code 1`
- 后果：
  - 读 AutoCAD / ACadSharp 等标准 DXF 生成器的文件 → `file_path` 恒为空（标准里 IMAGE 实体不带 code 1）
  - 虽然 `ObjectData::ImageDef` 能被 reader 读入 `doc.objects`，但 IMAGE 与 IMAGEDEF 之间**无链接**，UI 显示时不知道该 raster image 指向哪个文件

**标准链路**：
```
IMAGE (entity)              IMAGEDEF (object)
├─ 10/20/30: insertion      ├─ 5: handle (eg "4AF")
├─ 11/21/31: u_vector       ├─ 1: file_name
├─ 12/22/32: v_vector       ├─ 10/20: size_in_pixels
├─ 13/23:    image_size     ├─ 281: resolution_unit
├─ 70:       display_flags  └─ ...
└─ 340:      imagedef_handle ─────▲
                                  │
                                  指向 IMAGEDEF.handle
```

`code 340` 是 "Hard-pointer to IMAGEDEF"（`DxfObjectHandle`），而 `code 1` 在 IMAGE 实体上在 AutoCAD 官方定义里是**保留未使用**的。

## 目标

1. `EntityData::Image` 新增字段 `image_def_handle: Handle`（默认 `Handle::NULL`），承载标准 340 链接
2. Reader：
   - `parse_image` 读取 code 340 → `image_def_handle`
   - 保留 code 1 为**遗留兼容 fallback**（我们自己旧版写出的文件）
   - `read_dxf` 主流程末尾加 **`resolve_image_def_links()` 阶段**：若 `file_path` 空而 `image_def_handle` 非零 → 从 `doc.objects` 查 `ObjectData::ImageDef.file_name` 回填
3. Writer：
   - `write_entity_data::Image` 写 code 340（若非零）
   - **Pre-write `ensure_image_defs()` 阶段**：扫描所有 IMAGE 实体，若 `image_def_handle == NULL` 且 `file_path` 非空 → 分配新 handle，插入 `ObjectData::ImageDef` 到 `doc.objects`，回填 IMAGE 的 handle
   - 保留 code 1 on IMAGE 输出（**可选**，默认关闭；用内部 feature flag 或 writer option 控制，便于逐步迁移）
4. Bridge 层（`src/io/native_bridge.rs`）：6 处构造 `EntityData::Image` 的地方追加 `image_def_handle: Handle::NULL` 初始化
5. 测试：
   - 标准 DXF（只有 340，无 code 1）→ file_path 正确回填
   - 遗留 DXF（只有 code 1，无 340）→ file_path 正确读取（fallback）
   - 双写 DXF（both 340 + code 1）→ 优先取 IMAGEDEF.file_name
   - Writer 自动生成 IMAGEDEF：构造 Image 只填 file_path → 写出 → 再读入 → file_path 保留、image_def_handle 非零
   - Round-trip：读 → 写 → 读，IMAGE.image_def_handle 与 IMAGEDEF.handle 严格一致

## 非目标

- 不处理 `IMAGEDEF_REACTOR` 的自动链接（现已能独立读写，交给 future work）
- 不支持 IMAGEDEF 的外部文件存在性校验（file_name 指向的实际 raster 文件可能不存在 — 那是 UI 层的 raster loader 的事）
- 不改 `ObjectData::ImageDef` 结构，不扩充 `resolution_unit` / `pixel_size` / `class_version` 字段（现有 `file_name + image_size` 够用；AutoCAD 写回时给未知字段默认值即可）
- 不追踪 IMAGEDEF 的 `XRecord` / 辅助字段
- 不处理 Binary DXF 路径单独的 IMAGE 逻辑（现有 "binary → text → reader" 路径会复用修正后的 text reader）
- 不改 DWG reader（`h7cad-native-dwg` / `acadrust::DwgReader` 当前是否已正确处理 IMAGE↔IMAGEDEF 是独立问题）

## 关键设计

### 1. `EntityData::Image` 新字段

`crates/h7cad-native-model/src/lib.rs:337-352` 追加：

```rust
Image {
    insertion: [f64; 3],
    u_vector: [f64; 3],
    v_vector: [f64; 3],
    image_size: [f64; 2],
    /// DXF code 340: Hard-pointer to linked IMAGEDEF object. Handle::NULL
    /// means the image entity is unlinked (legacy; writer will auto-create
    /// an IMAGEDEF during ensure_image_defs phase when file_path is set).
    image_def_handle: Handle,
    /// File path to the raster image. Authoritative when image_def_handle
    /// is NULL. When image_def_handle is set, this is a **cached mirror**
    /// of ImageDef.file_name (populated by resolve_image_def_links after
    /// DXF read); writers should prefer IMAGEDEF.file_name as source of truth.
    file_path: String,
    /// DXF code 70 image display flags bitfield.
    display_flags: i32,
},
```

Doc comment 明确"file_path 是 mirror"，下游代码继续直接读 file_path 不会 break。

### 2. Reader post-resolve

`crates/h7cad-native-dxf/src/lib.rs` 末尾（`read_dxf` 返回前）加：

```rust
fn resolve_image_def_links(doc: &mut CadDocument) {
    let imagedef_by_handle: HashMap<Handle, String> = doc.objects
        .iter()
        .filter_map(|o| match &o.data {
            ObjectData::ImageDef { file_name, .. } => Some((o.handle, file_name.clone())),
            _ => None,
        })
        .collect();

    for e in &mut doc.entities {
        if let EntityData::Image { image_def_handle, file_path, .. } = &mut e.data {
            if *image_def_handle != Handle::NULL && file_path.is_empty() {
                if let Some(name) = imagedef_by_handle.get(image_def_handle) {
                    *file_path = name.clone();
                }
            }
        }
    }
}
```

单次遍历，O(objects + entities)，不改 `doc.objects`。

### 3. Writer pre-resolve (`ensure_image_defs`)

`crates/h7cad-native-dxf/src/writer.rs` 入口（`write_dxf_string` / `write_dxf`）首尾：

```rust
fn ensure_image_defs(doc: &mut CadDocument) {
    for e in &mut doc.entities {
        if let EntityData::Image { image_def_handle, file_path, image_size, .. } = &mut e.data {
            if *image_def_handle == Handle::NULL && !file_path.is_empty() {
                let new_handle = doc.allocate_handle();
                doc.objects.push(CadObject {
                    handle: new_handle,
                    owner_handle: /* ACAD_IMAGE_DICT handle or NULL */,
                    data: ObjectData::ImageDef {
                        file_name: file_path.clone(),
                        image_size: *image_size,
                    },
                });
                *image_def_handle = new_handle;
            }
        }
    }
}
```

**注意事项**：
- `allocate_handle` 借用冲突：需把 IMAGE 遍历拆成两阶段，先收集 `(entity_idx, file_path, image_size)`，再一次性 allocate + push
- owner_handle：AutoCAD 标准要求 IMAGEDEF 的 owner 是 `ACAD_IMAGE_DICT` 子字典；若 doc 无该字典则用 `Handle::NULL`（有损但不破坏解析）
- 若 Writer 接受 `&CadDocument`（不可变）：增加 `ensure_image_defs(&mut doc)` 变体路径，或在 `write_dxf_string` 内部 clone doc 做预处理（clone 成本 vs 显式 mut API 的取舍 — 倾向后者）

### 4. Writer 实体输出

`writer.rs:863-884` 替换：

```rust
EntityData::Image {
    insertion, u_vector, v_vector, image_size,
    image_def_handle, file_path: _,  // 忽略 file_path，源为 IMAGEDEF
    display_flags,
} => {
    w.point3d(10, *insertion);
    w.point3d(11, *u_vector);
    w.point3d(12, *v_vector);
    w.pair_f64(13, image_size[0]);
    w.pair_f64(23, image_size[1]);
    if *image_def_handle != Handle::NULL {
        w.pair_handle(340, *image_def_handle);
    }
    if *display_flags != 0 {
        w.pair_i32(70, *display_flags);
    }
}
```

不再写 code 1 on IMAGE — 依赖 `ensure_image_defs` 保证 IMAGEDEF 存在。

## 实施步骤

### M1 — model 扩字段（15 min）

1. `crates/h7cad-native-model/src/lib.rs`：
   - `EntityData::Image` 加 `image_def_handle: Handle`
   - doc comment 更新（标注 file_path 为 mirror）
2. `cargo check -p h7cad-native-model` 确认 model crate 自身通过

### M2 — reader 修正（20 min）

1. `parse_image` 读 code 340（`i16` 两个半字节？不，340 是 string 型 handle，走 `u64::from_str_radix(val, 16)`）
2. `read_dxf` 末尾调 `resolve_image_def_links`
3. 单测（在 `entity_parsers.rs` 或 `lib.rs` 的 `#[cfg(test)]` 模块）：
   - `parse_image_reads_code_340`
   - `parse_image_legacy_code_1_fallback`
   - `resolve_image_def_links_fills_file_path`
   - `resolve_image_def_links_noop_when_file_path_already_set`

### M3 — writer 修正（30 min）

1. `ensure_image_defs` 实现（两阶段 allocate，避免借用冲突）
2. `write_entity_data::Image` 替换成标准输出
3. 入口函数（`write_dxf_string` / `write_dxf`）在写前调 `ensure_image_defs`
4. 单测：
   - `writer_emits_code_340_when_handle_set`
   - `writer_auto_creates_imagedef_when_handle_null`
   - `writer_does_not_emit_code_1_on_image`

### M4 — bridge 更新（10 min）

1. `src/io/native_bridge.rs`：6 处 `EntityData::Image { ... }` 构造追加 `image_def_handle: Handle::NULL`
2. `cargo check -p H7CAD` 确认主 crate 通过

### M5 — 集成测试（20 min）

新建 `crates/h7cad-native-dxf/tests/imagedef_roundtrip.rs`：

1. `image_standard_dxf_resolves_file_path`：手写含 IMAGEDEF + IMAGE(340) 的 DXF 字符串 → 解析 → 断言 file_path
2. `image_legacy_dxf_reads_code_1`：手写含 IMAGE(code 1) 但无 IMAGEDEF 的 DXF → 解析 → 断言 file_path
3. `image_writer_auto_creates_imagedef`：构造 CadDocument { IMAGE { file_path: "test.png", ... } } → write_dxf → read_dxf → 断言 image_def_handle 非零且 objects 含 IMAGEDEF
4. `image_roundtrip_preserves_handle_linkage`：构造 { IMAGE(handle=X) ↔ IMAGEDEF(handle=X) } → round-trip → 链接保持

### M6 — CHANGELOG + 收敛（10 min）

1. 在 `CHANGELOG.md` 顶部 `## [未发布]` 下追加本轮条目（中文）
2. `cargo test -p h7cad-native-dxf` 全绿
3. `cargo check -p H7CAD` 无回归

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| `allocate_handle` 需要 `&mut doc`，writer 当前签名 `&CadDocument` | 在 writer 入口改签名或做 `Cow<CadDocument>` 风格预处理 |
| owner_handle 设为 NULL 时某些严格 DXF parser 拒绝 | 先验：手写的 IMAGEDEF owner NULL 文件过 `read_dxf_bytes` round-trip，若过则接受有损 |
| 6 处 bridge 构造地方遗漏 | 编译失败会直接提示 "missing field `image_def_handle`"；过编译即覆盖 |
| 下游 UI 代码破坏 | 保持 `file_path` 字段签名，doc comment 承诺 "mirror" 语义；不删除 |

## 验收

- `cargo test -p h7cad-native-dxf` ≥ 85 passed（81 基线 + 4 新单测 + 4 新集成测试）
- `cargo check -p H7CAD` 零 warning 增量
- CHANGELOG 有对应条目
- 手动烟测：构造含 IMAGE 的 DXF → 写出 → 用 AutoCAD LT 或 ACadSharp 验证能识别 IMAGEDEF 链接（可选）

## 执行顺序

M1 → M2 → M3 → M4 → M5 → M6（严格串行；每一步过 compile + 自测后进下一步）
