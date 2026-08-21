//! SQL查询模块
//! 
//! 该模块提供SQL查询支持，允许用户使用标准SQL语法查询数据库中的数据。
#![allow(unsafe_code)]

mod query_parser;
mod query_executor;
mod result_set;

pub use query_parser::{SqlQuery, QueryParseError, parse_sql_query, WhereClause, Condition, ComparisonCondition, ComparisonOperator, OrderByClause, OrderDirection, QueryType, Value, JoinType, JoinClause};
pub use query_executor::{execute_query, QueryExecutionError};
pub use result_set::{ResultSet, ResultRow, ResultRowIter};

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
