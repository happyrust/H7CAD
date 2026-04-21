# 开发计划：`ObjectData::ImageDef` 扩字段到完整 DXF 标准

> 起稿：2026-04-21（第六轮）  
> 前置：`docs/plans/2026-04-21-imagedef-object-plan.md` + `docs/plans/2026-04-21-imagedef-auto-create-plan.md` 已完成。IMAGE↔IMAGEDEF 标准链接通了，但 `ObjectData::ImageDef` 目前只存两个字段（`file_name: String, image_size: [f64; 2]`），真实 AutoCAD 输出的 IMAGEDEF 还会带 **resolution_unit / pixel_size / class_version / image_is_loaded_flag** 四个字段。DWG 原生侧（`vendor_tmp/acadrust/src/io/dwg/dwg_stream_readers/object_reader/objects.rs::read_image_definition`）已读全 6 字段，DXF 侧漏读漏写导致 round-trip 丢信息。

## 动机

当前 DXF reader（`crates/h7cad-native-dxf/src/lib.rs:1136-1151`）只读 IMAGEDEF 的 code 1 / 10 / 20：

```rust
"IMAGEDEF" => {
    let mut file_name = String::new();
    let (mut w, mut h) = (0.0, 0.0);
    for &(code, ref val) in &codes {
        match code {
            1 => file_name = val.clone(),
            10 => w = val.parse().unwrap_or(0.0),
            20 => h = val.parse().unwrap_or(0.0),
            _ => {}
        }
    }
    ObjectData::ImageDef { file_name, image_size: [w, h] }
}
```

AutoCAD DXF Reference IMAGEDEF 标准字段（完整列表）：

| Code | 字段 | 类型 | 含义 |
|---|---|---|---|
| 1 | file_name | string | Raster 图像路径 |
| **10** | image_size_u | double | Image size in pixels — U direction |
| **20** | image_size_v | double | Image size in pixels — V direction |
| **11** | pixel_size_u | double | Default size of one pixel in AutoCAD units — U |
| **21** | pixel_size_v | double | Default size of one pixel in AutoCAD units — V |
| **90** | class_version | i32 | Class version |
| **71** | image_is_loaded_flag | i16 (0/1) | Image is loaded flag |
| **281** | resolution_unit | u8 | 0 = None, 2 = centimeters, 5 = inches |

实际 H7CAD 读 AutoCAD DXF 时这四组信息丢失，写回时用默认值（空），不符合 DWG native 侧的完整度。

## 目标

1. `ObjectData::ImageDef` 扩 4 字段：
   - `pixel_size: [f64; 2]`（code 11/21，default `[1.0, 1.0]`）
   - `class_version: i32`（code 90，default 0）
   - `image_is_loaded: bool`（code 71，default true）
   - `resolution_unit: u8`（code 281，default 0 = None）
2. DXF reader (`crates/h7cad-native-dxf/src/lib.rs`) IMAGEDEF 读全 7 group codes
3. DXF writer (`crates/h7cad-native-dxf/src/writer.rs`) IMAGEDEF 输出全 7 group codes
4. `ensure_image_defs` (writer pre-pass) 在 auto-create IMAGEDEF 时用合理默认值填充新字段（`pixel_size: [1.0, 1.0], class_version: 0, image_is_loaded: true, resolution_unit: 0`）
5. 测试：round-trip 保留所有新字段；legacy DXF（只有 code 1/10/20）读回时新字段走 default

## 非目标

- 不动 `EntityData::Image` 字段（IMAGE 实体的字段已在前轮稳定）
- 不改 `IMAGEDEF_REACTOR`（独立工作）
- 不改 DWG reader（vendor_tmp/acadrust 已经正确；H7CAD 这边只关心 DXF）
- 不做"pixel_size 推断"从 image_size 计算的 fallback（若原始 DXF 没有 code 11/21 → 默认 1.0/1.0，不反推）
- 不改 bridge（`acadrust::ImageDefinitionData` 已经有完整字段，bridge 方向是 native→acadrust，只要 native 侧有就 OK；若需要反向 bridge 可加，但 acadrust DXF writer 我们不用，不碰）
- 不扩 `ObjectData::ImageDefReactor`

## 关键设计

### 1. Model 扩字段

`crates/h7cad-native-model/src/lib.rs` 的 `ObjectData::ImageDef` 变体从 2 字段变 6 字段：

```rust
ImageDef {
    /// DXF code 1: Raster image absolute path.
    file_name: String,
    /// DXF code 10/20: image size in pixels (U, V).
    image_size: [f64; 2],
    /// DXF code 11/21: default size of one pixel in AutoCAD drawing
    /// units (U, V). Defaults to [1.0, 1.0] so a freshly auto-created
    /// IMAGEDEF treats 1 pixel = 1 drawing unit.
    pixel_size: [f64; 2],
    /// DXF code 90: class version. Defaults to 0.
    class_version: i32,
    /// DXF code 71: whether the referenced file was loaded at save
    /// time. Defaults to true.
    image_is_loaded: bool,
    /// DXF code 281: resolution unit (0 = None, 2 = centimeters,
    /// 5 = inches). Defaults to 0 = None.
    resolution_unit: u8,
},
```

### 2. Reader

扩 match 新 4 个 code：

