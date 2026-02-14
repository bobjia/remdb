//! SQL Query Execution Error Types
//!
//! This module contains error types used in SQL query execution.

use alloc::string::String;

/// 查询执行错误
#[derive(Debug, Clone, PartialEq)]
pub enum QueryExecutionError {
    /// 表未找到
    TableNotFound,
    /// 字段未找到
    FieldNotFound,
    /// 类型不匹配
    TypeMismatch,
    /// 无效的条件
    InvalidCondition,
    /// 内存不足
    OutOfMemory,
    /// 约束冲突
    ConstraintsConflicts,
    /// 内部错误
    InternalError,
    /// 操作不允许
    NotAllowed,
    /// 不支持的函数
    UnsupportedFunction(String),
    /// 无效的值
    InvalidValue,
    /// 资源限制超出
    ResourceLimitExceeded(String),
}

impl core::fmt::Display for QueryExecutionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            QueryExecutionError::TableNotFound => write!(f, "Table not found"),
            QueryExecutionError::FieldNotFound => write!(f, "Field not found"),
            QueryExecutionError::TypeMismatch => write!(f, "Type mismatch"),
            QueryExecutionError::InvalidCondition => write!(f, "Invalid condition"),
            QueryExecutionError::OutOfMemory => write!(f, "Out of memory"),
            QueryExecutionError::ConstraintsConflicts => write!(f, "Constraints conflicts"),
            QueryExecutionError::InternalError => write!(f, "Internal error"),
            QueryExecutionError::NotAllowed => write!(f, "Operation not allowed"),
            QueryExecutionError::UnsupportedFunction(func) => {
                write!(f, "Unsupported function: {}", func)
            }
            QueryExecutionError::InvalidValue => {
                write!(f, "Invalid value")
            }
            QueryExecutionError::ResourceLimitExceeded(msg) => {
                write!(f, "Resource limit exceeded: {}", msg)
            }
        }
    }
}

impl core::error::Error for QueryExecutionError {}
