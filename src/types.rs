//! H7CAD 共享基础类型门面层。
//!
//! 业务代码通过 `use crate::types::*` 访问 Vector/Color/Handle/LineWeight 等。
//! 未来把底层实现从 `acadrust` 切换到 native 实现时，只需修改本文件。
//!
//! 参见 `docs/plans/2026-04-17-acadrust-removal-plan.md` Layer 1。

#[allow(unused_imports)]
pub use acadrust::types::aci_table;
#[allow(unused_imports)]
pub use acadrust::types::{
    BoundingBox2D, BoundingBox3D, Color, DxfVersion, Handle, LineWeight, Matrix3, Matrix4,
    Transform, Transparency, Vector2, Vector3,
};