```rust
"IMAGEDEF" => {
    let mut file_name = String::new();
    let (mut w, mut h) = (0.0, 0.0);
    let mut pxu = 1.0;
    let mut pxv = 1.0;
    let mut class_version: i32 = 0;
    let mut image_is_loaded = true;
    let mut resolution_unit: u8 = 0;
    for &(code, ref val) in &codes {
        match code {
            1 => file_name = val.clone(),
            10 => w = val.parse().unwrap_or(0.0),
            20 => h = val.parse().unwrap_or(0.0),
            11 => pxu = val.parse().unwrap_or(1.0),
            21 => pxv = val.parse().unwrap_or(1.0),
            90 => class_version = val.parse().unwrap_or(0),
            71 => image_is_loaded = val.trim() != "0",
            281 => resolution_unit = val.trim().parse().unwrap_or(0),
            _ => {}
        }
    }
    ObjectData::ImageDef {
        file_name, image_size: [w, h],
        pixel_size: [pxu, pxv],
        class_version, image_is_loaded, resolution_unit,
    }
}
```

Legacy DXF 缺少新 code → 走初始化的 default（pixel_size [1, 1] / class_version 0 / image_is_loaded true / resolution_unit 0）。

### 3. Writer

对称输出（按 AutoCAD 常用顺序：1 → 10/20 → 11/21 → 90 → 71 → 281）：

```rust
ObjectData::ImageDef {
    file_name, image_size, pixel_size,
    class_version, image_is_loaded, resolution_unit,
} => {
    w.pair_str(0, "IMAGEDEF");
    w.pair_handle(5, obj.handle);
    w.pair_handle(330, obj.owner_handle);
    w.pair_str(1, file_name);
    w.pair_f64(10, image_size[0]);
    w.pair_f64(20, image_size[1]);
    w.pair_f64(11, pixel_size[0]);
    w.pair_f64(21, pixel_size[1]);
    w.pair_i32(90, *class_version);
    w.pair_i16(71, if *image_is_loaded { 1 } else { 0 });
    w.pair_i16(281, *resolution_unit as i16);
}
```

### 4. `ensure_image_defs` 同步

`writer.rs::ensure_image_defs` 构造 `ObjectData::ImageDef { file_name, image_size }` 的地方要补上新 4 字段默认值：

```rust
doc.objects.push(CadObject {
    handle: new_handle,
    owner_handle: Handle::NULL,
    data: ObjectData::ImageDef {
        file_name,
        image_size,
        pixel_size: [1.0, 1.0],
        class_version: 0,
        image_is_loaded: true,
        resolution_unit: 0,
    },
});
```

### 5. 其它构造点

`cargo check` 会扫出所有构造 `ObjectData::ImageDef { ... }` 的地方缺字段报错；逐一修复即可。前两轮的集成测试 fixtures 里也有几处，一并补。

## 实施步骤

### M1 — model 扩字段（10 min）

`h7cad-native-model/src/lib.rs` 改 `ObjectData::ImageDef` 变体 + 更新 doc comments。

### M2 — DXF reader（15 min）

`crates/h7cad-native-dxf/src/lib.rs` 扩 match 分支。

### M3 — DXF writer（15 min）

`crates/h7cad-native-dxf/src/writer.rs::write_object` 的 `ObjectData::ImageDef` 分支扩输出。

### M4 — ensure_image_defs 默认值（10 min）

`writer.rs::ensure_image_defs` 构造点补上新字段默认值。

### M5 — 集成测试 + 前轮测试修复（30 min）

- 在 `tests/imagedef_roundtrip.rs` 新增：
  - `imagedef_roundtrip_preserves_extended_fields`：手写完整 IMAGEDEF（含 11/21/90/71/281）→ read → 断言读回的 4 个新字段；write 回来 → read → 字段稳定
  - `imagedef_reader_legacy_no_ext_fields_uses_defaults`：legacy DXF（只 1/10/20）→ 新字段走 default
- 在 `tests/imagedef_ensure.rs` 新增：
  - `ensure_auto_created_imagedef_fills_standard_defaults`：auto-create 后读出的 IMAGEDEF 扩字段值 == 规范默认值
- 修前轮测试（前两轮手动构造 `ObjectData::ImageDef { file_name, image_size }` 处补字段）

### M6 — validator + CHANGELOG（10 min）

- `cargo test -p h7cad-native-dxf` ≥ 98 → **100+**（+3 新）
- `cargo test --bin H7CAD io::native_bridge` 无回归
- CHANGELOG 追加 "2026-04-21（六）" 条目

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 旧 `ObjectData::ImageDef { file_name, image_size }` 构造点编译失败 | `cargo check` 会精确定位每处；批量改 |
| `resolution_unit` DXF 规范是 u8 还是 i16？ | AutoCAD 写 code 281 是 i16（虽然值域只用 0/2/5），存 u8 够用；写入时 cast 回 i16 |
| Legacy DXF 无 code 71，解析出 image_is_loaded = default true | 符合 AutoCAD 行为（未设 = 已加载）|
| `pixel_size [1, 1]` 默认导致 image 显示比例错 | 真实 DXF 都会带 code 11/21；默认只对 auto-create 场景生效（此时用户只给了 file_path，没有像素→绘图单位比例信息，1:1 是合理初值）|

## 验收

- `cargo test -p h7cad-native-dxf` ≥ **101**（98 + 3 新 round-trip / legacy / ensure-defaults）
- `cargo test --bin H7CAD io::native_bridge` 仍 20/20
- `cargo check -p H7CAD` 零新 warning
- CHANGELOG 条目

## 执行顺序

M1 → M2 → M3 → M4 → M5 → M6（严格串行；每步过 compile）
