//! SQL查询模块
//!
//! 该模块提供SQL查询支持，允许用户使用标准SQL语法查询数据库中的数据。

mod error;
pub mod functions;
mod operations;
mod query_executor;
pub mod query_parser;
mod result_set;
mod utils;

pub use error::QueryExecutionError;
pub use query_executor::execute_query;
pub use query_parser::{
    parse_sql_query, ComparisonCondition, ComparisonOperator, Condition, JoinClause, JoinType,
    OrderByClause, OrderDirection, QueryParseError, QueryType, SqlQuery, Value, WhereClause,
};
pub use result_set::{ResultRow, ResultRowIter, ResultSet};
pub use utils::{
    check_memory_limit, parse_data_type_with_precision, process_at_time_zone,
    process_timezone_function, process_to_char, process_to_epoch, process_to_iso8601,
};

/// SQL查询结果
pub type SqlResult<T> = core::result::Result<T, SqlError>;

/// SQL错误类型
#[derive(Debug, Clone, PartialEq)]
pub enum SqlError {
    /// 查询解析错误
    ParseError(QueryParseError),
    /// 查询执行错误
    ExecutionError(QueryExecutionError),
    /// 不支持的SQL语句
    UnsupportedStatement,
    /// 无效的表名
    InvalidTableName,
    /// 无效的字段名
    InvalidFieldName,
}

impl From<QueryParseError> for SqlError {
    fn from(err: QueryParseError) -> Self {
        SqlError::ParseError(err)
    }
}

impl From<QueryExecutionError> for SqlError {
    fn from(err: QueryExecutionError) -> Self {
        SqlError::ExecutionError(err)
    }
}

impl core::fmt::Display for SqlError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SqlError::ParseError(err) => write!(f, "Parse error: {}", err),
            SqlError::ExecutionError(err) => write!(f, "Execution error: {}", err),
            SqlError::UnsupportedStatement => write!(f, "Unsupported SQL statement"),
            SqlError::InvalidTableName => write!(f, "Invalid table name"),
            SqlError::InvalidFieldName => write!(f, "Invalid field name"),
        }
    }
}

impl core::error::Error for SqlError {}
