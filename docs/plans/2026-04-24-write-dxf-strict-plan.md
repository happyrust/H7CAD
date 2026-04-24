# Phase 1：write_dxf_strict 落地 + DXF 下一步开发计划

> 起稿：2026-04-24（三十轮续）
> 前置：post-DIMALT roadmap 已定稿；DxfWriteError enum 已存在于 writer.rs
> 目的：落地 Phase 1（write_dxf_strict），并规划后续 Phase 2-4 的执行节奏

---

## 1. Phase 1：write_dxf_strict（本轮执行）

### 1.1 目标

新增 `write_dxf_strict` 公有函数，返回 `Result<String, DxfWriteError>`，
与现有 `write_dxf_string` 的 `Result<String, String>` 并存：

```rust
pub fn write_dxf_strict(doc: &CadDocument) -> Result<String, DxfWriteError>
```

### 1.2 改动清单

| 文件 | 改动 |
|------|------|
| `crates/h7cad-native-dxf/src/writer.rs` | 新增 `write_dxf_strict` 函数，复用 `write_dxf_string_impl` |
| `crates/h7cad-native-dxf/src/lib.rs` | re-export `write_dxf_strict` + `DxfWriteError` |
| `crates/h7cad-native-dxf/tests/writer_error_types.rs` | 新测试文件，3 条测试 |

### 1.3 兼容性

- `write_dxf_string` 签名 **不变**（`Result<String, String>`）
- `write_dxf_strict` 是**新增** API，不破坏任何下游
- `DxfWriteError` 已有 `From<String>` 和 `Display`，下游可无缝转换

### 1.4 测试

1. `write_dxf_strict_returns_ok_for_minimal_doc` — 空文档写入成功
2. `write_dxf_strict_matches_write_dxf_string` — 两个 API 产出相同结果
3. `dxf_write_error_display_roundtrip` — Display 输出可读

### 1.5 验收

- `cargo test -p h7cad-native-dxf` 169 → 172+ 全绿
- `cargo check -p H7CAD` 零新 warning
- `cargo check -p h7cad-native-facade` 零新 warning

---

## 2. Phase 2：Entity 覆盖扩展（下一阶段）

按 post-DIMALT roadmap Path B，每轮 1 实体：

| 轮次 | 实体 | 复杂度 | 说明 |
|------|------|--------|------|
| R31 | ACAD_PROXY_ENTITY 增强 | ★★ | 已有 ProxyEntity 但缺 raw binary data |
| R32 | OLE2FRAME | ★★★ | OLE 对象嵌入，含 binary stream |
| R33 | MPOLYGON | ★★★ | 多边形填充，类似 Hatch 但独立类型 |
| R34 | GEOPOSITIONMARKER | ★★ | 地理位置标记，字段简单 |
| R35 | INSERT block_record fix | ★★ | 修复 group code 340 缺失 |

---

## 3. Phase 3：DWG 红灯修复（战略级）

待 Phase 2 实体覆盖达 50+ 后启动：

- AC1015 LINE body_decode 修复
- CIRCLE / ARC / POINT / TEXT 修复
- 最终目标：`real_dwg_samples_baseline_m3b` 全绿

---

## 4. Phase 4：长尾打磨

并行推进：
- HEADER 长尾（Path A）：每轮 3-5 变量
- Fuzz 保真（Path D）：proptest 框架 + f64 edge case
- IMAGE/IMAGEDEF 标准链接（替代 code 1 workaround）
- DXF 诊断信息接入（load 时返回 notices）
- Binary DXF 写入

---

## 5. 状态

- [x] 开发计划定稿
- [x] Phase 1 落地（write_dxf_strict）— 2026-04-24 完成
- [ ] Phase 2 实体扩展
- [ ] Phase 3 DWG 修复
- [ ] Phase 4 长尾打磨
