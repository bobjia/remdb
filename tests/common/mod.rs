//! 测试公共模块
//!
//! 该模块包含测试用例的公共辅助代码，包括：
//! - 平台抽象层实现
//! - 数据库初始化辅助函数
//! - 测试工具函数

pub mod db_setup;
pub mod platform;

pub use db_setup::{setup_test_db, setup_test_db_with_memory};
