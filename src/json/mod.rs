//! JSON支持模块
//!
//! 该模块提供轻量级JSON数据类型支持，包括：
//! - 二进制JSON存储（MessagePack/CBOR）
//! - 专用JSON内存池
//! - JSON路径查询
//! - JSON文档操作

pub mod document;
pub mod memory_pool;
pub mod path;

pub use crate::types::JsonStorage;
pub use document::JsonDocument;
pub use document::JsonQueryResult;
pub use document::JsonValue;
pub use memory_pool::BlockHeader;
pub use memory_pool::JsonMemoryPool;
