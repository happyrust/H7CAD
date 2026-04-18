# DWG Native 下一轮移植计划（AC1015 收口：恢复率闭环 + INSERT 进入）

## Summary

下一轮仍只做 **AC1015**，不切 `facade`，不切主程序 `DWG runtime`，主战场仍是  
`D:/work/plant-code/cad/H7CAD/crates/h7cad-native-dwg`。

当前已完成之基线是：真实 `sample_AC1015.dwg` 可恢复 `84` 个实体，其中  
`26 LINE / 4 CIRCLE / 1 ARC / 6 POINT / 26 TEXT / 15 LWPOLYLINE / 6 HATCH`；  
然同一真实样本之 handle-map histogram 已显示更高的理论上限：  
`82 LINE / 9 CIRCLE / 3 ARC / 34 POINT / 17 LWPOLYLINE / 6 HATCH / 26 TEXT`。  

故下一轮之核心，不宜再泛泛加新类型，而应先把**已支持类型的恢复率闭环**做实；  
其后再引入**高实用价值且已具 native model / DXF / bridge 支撑面**的 `INSERT`。

---

## Thought（思路摘要）

此轮宜分二段而行：

1. **先补“已支持却未恢复”之缺口**  
   今 `TEXT/HATCH` 已近乎吃满，而 `LINE/POINT/CIRCLE/ARC/LWPOLYLINE` 与 histogram 尚有明显差距。若不先收口，贸然再扩实体族，只会把 silent-skip 面越铺越大。

2. **再接 `INSERT`，但必须带 block resolution 一起做**  
   `D:/work/plant-code/cad/H7CAD/crates/h7cad-native-model/src/lib.rs` 已有 `EntityData::Insert`，  
   `D:/work/plant-code/cad/H7CAD/src/io/native_bridge.rs` 已有 native↔acadrust 映射，  
   `D:/work/plant-code/cad/H7CAD/crates/h7cad-native-dxf` 亦已有 `INSERT + ATTRIB` 解析与 roundtrip 经验。  
   故 `INSERT` 已非“要先造模型”的题，而是“要把 DWG object/body/common/block 关系接通”的题，适合作为 AC1015 下一批首个新增实体。

---

## Implementation Plan（实施计划）

### 1. 建立“已支持类型恢复率”回归门禁
在  
`D:/work/plant-code/cad/H7CAD/crates/h7cad-native-dwg/tests/real_samples.rs`  
新增一组基于真实 AC1015 histogram 的回归门槛，分两层：

- **硬门槛**：已支持类型不得回退到 0 或明显跌落
- **恢复率门槛**：对 `LINE / POINT / CIRCLE / ARC / LWPOLYLINE` 建立最低恢复比例或最低计数 floor

本轮建议默认门槛如下：
- `TEXT`：维持 `26/26`
- `HATCH`：维持 `6/6`
- `LWPOLYLINE`：至少 `16/17`
- `LINE`：先提升到 **显著高于当前 26** 的稳定门槛
- `POINT`：先提升到 **显著高于当前 6** 的稳定门槛
- `CIRCLE / ARC`：分别提升到接近真实样本分布

此门槛不必一步到理论上限，但必须足以把“本轮是否真有进展”钉死。

### 2. 为 AC1015 enrichment 增加失败原因分型，而非继续静默吞掉
在  
`D:/work/plant-code/cad/H7CAD/crates/h7cad-native-dwg/src/lib.rs`  
及相关 reader/decoder 模块中，把当前 best-effort enrichment 的失败分类显式化。建议内部至少区分：

- `slice_miss`
- `header_fail`
- `handle_mismatch`
- `common_decode_fail`
- `body_decode_fail`
- `unsupported_type`

并在测试层输出/断言“已支持类型”的失败结构，避免出现：
- header 已读出
- type 已支持
- 但 body/common 因某个 optional 分支偏移而被静默丢弃

目标是把“恢复率差距”从黑盒，变成可定位的白盒。

### 3. 收口已支持实体族：先修恢复率，不新增更多散乱类型
针对当前已接线集合：
- `LINE`
- `ARC`
- `CIRCLE`
- `POINT`
- `TEXT`
- `LWPOLYLINE`
- `HATCH`

本轮重点不是再加第七、第八个无关类型，而是逐类审计：
- common preamble 是否仍有 optional flag 变体未覆盖
- main stream / handle stream 的消费顺序是否有分支漂移
- 某些 record subtype 是否被误判为已支持但实际 decoder 仍太窄
- 是否存在同类型的多种 body layout，只覆盖了其中一支

优先顺序建议：
1. `LINE`
2. `POINT`
3. `CIRCLE / ARC`
4. `LWPOLYLINE`

因为它们与 histogram 的差距最大，且修通后对真实图纸可见内容收益最高。

### 4. 在恢复率闭环后，引入 `INSERT`
在  
`D:/work/plant-code/cad/H7CAD/crates/h7cad-native-dwg/src/lib.rs`  
新增 `INSERT` object type dispatch，并增设对应 decoder 模块。  
实现目标不是“只有一个 block_name 的几何壳”，而是最小可用的 `INSERT`：

- `block_name`
- `insertion`
- `scale`
- `rotation`
- `owner_handle`
- `layer_name`
- 如真实样本存在 `attrib` 序列，则先至少保留 `has_attribs` 与空/最小 `attribs` 语义，不强求本轮完整 ATTRIB 串接

