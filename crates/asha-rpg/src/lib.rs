//! Supported public facade for portable RPG authority.
//!
//! Rust owns normalized semantic declarations and the deterministic authority
//! loop. Downstream games own authored content and product workflows.

#![forbid(unsafe_code)]

pub use rpg_core::*;
pub use rpg_ir::*;
pub use rpg_runtime::*;