同时要求接上 block resolution：
- 能通过 `block_name` 或 block-record 关联，在 native document 中解析到目标 block record
- 对真实样本建立“至少部分 INSERT 可 resolve 到 block record”之断言

### 5. 仅做 read-path，保持外部边界不变
以下边界本轮明确不动：

- `D:/work/plant-code/cad/H7CAD/crates/h7cad-native-facade/src/lib.rs`
  - 仍保持 `NativeFormat::Dwg => Err("native DWG reader not implemented yet")`
- `D:/work/plant-code/cad/H7CAD/src/io/mod.rs`
  - 仍继续用 `acadrust::DwgReader / DwgWriter`
- 不做 DWG writer
- 不做 AC1018+
- 不做 runtime rollout

### 6. 文档同步为“恢复率闭环阶段”
更新  
`D:/work/plant-code/cad/H7CAD/CHANGELOG.md`  
与必要注释口径，明确下一轮状态应表述为：

- 已支持实体：从“可恢复”推进到“恢复率受门禁保护”
- 新增实体：`INSERT` 进入 AC1015 read-path
- facade/runtime 仍未开放

---

## Task List（任务清单）

1. 在真实样本测试中新增“已支持类型恢复率”门槛  
2. 给 enrichment 增加失败分类统计与测试可见性  
3. 逐类收口 `LINE / POINT / CIRCLE / ARC / LWPOLYLINE` 的恢复差距  
4. 新增 `INSERT` 的 object dispatch、body decode 与 common metadata 写回  
5. 为 `INSERT` 增加 block resolution 与真实样本断言  
6. 更新 changelog 与阶段口径说明  
7. 跑 DWG crate / facade / 主程序只读验证，确认边界未漂移  

---

## Important API / Interface Changes（接口与类型变更）

### 对外 public API
本轮**不新增**对外 crate API；以下入口保持不变：
- `D:/work/plant-code/cad/H7CAD/crates/h7cad-native-dwg/src/lib.rs` 中 `read_dwg(bytes)`
- `D:/work/plant-code/cad/H7CAD/crates/h7cad-native-dwg/src/lib.rs` 中 `sniff_version(bytes)`

### 内部接口
建议新增或收敛如下内部能力：

- enrichment 内部统计结构  
  例如：
  - `Ac1015RecoveryStats`
  - `SupportedEntityFailureKind`
- `INSERT` decoder 返回最小可用 native 载荷
- 对已支持实体的 decoder 统一返回可区分的失败类别，而不是单一 `Option`

### native model 依赖前提
`D:/work/plant-code/cad/H7CAD/crates/h7cad-native-model/src/lib.rs`  
已具备 `EntityData::Insert`，其字段足以承载本轮目标：
- `block_name`
- `insertion`
- `scale`
- `rotation`
- `has_attribs`
- `attribs`

故本轮默认**不改 native model 公共结构**，除非实现中发现 DWG 最小可用 `INSERT` 仍缺一项必需字段。

---

## Test Plan（测试计划）

### 必跑命令
```powershell
cargo test -p h7cad-native-dwg -- --test-threads=1
cargo test -p h7cad-native-dwg --test real_samples -- --nocapture --test-threads=1
cargo test -p h7cad-native-facade -- --test-threads=1
cargo check -p H7CAD
```

### 阶段验收

#### A. 恢复率闭环验收
对真实 `AC1015`：
- `TEXT` 继续全量恢复
- `HATCH` 继续全量恢复
- `LWPOLYLINE` 至少逼近 `17`
- `LINE / POINT / CIRCLE / ARC` 均较当前基线显著提升
- 已支持类型的失败原因在测试输出中可见、可比较、可回归

#### B. common metadata 语义验收
对恢复出的实体抽样断言：
- `owner_handle != NULL`
- `layer_name != "0"` 不能全部默认
- `color_index / linetype_name` 至少部分为真实值
- 对新增 `INSERT`，同样要求 common metadata 非纯默认污染

#### C. INSERT 验收
对真实样本与/或合成样本：
- `read_dwg()` 能返回 `EntityData::Insert`
- `block_name` 非空
- 至少部分 `INSERT` 可通过 document 解析到对应 block record
- 若存在属性序列，先至少保证 `has_attribs` 语义正确；若本轮接入 `attribs`，则补最小数量断言

#### D. 边界验收
- `D:/work/plant-code/cad/H7CAD/crates/h7cad-native-facade/src/lib.rs` 的 DWG 仍保持不可用
- `D:/work/plant-code/cad/H7CAD/src/io/mod.rs` 的 runtime DWG 路径保持不变
- 全 workspace 不因本轮 DWG crate 收口而引入兼容性回退

---

## Assumptions（假设与默认决策）

- 下一轮继续以 **AC1015 收口** 为唯一主线，不扩 AC1018+。
- `INSERT` 被纳入本轮，是因为 native model、DXF parser、bridge 已有现成承载面；故其风险显著低于从零开新实体族。
- 本轮默认优先级固定为：  
  **恢复率门禁 > 已支持类型收口 > INSERT > 文档同步**
- 本轮不做 DWG writer、不做 facade rollout、不做主程序 runtime 切换。
- 若 `INSERT` 的完整 `ATTRIB` 串接在实现中证实会明显拖慢恢复率主线，则默认先交付“可解析 block_name + insertion + transform + block resolution”的最小可用版本，把完整 `ATTRIB` 细化延后到再下一轮。
