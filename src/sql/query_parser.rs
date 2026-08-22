//! SQL查询解析器
//!
//! 该模块负责将SQL查询字符串解析为结构化的查询对象。

use alloc::boxed::Box;
use crate::RemDbError;
use std::collections::HashMap;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;

#[cfg(feature = "log")]
use crate::log::debug;

/// 解析时间字符串为微秒时间戳
/// 支持的格式：
/// - '2024-01-15 10:30:45'
/// - '2024-01-15T10:30:45.123Z'
/// - '2024-01-15 10:30:45.123+08'
/// - 1673778645123456 (微秒时间戳)
pub fn parse_time_string(time_str: &str) -> Result<i64, ()> {
    if time_str.starts_with('[') && time_str.ends_with(']') {
        return Err(());
    }
    
    if time_str.contains(|c| c == 'Y' || c == 'M' || c == 'D' || c == 'H' || c == 'I' || c == 'S') {
        return Err(());
    }
    
    if let Ok(timestamp) = time_str.parse::<i64>() {
        return Ok(timestamp);
    }
    
    let time_str = time_str.trim();
    let mut parts = time_str.split_whitespace();
    
    let date_part = parts.next().ok_or(())?;
    let date_components: Vec<&str> = date_part.split('-').collect();
    if date_components.len() != 3 {
        return Err(());
    }
    
    let year = date_components[0].parse::<i64>().map_err(|_| ())?;
    let month = date_components[1].parse::<i64>().map_err(|_| ())?;
    let day = date_components[2].parse::<i64>().map_err(|_| ())?;
    
    let mut hour = 0;
    let mut minute = 0;
    let mut second = 0;
    
    if let Some(time_part) = parts.next() {
        let (time_only, _tz_offset_seconds) = split_timezone_from_time(time_part);
        let time_components: Vec<&str> = time_only.split(':').collect();
        if time_components.len() != 3 {
            return Err(());
        }
        
        hour = time_components[0].parse::<i64>().map_err(|_| ())?;
        minute = time_components[1].parse::<i64>().map_err(|_| ())?;
        second = time_components[2].parse::<i64>().map_err(|_| ())?;
    }
    
    let mut seconds = 0;
    
    for _y in 1970..year {
        seconds += 365 * 24 * 60 * 60;
    }
    
    let days_in_month = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for m in 0..(month - 1) {
        seconds += days_in_month[m as usize] * 24 * 60 * 60;
    }
    
    seconds += (day - 1) * 24 * 60 * 60;
    seconds += hour * 60 * 60;
    seconds += minute * 60;
    seconds += second;
    
    Ok(seconds * 1000000)
}

fn split_timezone_from_time(time_part: &str) -> (&str, i32) {
    if let Some(pos) = time_part.find(|c| c == '+' || c == '-') {
        if pos > 0 {
            let before = &time_part[..pos];
            let after = &time_part[pos..];
            if after.len() > 1 && after.chars().nth(1).map_or(false, |c| c.is_ascii_digit()) {
                let tz_seconds = parse_timezone_offset(after).unwrap_or(0);
                return (before, tz_seconds);
            }
        }
    }
    (time_part, 0)
}

fn parse_timezone_offset(tz_str: &str) -> Option<i32> {
    let sign = if tz_str.starts_with('+') { 1 } else if tz_str.starts_with('-') { -1 } else { return None };
    let offset_str = &tz_str[1..];
    
    let parts: Vec<&str> = offset_str.split(':').collect();
    if parts.len() == 2 {
        let hours = parts[0].parse::<i32>().ok()?;
        let minutes = parts[1].parse::<i32>().ok()?;
        Some(sign * (hours * 3600 + minutes * 60))
    } else if offset_str.len() == 2 {
        let hours = offset_str.parse::<i32>().ok()?;
        Some(sign * hours * 3600)
    } else if offset_str.len() == 4 {
        let hours = offset_str[0..2].parse::<i32>().ok()?;
        let minutes = offset_str[2..4].parse::<i32>().ok()?;
        Some(sign * (hours * 3600 + minutes * 60))
    } else {
        None
    }
}

/// GROUP BY子句
#[derive(Debug, Clone, PartialEq)]
pub struct GroupByClause {
    /// 分组表达式列表
    pub expressions: Vec<Expression>,
    /// 分组字段列表（兼容旧版本）
    pub fields: Vec<String>,
}

/// JOIN类型枚举
#[derive(Debug, Clone, PartialEq)]
pub enum JoinType {
    /// 内连接
    Inner,
    /// 左连接
    Left,
    /// 右连接
    Right,
    /// 全连接
    Full,
}

/// JOIN子句
#[derive(Debug, Clone, PartialEq)]
pub struct JoinClause {
    /// JOIN类型
    pub join_type: JoinType,
    /// 连接表名
    pub table_name: String,
    /// 连接表别名
    pub table_alias: Option<String>,
    /// 连接条件
    pub on_condition: Condition,
}

/// 窗口函数子句
#[derive(Debug, Clone, PartialEq)]
pub struct WindowFunctionClause {
    /// 窗口函数名称
    pub name: String,
    /// 窗口函数参数
    pub args: Vec<Expression>,
    /// 窗口名称
    pub window_name: Option<String>,
    /// 分区字段
    pub partition_by: Vec<String>,
    /// 排序字段
    pub order_by: Option<OrderByClause>,
    /// 窗口框架
    pub frame_clause: Option<String>,
}

/// SQL查询结构
#[derive(Debug, Clone, PartialEq)]
pub struct SqlQuery {
    /// 查询类型
    pub query_type: QueryType,
    /// 要查询的主表名
    pub table_name: String,
    /// 主表别名
    pub table_alias: Option<String>,
    /// JOIN子句列表
    pub joins: Vec<JoinClause>,
    /// 要选择的字段列表（支持表达式）
    pub columns: Vec<Expression>,
    /// 是否选择所有字段（*）
    pub select_all: bool,
    /// 是否使用DISTINCT去重
    pub distinct: bool,
    /// 查询条件
    pub where_clause: Option<WhereClause>,
    /// HAVING条件
    pub having_clause: Option<WhereClause>,
    /// 分组条件
    pub group_by: Option<GroupByClause>,
    /// 排序条件
    pub order_by: Option<OrderByClause>,
    /// 结果限制
    pub limit: Option<usize>,
    /// 降采样时间间隔（如"1h"、"5m"）
    pub sample_by: Option<String>,
    /// 缺失数据填充策略
    pub fill_clause: Option<FillClause>,
    /// 窗口函数子句
    pub window_functions: Vec<WindowFunctionClause>,
    /// 要插入的字段列表
    pub insert_columns: Vec<String>,
    /// 要插入的值列表
    pub values: Vec<Vec<Value>>,
    /// 表字段定义（用于CREATE TABLE）：(字段名, 类型, 主键, 非空, 唯一, 自增, 默认值)
    pub table_def: Vec<(String, String, bool, bool, bool, bool, Option<Value>)>,
    /// 主键字段名列表（用于CREATE TABLE，支持复合主键）
    pub primary_key: Option<Vec<String>>,
    /// 索引字段名列表（用于CREATE INDEX，支持组合索引）
    pub index_column: Option<Vec<String>>,
    /// 索引类型（用于CREATE INDEX）
    pub index_type: Option<String>,
    /// 索引参数（用于CREATE INDEX WITH子句）
    pub index_params: HashMap<String, String>,
    /// 索引构建模式（ONLINE/OFFLINE）
    pub index_online: bool,
    /// 更新的字段值对（用于UPDATE）：(字段名, 新值表达式)
    pub update_pairs: Vec<(String, Expression)>,
    /// 是否忽略重复键
    pub ignore_duplicates: bool,
    /// 是否使用IF NOT EXISTS子句
    pub if_not_exists: bool,
    /// 模型文件路径（用于CREATE MODEL）
    pub model_path: String,
    /// 模型输入参数（用于CREATE MODEL）：(参数名, 类型)
    pub model_inputs: Vec<(String, String)>,
    /// 模型输出（用于CREATE MODEL）：(名称, 类型)
    pub model_output: (String, String),
    /// 表配置参数（用于CREATE TABLE WITH CONFIGURATION子句）
    pub table_config: HashMap<String, String>,
}

impl Default for SqlQuery {
    fn default() -> Self {
        Self {
            query_type: QueryType::Select,
            table_name: String::new(),
            table_alias: None,
            columns: Vec::new(),
            select_all: false,
            distinct: false,
            where_clause: None,
            having_clause: None,
            order_by: None,
            group_by: None,
            joins: Vec::new(),
            limit: None,
            sample_by: None,
            fill_clause: None,
            window_functions: Vec::new(),
            insert_columns: Vec::new(),
            values: Vec::new(),
            table_def: Vec::new(),
            primary_key: None,
            index_column: None,
            index_type: None,
            index_params: HashMap::new(),
            index_online: false,
            update_pairs: Vec::new(),
            ignore_duplicates: false,
            if_not_exists: false,
            model_path: String::new(),
            model_inputs: Vec::new(),
            model_output: (String::new(), String::new()),
            table_config: HashMap::new(),
        }
    }
}

#[test]
fn test_parse_composite_primary_key() {
    use super::parse_sql_query;
    let sql = "CREATE TABLE IF NOT EXISTS test_composite_pk (id1 INTEGER, id2 INTEGER, name TEXT, PRIMARY KEY (id1, id2))";
    let result = parse_sql_query(sql);
    assert!(result.is_ok());
    let query = result.unwrap();
    assert!(query.primary_key.is_some());
    let pk = query.primary_key.unwrap();
    assert_eq!(pk, vec!["id1", "id2"]);
}

/// 索引类型枚举（用于CREATE INDEX语句）
#[derive(Debug, Clone, PartialEq)]
pub enum IndexType {
    /// B-Tree索引
    BTree,
    /// HNSW向量索引
    HNSW,
    /// IVF_FLAT向量索引
    IVF,
    /// 默认索引类型
    Default,
}

impl std::fmt::Display for IndexType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IndexType::BTree => write!(f, "BTree"),
            IndexType::HNSW => write!(f, "HNSW"),
            IndexType::IVF => write!(f, "IVF"),
            IndexType::Default => write!(f, "Default"),
        }
    }
}

/// 查询类型
    #[derive(Debug, Clone, PartialEq)]
    pub enum QueryType {
        /// SELECT查询
        Select,
        /// INSERT查询
        Insert,
        /// UPDATE查询
        Update,
        /// DELETE查询
        Delete,
        /// DESCRIBE TABLE查询
        Describe,
        /// CREATE TABLE查询
        CreateTable,
        /// CREATE TIMESERIES TABLE查询
        CreateTimeSeriesTable,
        /// CREATE INDEX查询
        CreateIndex,
        /// CREATE DATABASE查询
        CreateDatabase,
        /// CREATE MODEL查询
        CreateModel,
        /// CREATE ROLE查询
        CreateRole,
        /// GRANT PERMISSION查询
        GrantPermission,
        /// GRANT ROLE查询
        GrantRole,
        /// REVOKE PERMISSION查询
        RevokePermission,
        /// REVOKE ROLE查询
        RevokeRole,
        /// DROP ROLE查询
        DropRole,
        /// CREATE USER查询
        CreateUser,
        /// DROP USER查询
        DropUser,
        /// USE DATABASE查询
        UseDatabase,
        /// CLOSE DATABASE查询
        CloseDatabase,
        /// DROP DATABASE查询
        DropDatabase,
        /// ALTER TABLE查询
        AlterTable,
        /// DROP TABLE查询
        DropTable,
        /// BEGIN TRANSACTION查询
        BeginTransaction,
        /// COMMIT查询
        Commit,
        /// ROLLBACK查询
        Rollback,
        /// CREATE CHECKPOINT查询
        CreateCheckpoint,
        /// SHOW INDEX BUILD STATUS查询
        ShowIndexBuildStatus,
        /// REINDEX查询
        Reindex,
        /// SHOW TABLES查询
        ShowTables,
        /// 其他查询类型（暂不支持）
        Other,
    }

/// WHERE子句
#[derive(Debug, Clone, PartialEq)]
pub struct WhereClause {
    /// 条件表达式
    pub condition: Condition,
}

/// 条件表达式
#[derive(Debug, Clone, PartialEq)]
pub enum Condition {
    /// 比较条件
    Comparison(ComparisonCondition),
    /// BETWEEN条件
    Between(BetweenCondition),
    /// AND条件组合
    And(Box<Condition>, Box<Condition>),
    /// OR条件组合
    Or(Box<Condition>, Box<Condition>),
    /// NOT条件
    Not(Box<Condition>),
}

/// 时序数据插值策略
#[derive(Debug, Clone, PartialEq)]
pub enum FillClause {
    /// 使用前一个值填充
    Prev,
    /// 使用线性插值填充
    Linear,
    /// 使用后一个值填充
    Next,
    /// 使用固定值填充
    FixedValue(f64),
}

/// BETWEEN条件
#[derive(Debug, Clone, PartialEq)]
pub struct BetweenCondition {
    /// 字段名
    pub field: String,
    /// 最小值
    pub min_value: Value,
    /// 最大值
    pub max_value: Value,
}

/// 比较条件
#[derive(Debug, Clone, PartialEq)]
pub struct ComparisonCondition {
    /// 字段名
    pub field: String,
    /// 比较运算符
    pub operator: ComparisonOperator,
    /// 比较值
    pub value: Value,
}

/// 比较运算符
#[derive(Debug, Clone, PartialEq)]
pub enum ComparisonOperator {
    /// 等于
    Equal,
    /// 不等于
    NotEqual,
    /// 大于
    GreaterThan,
    /// 大于等于
    GreaterThanOrEqual,
    /// 小于
    LessThan,
    /// 小于等于
    LessThanOrEqual,
    /// 包含（LIKE，暂不支持）
    Like,
}

/// ORDER BY子句
#[derive(Debug, Clone, PartialEq)]
pub struct OrderByClause {
    /// 排序字段
    pub field: String,
    /// 排序方向
    pub direction: OrderDirection,
}

/// 排序方向
#[derive(Debug, Clone, PartialEq)]
pub enum OrderDirection {
    /// 升序
    Ascending,
    /// 降序
    Descending,
}

/// SQL表达式
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    /// 字段引用
    Field {
        /// 字段名
        name: String,
        /// 别名
        alias: Option<String>,
    },
    /// 函数调用
    FunctionCall {
        /// 函数名
        name: String,
        /// 函数参数
        args: Vec<Expression>,
        /// 别名
        alias: Option<String>,
    },
    /// 常量值
    Constant {
        /// 常量值
        value: Value,
        /// 别名
        alias: Option<String>,
    },
    /// 二元操作
    BinaryOp {
        /// 左操作数
        left: Box<Expression>,
        /// 操作符
        op: BinaryOperator,
        /// 右操作数
        right: Box<Expression>,
        /// 别名
        alias: Option<String>,
    },
    /// 逻辑操作
    LogicalOp {
        /// 左操作数
        left: Box<Expression>,
        /// 操作符
        op: LogicalOperator,
        /// 右操作数
        right: Box<Expression>,
        /// 别名
        alias: Option<String>,
    },
    /// 一元操作
    UnaryOp {
        /// 操作符
        op: UnaryOperator,
        /// 操作数
        operand: Box<Expression>,
        /// 别名
        alias: Option<String>,
    },
}

/// 一元操作符
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnaryOperator {
    /// 逻辑非
    Not,
    /// 负号
    Minus,
    /// 正号
    Plus,
}

/// 逻辑操作符
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogicalOperator {
    /// 逻辑与
    And,
    /// 逻辑或
    Or,
}

/// 二元操作符
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinaryOperator {
    /// 加法
    Add,
    /// 减法
    Subtract,
    /// 乘法
    Multiply,
    /// 除法
    Divide,
    /// 等于
    Equal,
    /// 不等于
    NotEqual,
    /// 大于
    GreaterThan,
    /// 大于等于
    GreaterThanOrEqual,
    /// 小于
    LessThan,
    /// 小于等于
    LessThanOrEqual,
    /// 向量L2距离 (<->)
    VectorL2,
    /// 向量内积 (<#>)
    VectorIP,
    /// 向量余弦相似度 (<=>)
    VectorCosine,
}

/// 值类型
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// 整数
    Integer(i64),
    /// 浮点数
    Float(f64),
    /// 字符串
    String(String),
    /// 布尔值
    Boolean(bool),
    /// NULL值
    Null,
    /// 标识符（字段名、表名等）
    Identifier(String),
    /// JSON值
    Json(String),
}

/// 查询解析错误
#[derive(Debug, Clone, PartialEq)]
pub enum QueryParseError {
    /// 无效的SQL语法
    InvalidSyntax,
    /// 不支持的关键字
    UnsupportedKeyword,
    /// 无效的表名
    InvalidTableName,
    /// 无效的字段名
    InvalidFieldName,
    /// 无效的条件
    InvalidCondition,
    /// 无效的运算符
    InvalidOperator,
    /// 无效的值
    InvalidValue,
    /// 缺少必要的子句
    MissingClause,
}

impl core::fmt::Display for QueryParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            QueryParseError::InvalidSyntax => write!(f, "Invalid SQL syntax"),
            QueryParseError::UnsupportedKeyword => write!(f, "Unsupported SQL keyword"),
            QueryParseError::InvalidTableName => write!(f, "Invalid table name"),
            QueryParseError::InvalidFieldName => write!(f, "Invalid field name"),
            QueryParseError::InvalidCondition => write!(f, "Invalid condition"),
            QueryParseError::InvalidOperator => write!(f, "Invalid operator"),
            QueryParseError::InvalidValue => write!(f, "Invalid value"),
            QueryParseError::MissingClause => write!(f, "Missing required clause"),
        }
    }
}

impl From<RemDbError> for QueryParseError {
    fn from(_: RemDbError) -> Self {
        QueryParseError::InvalidSyntax
    }
}

impl core::error::Error for QueryParseError {}

/// SQL查询解析器
pub struct SqlParser {
    /// 输入字符串
    input: String,
    /// 当前位置
    position: usize,
    /// 当前行号
    line: usize,
    /// 当前列号
    column: usize,
}

impl SqlParser {
    /// 创建新的解析器
    pub fn new(input: String) -> Self {
        SqlParser {
            input,
            position: 0,
            line: 1,
            column: 1,
        }
    }

    /// 解析带引号的字符串
    fn parse_string(&mut self) -> Result<String, QueryParseError> {
        if let Some(quote_char) = self.peek_char() {
            if quote_char == '"' || quote_char == '\'' {
                self.next_char(); // 跳过引号
                let mut string_value = String::new();

                while let Some(c) = self.next_char() {
                    if c == quote_char {
                        break;
                    }
                    string_value.push(c);
                }

                Ok(string_value)
            } else {
                Err(QueryParseError::InvalidSyntax)
            }
        } else {
            Err(QueryParseError::InvalidSyntax)
        }
    }

    /// 解析SQL查询
    pub fn parse(&mut self) -> Result<SqlQuery, QueryParseError> {
        self.skip_whitespace();

        // 解析查询类型
        let query_type = self.parse_query_type()?;

        let query = match query_type {
            QueryType::Select => self.parse_select_query(),
            QueryType::Insert => self.parse_insert_query(),
            QueryType::Update => self.parse_update_query(),
            QueryType::Delete => self.parse_delete_query(),
            QueryType::Describe => self.parse_describe_query(),
            QueryType::CreateTable => self.parse_create_table_query(),
            QueryType::CreateTimeSeriesTable => {
                let mut query = self.parse_create_table_query()?;
                query.query_type = QueryType::CreateTimeSeriesTable;
                Ok(query)
            }
            QueryType::CreateIndex => self.parse_create_index_query(),
            QueryType::CreateDatabase => self.parse_create_database_query(),
            QueryType::CreateModel => self.parse_create_model_query(),
            QueryType::CreateRole => self.parse_create_role_query(),
            QueryType::CreateUser => self.parse_create_user_query(),
            QueryType::GrantPermission => self.parse_grant_permission_query(),
            QueryType::GrantRole => self.parse_grant_role_query(),
            QueryType::RevokePermission => self.parse_revoke_permission_query(),
            QueryType::RevokeRole => self.parse_revoke_role_query(),
            QueryType::DropRole => self.parse_drop_role_query(),
            QueryType::DropUser => self.parse_drop_user_query(),
            QueryType::UseDatabase => self.parse_use_database_query(),
            QueryType::CloseDatabase => self.parse_close_database_query(),
            QueryType::DropDatabase => self.parse_drop_database_query(),
            QueryType::AlterTable => self.parse_alter_table_query(),
            QueryType::DropTable => self.parse_drop_table_query(),
            QueryType::BeginTransaction => Ok(SqlQuery {
            query_type,
            table_name: String::new(),
            table_alias: None,
            joins: Vec::new(),
            columns: Vec::new(),
            select_all: false,
            distinct: false,
            where_clause: None,
            having_clause: None,
            group_by: None,
            order_by: None,
            limit: None,
            sample_by: None,
            fill_clause: None,
            window_functions: Vec::new(),
            insert_columns: Vec::new(),
            values: Vec::new(),
            table_def: Vec::new(),
            primary_key: None,
            index_column: None,
            index_type: None,
            index_params: HashMap::new(),
            index_online: true,
            update_pairs: Vec::new(),
            ignore_duplicates: false,
            if_not_exists: false,
            model_path: String::new(),
            model_inputs: Vec::new(),
            model_output: (String::new(), String::new()),
            table_config: HashMap::new(),
        }),
            QueryType::Commit => Ok(SqlQuery {
                query_type: QueryType::Commit,
                table_name: String::new(),
                table_alias: None,
                joins: Vec::new(),
                columns: Vec::new(),
                select_all: false,
                distinct: false,
                where_clause: None,
                having_clause: None,
                group_by: None,
                order_by: None,
                limit: None,
                sample_by: None,
                fill_clause: None,
                window_functions: Vec::new(),
                insert_columns: Vec::new(),
                values: Vec::new(),
                table_def: Vec::new(),
                primary_key: None,
                index_column: None,
                index_type: None,
                index_params: HashMap::new(),
                index_online: true,
                update_pairs: Vec::new(),
                ignore_duplicates: false,
                if_not_exists: false,
                model_path: String::new(),
                model_inputs: Vec::new(),
                model_output: (String::new(), String::new()),
                table_config: HashMap::new(),
            }),
            QueryType::Rollback => Ok(SqlQuery {
                query_type: QueryType::Rollback,
                table_name: String::new(),
                table_alias: None,
                joins: Vec::new(),
                columns: Vec::new(),
                select_all: false,
                distinct: false,
                where_clause: None,
                having_clause: None,
                group_by: None,
                order_by: None,
                limit: None,
                sample_by: None,
                fill_clause: None,
                window_functions: Vec::new(),
                insert_columns: Vec::new(),
                values: Vec::new(),
                table_def: Vec::new(),
                primary_key: None,
                index_column: None,
                index_type: None,
                index_params: HashMap::new(),
                index_online: true,
                update_pairs: Vec::new(),
                ignore_duplicates: false,
                if_not_exists: false,
                model_path: String::new(),
                model_inputs: Vec::new(),
                model_output: (String::new(), String::new()),
                table_config: HashMap::new(),
            }),
            QueryType::CreateCheckpoint => Ok(SqlQuery {
                query_type: QueryType::CreateCheckpoint,
                table_name: String::new(),
                table_alias: None,
                joins: Vec::new(),
                columns: Vec::new(),
                select_all: false,
                distinct: false,
                where_clause: None,
                having_clause: None,
                group_by: None,
                order_by: None,
                limit: None,
                sample_by: None,
                fill_clause: None,
                window_functions: Vec::new(),
                insert_columns: Vec::new(),
                values: Vec::new(),
                table_def: Vec::new(),
                primary_key: None,
                index_column: None,
                index_type: None,
                index_params: HashMap::new(),
                index_online: true,
                update_pairs: Vec::new(),
                ignore_duplicates: false,
                if_not_exists: false,
                model_path: String::new(),
                model_inputs: Vec::new(),
                model_output: (String::new(), String::new()),
                table_config: HashMap::new(),
            }),
            QueryType::ShowIndexBuildStatus => {
                // 解析可选的 FOR <object_name>
                let mut object_name = String::new();
                self.skip_whitespace();
                if self.match_keyword("FOR") {
                    self.skip_whitespace();
                    object_name = self.parse_identifier()?;
                }
                Ok(SqlQuery {
                    query_type: QueryType::ShowIndexBuildStatus,
                    table_name: object_name,
                    table_alias: None,
                    joins: Vec::new(),
                    columns: Vec::new(),
                    select_all: false,
                    distinct: false,
                    where_clause: None,
                    having_clause: None,
                    group_by: None,
                    order_by: None,
                    limit: None,
                    sample_by: None,
                    fill_clause: None,
                    window_functions: Vec::new(),
                    insert_columns: Vec::new(),
                    values: Vec::new(),
                    table_def: Vec::new(),
                    primary_key: None,
                    index_column: None,
                    index_type: None,
                    index_params: HashMap::new(),
                    index_online: true,
                    update_pairs: Vec::new(),
                    ignore_duplicates: false,
                    if_not_exists: false,
                    model_path: String::new(),
                    model_inputs: Vec::new(),
                    model_output: (String::new(), String::new()),
                    table_config: HashMap::new(),
                })
            },
            QueryType::Reindex => self.parse_reindex_query(),
            QueryType::ShowTables => Ok(SqlQuery {
                query_type: QueryType::ShowTables,
                table_name: String::new(),
                table_alias: None,
                joins: Vec::new(),
                columns: Vec::new(),
                select_all: false,
                distinct: false,
                where_clause: None,
                having_clause: None,
                group_by: None,
                order_by: None,
                limit: None,
                sample_by: None,
                fill_clause: None,
                window_functions: Vec::new(),
                insert_columns: Vec::new(),
                values: Vec::new(),
                table_def: Vec::new(),
                primary_key: None,
                index_column: None,
                index_type: None,
                index_params: HashMap::new(),
                index_online: true,
                update_pairs: Vec::new(),
                ignore_duplicates: false,
                if_not_exists: false,
                model_path: String::new(),
                model_inputs: Vec::new(),
                model_output: (String::new(), String::new()),
                table_config: HashMap::new(),
            }),
            QueryType::Other => Err(QueryParseError::UnsupportedKeyword),
        }?;

        // 处理语句末尾可能存在的分号
        self.skip_whitespace();
        self.match_char(';');

        Ok(query)
    }

    /// 解析UPDATE查询
    fn parse_update_query(&mut self) -> Result<SqlQuery, QueryParseError> {
        // 解析表名
        self.skip_whitespace();
        let table_name = self.parse_identifier()?;

        // 解析SET关键字
        self.skip_whitespace();
        self.expect_keyword("SET")?;

        // 解析SET子句
        let mut update_pairs = Vec::new();

        loop {
            self.skip_whitespace();
            let field_name = self.parse_identifier()?;

            self.skip_whitespace();
            self.expect_char('=')?;

            self.skip_whitespace();
            let value_expr = self.parse_expression()?;

            update_pairs.push((field_name, value_expr));

            self.skip_whitespace();
            if self.match_char(',') {
                continue;
            } else {
                break;
            }
        }

        // 解析WHERE子句（可选）
        let where_clause = self.parse_where_clause()?;

        Ok(SqlQuery {
            query_type: QueryType::Update,
            table_name,
            table_alias: None,
            joins: Vec::new(),
            columns: Vec::new(),
            select_all: false,
            distinct: false,
            where_clause,
            group_by: None,
            order_by: None,
            limit: None,
            sample_by: None,
            fill_clause: None,
            insert_columns: Vec::new(),
            values: Vec::new(),
            table_def: Vec::new(),
            primary_key: None,
            index_column: None,
            index_type: None,
            index_params: HashMap::new(),
            index_online: true,
            update_pairs,
            ignore_duplicates: false,
            if_not_exists: false,
            ..Default::default()
        })
    }

    /// 解析DESCRIBE TABLE查询
    fn parse_describe_query(&mut self) -> Result<SqlQuery, QueryParseError> {
        // 解析TABLE关键字（可选，支持DESCRIBE table_name和DESCRIBE TABLE table_name两种语法）
        self.skip_whitespace();
        self.match_keyword("TABLE");

        // 解析表名
        self.skip_whitespace();
        let table_name = self.parse_identifier()?;

        Ok(SqlQuery {
            query_type: QueryType::Describe,
            table_name,
            table_alias: None,
            joins: Vec::new(),
            columns: Vec::new(),
            select_all: false,
            distinct: false,
            where_clause: None,
            group_by: None,
            order_by: None,
            limit: None,
            sample_by: None,
            fill_clause: None,
            insert_columns: Vec::new(),
            values: Vec::new(),
            table_def: Vec::new(),
            primary_key: None,
            index_column: None,
            index_type: None,
            index_params: HashMap::new(),
            index_online: true,
            update_pairs: Vec::new(),
            ignore_duplicates: false,
            if_not_exists: false,
            ..Default::default()
        })
    }

    /// 解析INSERT查询
    fn parse_insert_query(&mut self) -> Result<SqlQuery, QueryParseError> {
        #[cfg(feature = "log")]
        debug!("parse_insert_query called");
        // 检查是否有IGNORE关键字
        let mut ignore_duplicates = false;
        self.skip_whitespace();
        if self.match_keyword("IGNORE") {
            ignore_duplicates = true;
        }

        // 解析INTO关键字
        self.skip_whitespace();
        self.expect_keyword("INTO")?;

        // 解析表名
        self.skip_whitespace();
        let table_name = self.parse_identifier()?;

        // 解析插入的字段列表（可选）
        let insert_columns = self.parse_insert_columns()?;

        // 解析VALUES关键字
        self.skip_whitespace();
        self.expect_keyword("VALUES")?;

        // 解析值列表
        let values = self.parse_values()?;

        Ok(SqlQuery {
            query_type: QueryType::Insert,
            table_name,
            table_alias: None,
            joins: Vec::new(),
            columns: Vec::new(),
            select_all: false,
            distinct: false,
            where_clause: None,
            having_clause: None,
            group_by: None,
            order_by: None,
            limit: None,
            sample_by: None,
            fill_clause: None,
            window_functions: Vec::new(),
            insert_columns,
            values,
            table_def: Vec::new(),
            primary_key: None,
            index_column: None,
            index_type: None,
            index_params: HashMap::new(),
            index_online: true,
            update_pairs: Vec::new(),
            ignore_duplicates,
            if_not_exists: false,
            model_path: String::new(),
            model_inputs: Vec::new(),
            model_output: (String::new(), String::new()),
            table_config: HashMap::new(),
        })
    }

    /// 解析插入的字段列表
    fn parse_insert_columns(&mut self) -> Result<Vec<String>, QueryParseError> {
        self.skip_whitespace();

        if self.match_char('(') {
            // 解析字段列表
            let mut columns = Vec::new();

            loop {
                self.skip_whitespace();
                let column = self.parse_identifier()?;
                columns.push(column);

                self.skip_whitespace();
                if self.match_char(')') {
                    break;
                }

                if !self.match_char(',') {
                    return Err(QueryParseError::InvalidSyntax);
                }
            }

            Ok(columns)
        } else {
            // 没有指定字段，返回空列表
            Ok(Vec::new())
        }
    }

    /// 解析值列表
    fn parse_values(&mut self) -> Result<Vec<Vec<Value>>, QueryParseError> {
        #[cfg(feature = "log")]
        debug!("parse_values called");
        let mut all_values = Vec::new();

        loop {
            self.skip_whitespace();

            // 解析一个值列表
            if !self.match_char('(') {
                return Err(QueryParseError::InvalidSyntax);
            }

            let mut values = Vec::new();

            loop {
                self.skip_whitespace();
                #[cfg(feature = "log")]
                debug!("About to call parse_value");
                let value = self.parse_value()?;
                #[cfg(feature = "log")]
                debug!("parse_value returned: {:?}", value);
                values.push(value);

                self.skip_whitespace();
                if self.match_char(')') {
                    break;
                }

                if !self.match_char(',') {
                    return Err(QueryParseError::InvalidSyntax);
                }
            }

            all_values.push(values);
            self.skip_whitespace();
            if !self.match_char(',') {
                break;
            }
        }

        #[cfg(feature = "log")]
        debug!("parse_values returning {:?} values", all_values.len());
        Ok(all_values)
    }

    /// 解析DELETE查询
    fn parse_delete_query(&mut self) -> Result<SqlQuery, QueryParseError> {
        // 解析FROM关键字
        self.skip_whitespace();
        self.expect_keyword("FROM")?;

        // 解析表名
        self.skip_whitespace();
        let table_name = self.parse_identifier()?;

        // 解析WHERE子句（可选）
        let where_clause = self.parse_where_clause()?;

        Ok(SqlQuery {
            query_type: QueryType::Delete,
            table_name,
            table_alias: None,
            joins: Vec::new(),
            columns: Vec::new(),
            select_all: false,
            distinct: false,
            where_clause,
            having_clause: None,
            group_by: None,
            order_by: None,
            limit: None,
            sample_by: None,
            fill_clause: None,
            window_functions: Vec::new(),
            insert_columns: Vec::new(),
            values: Vec::new(),
            table_def: Vec::new(),
            primary_key: None,
            index_column: None,
            index_type: None,
            index_params: HashMap::new(),
            index_online: true,
            update_pairs: Vec::new(),
            ignore_duplicates: false,
            if_not_exists: false,
            model_path: String::new(),
            model_inputs: Vec::new(),
            model_output: (String::new(), String::new()),
            table_config: HashMap::new(),
        })
    }

    /// 解析CREATE TABLE查询
    fn parse_create_table_query(&mut self) -> Result<SqlQuery, QueryParseError> {
        // 解析IF NOT EXISTS子句
        let mut if_not_exists = false;
        self.skip_whitespace();
        if self.match_keyword("IF") {
            self.skip_whitespace();
            self.expect_keyword("NOT")?;
            self.skip_whitespace();
            self.expect_keyword("EXISTS")?;
            if_not_exists = true;
        }
        
        // 解析表名
        self.skip_whitespace();
        let table_name = self.parse_identifier()?;

        // 解析左括号
        self.skip_whitespace();
        self.expect_char('(')?;

        // 解析字段定义和约束
        let mut table_def = Vec::new();
        let mut primary_key = None;
        let mut primary_key_fields = Vec::new();

        loop {
            self.skip_whitespace();
            
            // 检查是否是PRIMARY KEY约束
            #[cfg(feature = "log")]
            {
                let remaining: String = self.input[self.position..].chars().take(50).collect();
                debug!("parse_create_table_query: position={}, remaining='{}'", self.position, remaining);
            }
            // 调试：打印当前位置和剩余输入
            let remaining_debug: String = self.input[self.position..].chars().take(50).collect();
            let is_primary = self.match_keyword("PRIMARY");
            #[cfg(feature = "log")]
            debug!("parse_create_table_query: match_keyword('PRIMARY')={}, position_after={}", is_primary, self.position);
            if is_primary {
                self.skip_whitespace();
                self.expect_keyword("KEY")?;
                self.skip_whitespace();
                self.expect_char('(')?;
                
                // 解析复合主键字段列表
                loop {
                    self.skip_whitespace();
                    let field_name = self.parse_identifier()?;
                    primary_key_fields.push(field_name);
                    
                    self.skip_whitespace();
                    if self.match_char(')') {
                        break;
                    }
                    
                    if !self.match_char(',') {
                        return Err(QueryParseError::InvalidSyntax);
                    }
                }
                
                primary_key = Some(primary_key_fields.clone());
                
                // 不在这里更新字段定义中的主键标志，因为字段可能还未定义
                // 将在所有字段定义完成后统一处理
            } else if self.match_keyword("FOREIGN") {
                // 跳过 FOREIGN KEY 约束（remdb 不支持外键约束）
                self.skip_whitespace();
                self.expect_keyword("KEY")?;
                self.skip_whitespace();
                self.expect_char('(')?;
                // 跳过字段列表
                loop {
                    self.skip_whitespace();
                    self.parse_identifier()?;
                    self.skip_whitespace();
                    if self.match_char(')') {
                        break;
                    }
                    if !self.match_char(',') {
                        return Err(QueryParseError::InvalidSyntax);
                    }
                }
                // 跳过 REFERENCES 子句
                self.skip_whitespace();
                self.expect_keyword("REFERENCES")?;
                self.skip_whitespace();
                self.parse_identifier()?; // 父表名
                self.skip_whitespace();
                if self.match_char('(') {
                    loop {
                        self.skip_whitespace();
                        self.parse_identifier()?;
                        self.skip_whitespace();
                        if self.match_char(')') {
                            break;
                        }
                        if !self.match_char(',') {
                            return Err(QueryParseError::InvalidSyntax);
                        }
                    }
                }
            } else if self.match_keyword("CONSTRAINT") {
                // 跳过 CONSTRAINT 子句（remdb 不支持）
                self.skip_whitespace();
                // 跳过约束名称
                self.parse_identifier()?;
                self.skip_whitespace();
                // 检查约束类型
                if self.match_keyword("PRIMARY") {
                    self.skip_whitespace();
                    self.expect_keyword("KEY")?;
                    self.skip_whitespace();
                    self.expect_char('(')?;
                    loop {
                        self.skip_whitespace();
                        let field_name = self.parse_identifier()?;
                        primary_key_fields.push(field_name);
                        self.skip_whitespace();
                        if self.match_char(')') {
                            break;
                        }
                        if !self.match_char(',') {
                            return Err(QueryParseError::InvalidSyntax);
                        }
                    }
                    primary_key = Some(primary_key_fields.clone());
                } else if self.match_keyword("UNIQUE") {
                    // 跳过 UNIQUE 约束
                    self.skip_whitespace();
                    self.expect_char('(')?;
                    loop {
                        self.skip_whitespace();
                        self.parse_identifier()?;
                        self.skip_whitespace();
                        if self.match_char(')') {
                            break;
                        }
                        if !self.match_char(',') {
                            return Err(QueryParseError::InvalidSyntax);
                        }
                    }
                } else if self.match_keyword("FOREIGN") {
                    // 跳过 FOREIGN KEY 约束
                    self.skip_whitespace();
                    self.expect_keyword("KEY")?;
                    self.skip_whitespace();
                    self.expect_char('(')?;
                    loop {
                        self.skip_whitespace();
                        self.parse_identifier()?;
                        self.skip_whitespace();
                        if self.match_char(')') {
                            break;
                        }
                        if !self.match_char(',') {
                            return Err(QueryParseError::InvalidSyntax);
                        }
                    }
                    self.skip_whitespace();
                    self.expect_keyword("REFERENCES")?;
                    self.skip_whitespace();
                    self.parse_identifier()?;
                    self.skip_whitespace();
                    if self.match_char('(') {
                        loop {
                            self.skip_whitespace();
                            self.parse_identifier()?;
                            self.skip_whitespace();
                            if self.match_char(')') {
                                break;
                            }
                            if !self.match_char(',') {
                                return Err(QueryParseError::InvalidSyntax);
                            }
                        }
                    }
                } else {
                    return Err(QueryParseError::InvalidSyntax);
                }
            } else {
                // 解析普通字段定义
                let field_name = self.parse_identifier()?;

                self.skip_whitespace();
                // 解析数据类型，支持复杂类型如 VARCHAR(255), INT UNSIGNED
                let data_type = self.parse_data_type()?.to_uppercase();

                // 初始化约束标志
                let mut is_primary_key = false;
                let mut is_not_null = false;
                let mut is_unique = false;
                let mut is_auto_increment = false;
                let mut default_value: Option<Value> = None;

                // 检查约束条件
                loop {
                    self.skip_whitespace();

                    if self.match_keyword("PRIMARY") {
                        self.skip_whitespace();
                        self.expect_keyword("KEY")?;
                        is_primary_key = true;
                        primary_key_fields.push(field_name.clone());
                        primary_key = Some(primary_key_fields.clone());
                    } else if self.match_keyword("NOT") {
                        self.skip_whitespace();
                        self.expect_keyword("NULL")?;
                        is_not_null = true;
                    } else if self.match_keyword("UNIQUE") {
                        is_unique = true;
                    } else if self.match_keyword("AUTOINCREMENT")
                        || self.match_keyword("AUTO_INCREMENT")
                    {
                        is_auto_increment = true;
                    } else if self.match_keyword("DEFAULT") {
                        self.skip_whitespace();
                        let value = self.parse_value()?;
                        default_value = Some(value);
                    } else {
                        // 没有更多约束
                        break;
                    }
                }

                // SQLite兼容：INTEGER PRIMARY KEY自动设为自增
                if data_type == "INTEGER" && is_primary_key {
                    is_auto_increment = true;
                }

                table_def.push((
                    field_name,
                    data_type,
                    is_primary_key,
                    is_not_null,
                    is_unique,
                    is_auto_increment,
                    default_value,
                ));
            }

            self.skip_whitespace();
            if self.match_char(')') {
                break;
            }

            if !self.match_char(',') {
                return Err(QueryParseError::InvalidSyntax);
            }
        }

        // 如果没有显式的主键定义，检查是否有字段标记为主键
        if primary_key.is_none() {
            primary_key = primary_key_fields.into_iter().collect::<Vec<_>>().into();
        }

        // 统一处理主键标志的更新
        if let Some(pk_fields) = &primary_key {
            for (field_name, _, ref mut is_pk, _, _, _, _) in &mut table_def {
                if pk_fields.contains(field_name) {
                    *is_pk = true;
                }
            }
        }

        // 解析WITH CONFIGURATION子句
        let mut table_config = HashMap::new();
        self.skip_whitespace();
        if self.match_keyword("WITH") {
            self.skip_whitespace();
            self.expect_keyword("CONFIGURATION")?;
            self.skip_whitespace();
            self.expect_char('(')?;

            // 解析配置参数
            loop {
                self.skip_whitespace();
                let param_name = self.parse_identifier()?.to_uppercase();

                self.skip_whitespace();
                self.expect_char('=')?;

                self.skip_whitespace();
                let param_value = self.parse_value()?;
                let value_str = match param_value {
                    Value::String(s) => s,
                    Value::Identifier(id) => id,
                    Value::Integer(i) => i.to_string(),
                    Value::Float(f) => f.to_string(),
                    Value::Boolean(b) => b.to_string(),
                    Value::Json(s) => s,
                    _ => return Err(QueryParseError::InvalidValue),
                };

                table_config.insert(param_name, value_str);

                self.skip_whitespace();
                if self.match_char(')') {
                    break;
                } else if !self.match_char(',') {
                    return Err(QueryParseError::InvalidSyntax);
                }
            }
        }

        Ok(SqlQuery {
            query_type: QueryType::CreateTable,
            table_name,
            table_alias: None,
            joins: Vec::new(),
            columns: Vec::new(),
            select_all: false,
            distinct: false,
            where_clause: None,
            having_clause: None,
            group_by: None,
            order_by: None,
            limit: None,
            sample_by: None,
            fill_clause: None,
            window_functions: Vec::new(),
            insert_columns: Vec::new(),
            values: Vec::new(),
            table_def,
            primary_key,
            index_column: None,
            index_type: None,
            index_params: HashMap::new(),
            index_online: true,
            update_pairs: Vec::new(),
            ignore_duplicates: false,
            if_not_exists,
            model_path: String::new(),
            model_inputs: Vec::new(),
            model_output: (String::new(), String::new()),
            table_config,
        })
    }

    /// 解析CREATE INDEX查询
    fn parse_create_index_query(&mut self) -> Result<SqlQuery, QueryParseError> {
        // 解析索引名称（可选）
        self.skip_whitespace();
        let _index_name = self.parse_identifier()?;

        // 解析ON关键字
        self.skip_whitespace();
        self.expect_keyword("ON")?;

        // 解析表名
        self.skip_whitespace();
        let table_name = self.parse_identifier()?;

        // 解析左括号
        self.skip_whitespace();
        self.expect_char('(')?;

        // 解析索引字段列表（支持组合索引）
        let mut index_columns = Vec::new();
        loop {
            self.skip_whitespace();
            let column_name = self.parse_identifier()?;
            index_columns.push(column_name);
            
            // 跳过排序方向（ASC/DESC，可选）
            self.skip_whitespace();
            if self.match_keyword("ASC") || self.match_keyword("DESC") {
                // 目前忽略排序方向，以后可以扩展支持
            }
            
            self.skip_whitespace();
            if self.match_char(')') {
                break;
            }
            
            if !self.match_char(',') {
                return Err(QueryParseError::InvalidSyntax);
            }
        }

        // 解析索引类型（可选）
        let mut index_type = None;
        self.skip_whitespace();
        if self.match_keyword("USING") {
            self.skip_whitespace();
            index_type = Some(self.parse_identifier()?.to_uppercase());
        }

        // 解析索引参数（可选）
        let mut index_params = HashMap::new();
        self.skip_whitespace();
        if self.match_keyword("WITH") {
            self.skip_whitespace();
            self.expect_char('(')?;
            
            // 解析参数列表
            loop {
                self.skip_whitespace();
                let param_name = self.parse_identifier()?.to_uppercase();
                
                self.skip_whitespace();
                self.expect_char('=')?;
                
                self.skip_whitespace();
                let param_value = self.parse_value()?;
                let value_str = match param_value {
                    Value::String(s) => s,
                    Value::Identifier(id) => id,
                    Value::Integer(i) => i.to_string(),
                    Value::Float(f) => f.to_string(),
                    Value::Boolean(b) => b.to_string(),
                    Value::Json(s) => s,
                    _ => return Err(QueryParseError::InvalidValue),
                };
                
                index_params.insert(param_name, value_str);
                
                self.skip_whitespace();
                if self.match_char(',') {
                    continue;
                } else {
                    break;
                }
            }
            
            self.skip_whitespace();
            self.expect_char(')')?;
        }

        // 解析构建模式（ONLINE/OFFLINE，可选）
        let mut index_online = true; // 默认在线构建
        self.skip_whitespace();
        if self.match_keyword("ONLINE") {
            index_online = true;
        } else if self.match_keyword("OFFLINE") {
            index_online = false;
        }

        Ok(SqlQuery {
            query_type: QueryType::CreateIndex,
            table_name,
            table_alias: None,
            joins: Vec::new(),
            columns: Vec::new(),
            select_all: false,
            distinct: false,
            where_clause: None,
            having_clause: None,
            group_by: None,
            order_by: None,
            limit: None,
            sample_by: None,
            fill_clause: None,
            window_functions: Vec::new(),
            insert_columns: Vec::new(),
            values: Vec::new(),
            table_def: Vec::new(),
            primary_key: None,
            index_column: Some(index_columns),
            index_type,
            index_params,
            index_online,
            update_pairs: Vec::new(),
            ignore_duplicates: false,
            if_not_exists: false,
            model_path: String::new(),
            model_inputs: Vec::new(),
            model_output: (String::new(), String::new()),
            table_config: HashMap::new(),
        })
    }

    /// 解析REINDEX查询
    fn parse_reindex_query(&mut self) -> Result<SqlQuery, QueryParseError> {
        // 解析索引名称
        self.skip_whitespace();
        let index_name = self.parse_identifier()?;

        // 解析构建模式（ONLINE/OFFLINE，可选）
        let mut index_online = true; // 默认在线构建
        self.skip_whitespace();
        if self.match_keyword("ONLINE") {
            index_online = true;
        } else if self.match_keyword("OFFLINE") {
            index_online = false;
        }

        Ok(SqlQuery {
            query_type: QueryType::Reindex,
            table_name: index_name,
            table_alias: None,
            joins: Vec::new(),
            columns: Vec::new(),
            select_all: false,
            distinct: false,
            where_clause: None,
            having_clause: None,
            group_by: None,
            order_by: None,
            limit: None,
            sample_by: None,
            fill_clause: None,
            window_functions: Vec::new(),
            insert_columns: Vec::new(),
            values: Vec::new(),
            table_def: Vec::new(),
            primary_key: None,
            index_column: None,
            index_type: None,
            index_params: HashMap::new(),
            index_online,
            update_pairs: Vec::new(),
            ignore_duplicates: false,
            if_not_exists: false,
            model_path: String::new(),
            model_inputs: Vec::new(),
            model_output: (String::new(), String::new()),
            table_config: HashMap::new(),
        })
    }

    /// 解析ALTER TABLE查询
    fn parse_alter_table_query(&mut self) -> Result<SqlQuery, QueryParseError> {
        // 解析表名
        self.skip_whitespace();
        let table_name = self.parse_identifier()?;

        // 解析ALTER TABLE操作
        self.skip_whitespace();

        // 初始化字段定义列表
        let mut table_def = Vec::new();

        // 解析操作类型
        if self.match_keyword("ADD") {
            self.skip_whitespace();
            if self.match_keyword("COLUMN") {
                self.skip_whitespace();
            }

            // 解析字段定义
            let field_def = self.parse_column_definition()?;
            table_def.push(field_def);
        } else if self.match_keyword("DROP") {
            self.skip_whitespace();
            if self.match_keyword("COLUMN") {
                self.skip_whitespace();
            }

            // 解析要删除的字段名
            let field_name = self.parse_identifier()?;
            // 使用特殊标记表示DROP COLUMN操作
            table_def.push((field_name, "DROP".to_string(), false, false, false, false, None));
        } else if self.match_keyword("MODIFY") {
            self.skip_whitespace();
            if self.match_keyword("COLUMN") {
                self.skip_whitespace();
            }

            // 解析字段定义
            let field_def = self.parse_column_definition()?;
            table_def.push(field_def);
        } else if self.match_keyword("RENAME") {
            self.skip_whitespace();
            if self.match_keyword("COLUMN") {
                self.skip_whitespace();
            }

            // 解析旧字段名
            let old_name = self.parse_identifier()?;

            self.skip_whitespace();
            self.expect_keyword("TO")?;

            self.skip_whitespace();
            // 解析新字段名
            let new_name = self.parse_identifier()?;
            // 使用特殊标记表示RENAME COLUMN操作
            table_def.push((old_name, new_name, false, false, false, false, None));
        } else {
            return Err(QueryParseError::UnsupportedKeyword);
        }

        Ok(SqlQuery {
            query_type: QueryType::AlterTable,
            table_name,
            table_alias: None,
            joins: Vec::new(),
            columns: Vec::new(),
            select_all: false,
            distinct: false,
            where_clause: None,
            having_clause: None,
            group_by: None,
            order_by: None,
            limit: None,
            sample_by: None,
            fill_clause: None,
            window_functions: Vec::new(),
            insert_columns: Vec::new(),
            values: Vec::new(),
            table_def,
            primary_key: None,
            index_column: None,
            index_type: None,
            index_params: std::collections::HashMap::new(),
            index_online: true,
            update_pairs: Vec::new(),
            ignore_duplicates: false,
            if_not_exists: false,
            model_path: String::new(),
            model_inputs: Vec::new(),
            model_output: (String::new(), String::new()),
            table_config: HashMap::new(),
        })
    }

    /// 解析列定义
    fn parse_column_definition(&mut self) -> Result<(String, String, bool, bool, bool, bool, Option<Value>), QueryParseError> {
        let field_name = self.parse_identifier()?;

        self.skip_whitespace();
        // 解析数据类型，支持复杂类型如 VARCHAR(255), INT UNSIGNED
        let data_type = self.parse_data_type()?.to_uppercase();

        // 初始化约束标志
        let mut is_primary_key = false;
        let mut is_not_null = false;
        let mut is_unique = false;
        let mut is_auto_increment = false;
        let mut default_value: Option<Value> = None;

        // 检查约束条件
        loop {
            self.skip_whitespace();

            if self.match_keyword("PRIMARY") {
                self.skip_whitespace();
                self.expect_keyword("KEY")?;
                is_primary_key = true;
            } else if self.match_keyword("NOT") {
                self.skip_whitespace();
                self.expect_keyword("NULL")?;
                is_not_null = true;
            } else if self.match_keyword("UNIQUE") {
                is_unique = true;
            } else if self.match_keyword("AUTOINCREMENT")
                || self.match_keyword("AUTO_INCREMENT")
            {
                is_auto_increment = true;
            } else if self.match_keyword("DEFAULT") {
                self.skip_whitespace();
                let value = self.parse_value()?;
                default_value = Some(value);
            } else {
                // 没有更多约束
                break;
            }
        }

        Ok((field_name, data_type, is_primary_key, is_not_null, is_unique, is_auto_increment, default_value))
    }

    /// 解析DROP TABLE查询
    fn parse_drop_table_query(&mut self) -> Result<SqlQuery, QueryParseError> {
        // 解析IF EXISTS选项
        let mut if_exists = false;
        self.skip_whitespace();
        if self.match_keyword("IF") {
            self.skip_whitespace();
            self.expect_keyword("EXISTS")?;
            if_exists = true;
        }

        // 解析表名
        self.skip_whitespace();
        let table_name = self.parse_identifier()?;

        // 解析CASCADE | RESTRICT选项或DEFERRED选项
        self.skip_whitespace();
        let mut is_deferred = false;
        if self.match_keyword("DEFERRED") {
            is_deferred = true;
        } else if self.match_keyword("CASCADE") {
            // 一期实现仅标记，详细功能随未来依赖特性一同实现
        } else if self.match_keyword("RESTRICT") {
            // 默认行为，不需要特殊处理
        }

        // 创建SqlQuery对象
        let mut query = SqlQuery {
            query_type: QueryType::DropTable,
            table_name,
            table_alias: None,
            joins: Vec::new(),
            columns: Vec::new(),
            select_all: false,
            distinct: false,
            where_clause: None,
            having_clause: None,
            group_by: None,
            order_by: None,
            limit: None,
            sample_by: None,
            fill_clause: None,
            window_functions: Vec::new(),
            insert_columns: Vec::new(),
            values: Vec::new(),
            table_def: Vec::new(),
            primary_key: None,
            index_column: None,
            index_type: None,
            index_params: HashMap::new(),
            index_online: true,
            update_pairs: Vec::new(),
            ignore_duplicates: false,
            if_not_exists: false,
            model_path: String::new(),
            model_inputs: Vec::new(),
            model_output: (String::new(), String::new()),
            table_config: HashMap::new(),
        };

        // 使用table_def字段存储额外信息
        // 格式：(if_exists, is_deferred, 0, 0, 0, 0, None)
        query.table_def.push((if_exists.to_string(), is_deferred.to_string(), false, false, false, false, None));

        Ok(query)
    }

    /// 解析CREATE DATABASE查询
    fn parse_create_database_query(&mut self) -> Result<SqlQuery, QueryParseError> {
        // 解析IF NOT EXISTS选项
        let mut if_not_exists = false;
        self.skip_whitespace();
        if self.match_keyword("IF") {
            self.skip_whitespace();
            self.expect_keyword("NOT")?;
            self.skip_whitespace();
            self.expect_keyword("EXISTS")?;
            if_not_exists = true;
        }

        // 解析数据库名称
        self.skip_whitespace();
        let database_name = self.parse_identifier()?;

        // 解析USING SCHEMA子句
        let mut schema = None;
        self.skip_whitespace();
        if self.match_keyword("USING") {
            self.skip_whitespace();
            self.expect_keyword("SCHEMA")?;
            self.skip_whitespace();
            schema = Some(self.parse_identifier()?);
        }

        // 解析WITH CONFIGURATION子句
        let mut config_params = HashMap::new();
        self.skip_whitespace();
        if self.match_keyword("WITH") {
            self.skip_whitespace();
            self.expect_keyword("CONFIGURATION")?;
            self.skip_whitespace();
            self.expect_char('(')?;

            // 解析配置参数
            loop {
                self.skip_whitespace();
                let param_name = self.parse_identifier()?.to_uppercase();
                
                self.skip_whitespace();
                self.expect_char('=')?;
                
                self.skip_whitespace();
                let param_value = self.parse_value()?;
                let value_str = match param_value {
                    Value::String(s) => s,
                    Value::Identifier(id) => id,
                    Value::Integer(i) => i.to_string(),
                    Value::Float(f) => f.to_string(),
                    Value::Boolean(b) => b.to_string(),
                    Value::Json(s) => s,
                    _ => return Err(QueryParseError::InvalidValue),
                };
                
                config_params.insert(param_name, value_str);
                
                self.skip_whitespace();
                if self.match_char(')') {
                    break;
                } else if !self.match_char(',') {
                    return Err(QueryParseError::InvalidSyntax);
                }
            }
        }

        // 创建SqlQuery对象
        let query = SqlQuery {
            query_type: QueryType::CreateDatabase,
            table_name: database_name,
            table_alias: schema,
            joins: Vec::new(),
            columns: Vec::new(),
            select_all: false,
            distinct: false,
            where_clause: None,
            having_clause: None,
            group_by: None,
            order_by: None,
            limit: None,
            sample_by: None,
            fill_clause: None,
            window_functions: Vec::new(),
            insert_columns: Vec::new(),
            values: Vec::new(),
            table_def: Vec::new(),
            primary_key: None,
            index_column: None,
            index_type: None,
            index_params: config_params,
            index_online: true,
            update_pairs: Vec::new(),
            ignore_duplicates: if_not_exists,
            if_not_exists: false,
            model_path: String::new(),
            model_inputs: Vec::new(),
            model_output: (String::new(), String::new()),
            table_config: HashMap::new(),
        };

        Ok(query)
    }

    /// 解析CREATE MODEL查询
    fn parse_create_model_query(&mut self) -> Result<SqlQuery, QueryParseError> {
        // 解析IF NOT EXISTS子句
        let mut if_not_exists = false;
        self.skip_whitespace();
        if self.match_keyword("IF") {
            self.skip_whitespace();
            self.expect_keyword("NOT")?;
            self.skip_whitespace();
            self.expect_keyword("EXISTS")?;
            if_not_exists = true;
        }
        
        // 解析模型名称
        self.skip_whitespace();
        let model_name = self.parse_identifier()?;

        // 解析USING子句
        self.skip_whitespace();
        self.expect_keyword("USING")?;
        self.skip_whitespace();
        let model_path = self.parse_string()?;

        // 解析AS子句和输入参数
        self.skip_whitespace();
        self.expect_keyword("AS")?;
        self.skip_whitespace();
        self.expect_char('(')?;

        // 解析输入参数
        let mut model_inputs = Vec::new();
        loop {
            self.skip_whitespace();
            let param_name = self.parse_identifier()?;
            self.skip_whitespace();
            let param_type = self.parse_identifier()?.to_uppercase();
            model_inputs.push((param_name, param_type));

            self.skip_whitespace();
            if self.match_char(')') {
                break;
            } else if !self.match_char(',') {
                return Err(QueryParseError::InvalidSyntax);
            }
        }

        // 解析RETURNS子句
        self.skip_whitespace();
        self.expect_keyword("RETURNS")?;
        self.skip_whitespace();
        let return_type = self.parse_identifier()?.to_uppercase();

        Ok(SqlQuery {
            query_type: QueryType::CreateModel,
            table_name: model_name,
            table_alias: None,
            joins: Vec::new(),
            columns: Vec::new(),
            select_all: false,
            distinct: false,
            where_clause: None,
            having_clause: None,
            group_by: None,
            order_by: None,
            limit: None,
            sample_by: None,
            fill_clause: None,
            window_functions: Vec::new(),
            insert_columns: Vec::new(),
            values: Vec::new(),
            table_def: Vec::new(),
            primary_key: None,
            index_column: None,
            index_type: None,
            index_params: HashMap::new(),
            index_online: true,
            update_pairs: Vec::new(),
            ignore_duplicates: false,
            if_not_exists,
            model_path,
            model_inputs,
            model_output: ("result".to_string(), return_type),
            table_config: HashMap::new(),
        })
    }

    /// 解析USE DATABASE查询
    fn parse_use_database_query(&mut self) -> Result<SqlQuery, QueryParseError> {
        // 解析数据库名称
        self.skip_whitespace();
        let database_name = self.parse_identifier()?;

        // 创建SqlQuery对象
        let query = SqlQuery {
            query_type: QueryType::UseDatabase,
            table_name: database_name,
            table_alias: None,
            joins: Vec::new(),
            columns: Vec::new(),
            select_all: false,
            distinct: false,
            where_clause: None,
            having_clause: None,
            group_by: None,
            order_by: None,
            limit: None,
            sample_by: None,
            fill_clause: None,
            window_functions: Vec::new(),
            insert_columns: Vec::new(),
            values: Vec::new(),
            table_def: Vec::new(),
            primary_key: None,
            index_column: None,
            index_type: None,
            index_params: HashMap::new(),
            index_online: true,
            update_pairs: Vec::new(),
            ignore_duplicates: false,
            if_not_exists: false,
            model_path: String::new(),
            model_inputs: Vec::new(),
            model_output: (String::new(), String::new()),
            table_config: HashMap::new(),
        };

        Ok(query)
    }

    /// 解析CLOSE DATABASE查询
    fn parse_close_database_query(&mut self) -> Result<SqlQuery, QueryParseError> {
        // 解析数据库名称
        self.skip_whitespace();
        let database_name = self.parse_identifier()?;

        // 创建SqlQuery对象
        let query = SqlQuery {
            query_type: QueryType::CloseDatabase,
            table_name: database_name,
            table_alias: None,
            joins: Vec::new(),
            columns: Vec::new(),
            select_all: false,
            distinct: false,
            where_clause: None,
            having_clause: None,
            group_by: None,
            order_by: None,
            limit: None,
            sample_by: None,
            fill_clause: None,
            window_functions: Vec::new(),
            insert_columns: Vec::new(),
            values: Vec::new(),
            table_def: Vec::new(),
            primary_key: None,
            index_column: None,
            index_type: None,
            index_params: HashMap::new(),
            index_online: true,
            update_pairs: Vec::new(),
            ignore_duplicates: false,
            if_not_exists: false,
            model_path: String::new(),
            model_inputs: Vec::new(),
            model_output: (String::new(), String::new()),
            table_config: HashMap::new(),
        };

        Ok(query)
    }

    /// 解析DROP DATABASE查询
    fn parse_drop_database_query(&mut self) -> Result<SqlQuery, QueryParseError> {
        // 解析IF EXISTS选项
        let mut if_exists = false;
        self.skip_whitespace();
        if self.match_keyword("IF") {
            self.skip_whitespace();
            self.expect_keyword("EXISTS")?;
            if_exists = true;
        }

        // 解析数据库名称
        self.skip_whitespace();
        let database_name = self.parse_identifier()?;

        // 创建SqlQuery对象
        let query = SqlQuery {
            query_type: QueryType::DropDatabase,
            table_name: database_name,
            table_alias: None,
            joins: Vec::new(),
            columns: Vec::new(),
            select_all: false,
            distinct: false,
            where_clause: None,
            having_clause: None,
            group_by: None,
            order_by: None,
            limit: None,
            sample_by: None,
            fill_clause: None,
            window_functions: Vec::new(),
            insert_columns: Vec::new(),
            values: Vec::new(),
            table_def: Vec::new(),
            primary_key: None,
            index_column: None,
            index_type: None,
            index_params: HashMap::new(),
            index_online: true,
            update_pairs: Vec::new(),
            ignore_duplicates: if_exists,
            if_not_exists: false,
            model_path: String::new(),
            model_inputs: Vec::new(),
            model_output: (String::new(), String::new()),
            table_config: HashMap::new(),
        };

        Ok(query)
    }

    /// 解析查询类型
    fn parse_query_type(&mut self) -> Result<QueryType, QueryParseError> {
        if self.match_keyword("SELECT") {
            Ok(QueryType::Select)
        } else if self.match_keyword("INSERT") {
            Ok(QueryType::Insert)
        } else if self.match_keyword("UPDATE") {
            Ok(QueryType::Update)
        } else if self.match_keyword("DELETE") {
            Ok(QueryType::Delete)
        } else if self.match_keyword("DESCRIBE") {
            Ok(QueryType::Describe)
        } else if self.match_keyword("CREATE") {
            self.skip_whitespace();
            if self.match_keyword("CHECKPOINT") {
                Ok(QueryType::CreateCheckpoint)
            } else if self.match_keyword("TIMESERIES") {
                self.skip_whitespace();
                if self.match_keyword("TABLE") {
                    Ok(QueryType::CreateTimeSeriesTable)
                } else {
                    Ok(QueryType::Other)
                }
            } else if self.match_keyword("TABLE") {
                Ok(QueryType::CreateTable)
            } else if self.match_keyword("INDEX") {
                Ok(QueryType::CreateIndex)
            } else if self.match_keyword("DATABASE") {
                Ok(QueryType::CreateDatabase)
            } else if self.match_keyword("MODEL") {
                Ok(QueryType::CreateModel)
            } else if self.match_keyword("ROLE") {
                Ok(QueryType::CreateRole)
            } else if self.match_keyword("USER") {
                Ok(QueryType::CreateUser)
            } else {
                Ok(QueryType::Other)
            }
        } else if self.match_keyword("ALTER") {
            self.skip_whitespace();
            if self.match_keyword("TABLE") {
                Ok(QueryType::AlterTable)
            } else {
                Ok(QueryType::Other)
            }
        } else if self.match_keyword("DROP") {
            self.skip_whitespace();
            if self.match_keyword("TABLE") {
                Ok(QueryType::DropTable)
            } else if self.match_keyword("DATABASE") {
                Ok(QueryType::DropDatabase)
            } else if self.match_keyword("ROLE") {
                Ok(QueryType::DropRole)
            } else if self.match_keyword("USER") {
                Ok(QueryType::DropUser)
            } else {
                Ok(QueryType::Other)
            }
        } else if self.match_keyword("GRANT") {
            self.skip_whitespace();
            if self.match_keyword("ROLE") {
                Ok(QueryType::GrantRole)
            } else {
                Ok(QueryType::GrantPermission)
            }
        } else if self.match_keyword("REVOKE") {
            self.skip_whitespace();
            if self.match_keyword("ROLE") {
                Ok(QueryType::RevokeRole)
            } else {
                Ok(QueryType::RevokePermission)
            }
        } else if self.match_keyword("USE") {
            self.skip_whitespace();
            if self.match_keyword("DATABASE") {
                Ok(QueryType::UseDatabase)
            } else {
                Ok(QueryType::Other)
            }
        } else if self.match_keyword("CLOSE") {
            self.skip_whitespace();
            if self.match_keyword("DATABASE") {
                Ok(QueryType::CloseDatabase)
            } else {
                Ok(QueryType::Other)
            }
        } else if self.match_keyword("BEGIN") {
            self.skip_whitespace();
            // 支持BEGIN TRANSACTION语法
            self.match_keyword("TRANSACTION");
            Ok(QueryType::BeginTransaction)
        } else if self.match_keyword("COMMIT") {
            Ok(QueryType::Commit)
        } else if self.match_keyword("ROLLBACK") {
            Ok(QueryType::Rollback)
        } else if self.match_keyword("SHOW") {
            self.skip_whitespace();
            if self.match_keyword("INDEX") {
                self.skip_whitespace();
                if self.match_keyword("BUILD") {
                    self.skip_whitespace();
                    if self.match_keyword("STATUS") {
                        // 可选地解析 FOR <object_name>
                        // 注意：这里不消费 FOR 部分，留给 parse_query 处理
                        Ok(QueryType::ShowIndexBuildStatus)
                    } else {
                        Ok(QueryType::Other)
                    }
                } else {
                    Ok(QueryType::Other)
                }
            } else if self.match_keyword("TABLES") {
                Ok(QueryType::ShowTables)
            } else {
                Ok(QueryType::Other)
            }
        } else if self.match_keyword("REINDEX") {
            Ok(QueryType::Reindex)
        } else {
            Ok(QueryType::Other)
        }
    }

    /// 解析HAVING子句（可选）
    fn parse_having_clause(&mut self) -> Result<Option<WhereClause>, QueryParseError> {
        self.skip_whitespace();

        if self.match_keyword("HAVING") {
            self.skip_whitespace();
            let condition = self.parse_condition()?;
            Ok(Some(WhereClause { condition }))
        } else {
            Ok(None)
        }
    }

    /// 解析SELECT查询
    fn parse_select_query(&mut self) -> Result<SqlQuery, QueryParseError> {
        // 解析SELECT子句
        let (columns, select_all, distinct) = self.parse_select_clause()?;

        // 解析FROM子句和JOIN子句
        let (table_name, table_alias, joins) = self.parse_from_and_join_clauses()?;

        // 解析WHERE子句（可选）
        let where_clause = self.parse_where_clause()?;

        // 解析GROUP BY子句（可选）
        let group_by = self.parse_group_by_clause()?;

        // 解析HAVING子句（可选）
        let having_clause = self.parse_having_clause()?;

        // 解析ORDER BY子句（可选）
        let order_by = self.parse_order_by_clause()?;

        // 解析LIMIT子句（可选）
        let limit = self.parse_limit_clause()?;
        
        // 解析SAMPLE BY子句（可选，时序查询专用）
        let sample_by = self.parse_sample_by_clause()?;
        
        // 解析FILL子句（可选，时序查询专用）
        let fill_clause = self.parse_fill_clause()?;

        let mut query = SqlQuery {
            query_type: QueryType::Select,
            table_name,
            table_alias,
            joins,
            columns,
            select_all,
            distinct,
            where_clause,
            having_clause: None,
            group_by,
            order_by,
            limit,
            sample_by,
            fill_clause,
            window_functions: Vec::new(),
            insert_columns: Vec::new(),
            values: Vec::new(),
            table_def: Vec::new(),
            primary_key: None,
            index_column: None,
            index_type: None,
            index_params: std::collections::HashMap::new(),
            index_online: true,
            update_pairs: Vec::new(),
            ignore_duplicates: false,
            if_not_exists: false,
            model_path: String::new(),
            model_inputs: Vec::new(),
            model_output: (String::new(), String::new()),
            table_config: HashMap::new(),
        };

        // 添加HAVING子句支持
        query.having_clause = having_clause;

        Ok(query)
    }

    /// 解析SELECT子句
    fn parse_select_clause(&mut self) -> Result<(Vec<Expression>, bool, bool), QueryParseError> {
        self.skip_whitespace();

        // 检查是否使用DISTINCT
        let distinct = self.match_keyword("DISTINCT");
        self.skip_whitespace();

        // 检查是否选择所有字段（*）
        if self.match_char('*') {
            Ok((Vec::new(), true, distinct))
        } else {
            // 解析表达式列表
            let mut expressions = Vec::new();

            loop {
                self.skip_whitespace();
                let expr = self.parse_expression()?;
                expressions.push(expr);

                self.skip_whitespace();
                if !self.match_char(',') {
                    break;
                }
            }

            Ok((expressions, false, distinct))
        }
    }

    /// 解析表达式
    fn parse_expression(&mut self) -> Result<Expression, QueryParseError> {
        self.skip_whitespace();

        let mut left_expr = self.parse_primary_expression()?;

        // 检查是否有二元操作符
        loop {
            self.skip_whitespace();

            let saved_pos = self.position;
            let saved_col = self.column;

            // 尝试解析二元操作符
            let op = match self.peek_char() {
                Some('+') => {
                    self.next_char();
                    BinaryOperator::Add
                }
                Some('-') => {
                    self.next_char();
                    BinaryOperator::Subtract
                }
                Some('*') => {
                    self.next_char();
                    BinaryOperator::Multiply
                }
                Some('/') => {
                    self.next_char();
                    BinaryOperator::Divide
                }
                Some('<') => {
                    self.next_char();
                    match self.peek_char() {
                        Some('-') => {
                            self.next_char();
                            if self.peek_char() == Some('>') {
                                self.next_char();
                                BinaryOperator::VectorL2 // <->
                            } else {
                                // 回退到小于号
                                self.position = saved_pos;
                                self.column = saved_col;
                                break;
                            }
                        }
                        Some('#') => {
                            self.next_char();
                            if self.peek_char() == Some('>') {
                                self.next_char();
                                BinaryOperator::VectorIP // <#>
                            } else {
                                // 回退到小于号
                                self.position = saved_pos;
                                self.column = saved_col;
                                break;
                            }
                        }
                        Some('=') => {
                            self.next_char();
                            if self.peek_char() == Some('>') {
                                self.next_char();
                                BinaryOperator::VectorCosine // <=>
                            } else {
                                BinaryOperator::LessThanOrEqual
                            }
                        }
                        _ => BinaryOperator::LessThan,
                    }
                }
                Some('>') => {
                    self.next_char();
                    if self.peek_char() == Some('=') {
                        self.next_char();
                        BinaryOperator::GreaterThanOrEqual
                    } else {
                        BinaryOperator::GreaterThan
                    }
                }
                Some('=') => {
                    self.next_char();
                    BinaryOperator::Equal
                }
                Some('!') => {
                    self.next_char();
                    if self.peek_char() == Some('=') {
                        self.next_char();
                        BinaryOperator::NotEqual
                    } else {
                        break;
                    }
                }
                _ => break,
            };

            // 成功解析了操作符，继续解析右操作数
            self.skip_whitespace();
            let right_expr = self.parse_primary_expression()?;

            // 构建新的表达式，将之前的表达式作为左操作数
            left_expr = Expression::BinaryOp {
                left: Box::new(left_expr),
                op,
                right: Box::new(right_expr),
                alias: None,
            };
        }

        // 检查是否有IS NULL或IS NOT NULL语法
        self.skip_whitespace();
        if self.match_keyword("IS") {
            self.skip_whitespace();
            let is_not = self.match_keyword("NOT");
            self.skip_whitespace();
            if self.match_keyword("NULL") {
                // 构建IS NULL或IS NOT NULL表达式
                // 使用BinaryOp来表示IS NULL操作
                let right_expr = Expression::Constant { 
                    value: Value::Null, 
                    alias: None 
                };
                let op = if is_not {
                    BinaryOperator::NotEqual
                } else {
                    BinaryOperator::Equal
                };
                left_expr = Expression::BinaryOp {
                    left: Box::new(left_expr),
                    op,
                    right: Box::new(right_expr),
                    alias: None,
                };
            }
        }

        self.skip_whitespace();
        let alias = self.parse_alias()?;

        // 如果有别名，需要更新表达式的别名
        match left_expr {
            Expression::Field {
                alias: expr_alias,
                name,
                ..
            } => Ok(Expression::Field {
                name,
                alias: alias.or(expr_alias),
            }),
            Expression::FunctionCall {
                alias: expr_alias,
                name,
                args,
                ..
            } => Ok(Expression::FunctionCall {
                name,
                args,
                alias: alias.or(expr_alias),
            }),
            Expression::Constant {
                alias: expr_alias,
                value,
                ..
            } => Ok(Expression::Constant {
                value,
                alias: alias.or(expr_alias),
            }),
            Expression::BinaryOp {
                left, op, right, ..
            } => Ok(Expression::BinaryOp {
                left,
                op,
                right,
                alias,
            }),
            Expression::LogicalOp {
                left, op, right, ..
            } => Ok(Expression::LogicalOp {
                left,
                op,
                right,
                alias,
            }),
            Expression::UnaryOp {
                op, operand, ..
            } => Ok(Expression::UnaryOp {
                op,
                operand,
                alias,
            }),
        }
    }

    /// 解析基本表达式（字段、函数调用、常量、INTERVAL、*）
    fn parse_primary_expression(&mut self) -> Result<Expression, QueryParseError> {
        self.skip_whitespace();

        // 保存当前位置，用于回溯
        let saved_pos = self.position;
        let saved_col = self.column;

        // 检查是否是星号
        if self.match_char('*') {
            // 返回字段表达式，代表所有字段
            return Ok(Expression::Field {
                name: "*".to_string(),
                alias: None,
            });
        }

        // 检查是否是向量字面量
        if self.peek_char() == Some('[') {
            // 尝试解析向量字面量
            if let Ok(value) = self.parse_value() {
                return Ok(Expression::Constant { value, alias: None });
            } else {
                // 解析失败，回退到原始位置
                self.position = saved_pos;
                self.column = saved_col;
            }
        }

        // 检查是否是常量值（数字、字符串、布尔值）
        let current_char = self.peek_char().ok_or(QueryParseError::InvalidSyntax)?;
        if current_char.is_ascii_digit()
            || current_char == '-'
            || current_char == '"'
            || current_char == '\''
            || current_char.is_ascii_alphabetic()
        {
            // 检查是否是布尔值
            let saved_pos_bool = self.position;
            if self.match_keyword("TRUE") {
                return Ok(Expression::Constant { 
                    value: Value::Boolean(true), 
                    alias: None 
                });
            } else if self.match_keyword("FALSE") {
                return Ok(Expression::Constant { 
                    value: Value::Boolean(false), 
                    alias: None 
                });
            } else if self.match_keyword("NULL") {
                return Ok(Expression::Constant { 
                    value: Value::Null, 
                    alias: None 
                });
            }
            // 回退到原始位置
            self.position = saved_pos_bool;
            self.column = saved_col;
            
            // 检查是否是数字
            if current_char.is_ascii_digit() || current_char == '-'
            {
                // 解析常量值
                let value = self.parse_value()?;
                return Ok(Expression::Constant { value, alias: None });
            }
            // 检查是否是字符串
            else if current_char == '"' || current_char == '\''
            {
                // 解析常量值
                let value = self.parse_value()?;
                return Ok(Expression::Constant { value, alias: None });
            }
        }

        // 尝试直接解析函数调用，避免循环调用
        let func_saved_pos = self.position;
        let func_saved_col = self.column;
        
        // 尝试解析函数名
        if let Ok(function_name) = self.parse_identifier() {
            self.skip_whitespace();
            // 检查下一个字符是否是左括号
            if self.peek_char() == Some('(') {
                // 解析左括号
                self.next_char();
                
                // 解析函数参数
                let mut args = Vec::new();

                loop {
                    self.skip_whitespace();

                    if self.peek_char() == Some(')') {
                        break;
                    }

                    // 解析参数表达式
                    let arg_expr = self.parse_expression()?;
                    args.push(arg_expr);

                    self.skip_whitespace();
                    if self.match_char(',') {
                        continue;
                    } else {
                        break;
                    }
                }

                self.skip_whitespace();
                self.expect_char(')')?;

                return Ok(Expression::FunctionCall {
                    name: function_name,
                    args,
                    alias: None,
                });
            }
        }
        
        // 回退到原始位置
        self.position = func_saved_pos;
        self.column = func_saved_col;
        
        // 尝试解析标识符作为字段
        let identifier = self.parse_identifier()?;
        
        // 检查是否是INTERVAL常量
        if identifier.eq_ignore_ascii_case("INTERVAL") {
            // 解析INTERVAL常量
            self.skip_whitespace();

            // 解析间隔值（可能是数字或字符串）
            let interval_value = self.parse_value()?;

            // 检查是否有单位（如HOUR, MINUTE等）
            self.skip_whitespace();
            if let Ok(unit) = self.parse_identifier() {
                // 组合值和单位为字符串，如"1 HOUR"或"30 MINUTE"
                let interval_str = match interval_value {
                    Value::Integer(i) => alloc::format!("{} {}", i, unit),
                    Value::String(s) => alloc::format!("{} {}", s, unit),
                    _ => return Err(QueryParseError::InvalidValue),
                };

                // 将组合后的间隔字符串转换为微秒值
                // 这里我们暂时返回一个占位符，实际解析将在执行时进行
                return Ok(Expression::Constant {
                    value: Value::String(interval_str),
                    alias: None,
                });
            } else {
                // 只有值，没有单位
                return Ok(Expression::Constant {
                    value: interval_value,
                    alias: None,
                });
            }
        } 
        // 不是INTERVAL，直接返回字段表达式
        else {
            return Ok(Expression::Field {
                name: identifier,
                alias: None,
            });
        }
    }

    /// 解析函数调用
    fn parse_function_call(&mut self) -> Result<Expression, QueryParseError> {
        // 解析函数名
        let function_name = self.parse_identifier()?;

        self.skip_whitespace();

        // 检查是否有左括号
        if !self.match_char('(') {
            return Err(QueryParseError::InvalidSyntax);
        }

        // 解析参数列表
        let args = self.parse_function_args()?;

        // 解析右括号
        self.skip_whitespace();
        if !self.match_char(')') {
            return Err(QueryParseError::InvalidSyntax);
        }

        // 解析别名
        let alias = self.parse_alias()?;

        Ok(Expression::FunctionCall {
            name: function_name,
            args,
            alias,
        })
    }

    /// 解析函数参数列表
    fn parse_function_args(&mut self) -> Result<Vec<Expression>, QueryParseError> {
        let mut args = Vec::new();

        self.skip_whitespace();

        // 检查是否是空参数列表
        if self.peek_char() == Some(')') {
            return Ok(args);
        }

        loop {
            // 解析单个参数
            let arg = self.parse_expression()?;
            args.push(arg);

            self.skip_whitespace();

            // 检查是否还有更多参数
            if self.match_char(',') {
                continue;
            } else {
                break;
            }
        }

        Ok(args)
    }

    /// 解析别名
    fn parse_alias(&mut self) -> Result<Option<String>, QueryParseError> {
        self.skip_whitespace();

        // 保存当前位置，用于回溯
        let _saved_pos = self.position;
        let _saved_col = self.column;

        // 检查是否有AS关键字
        if self.match_keyword("AS") {
            self.skip_whitespace();
        }

        // 检查下一个字符是否是关键字
        let next_token = self.peek_identifier();
        if let Some(token) = next_token {
            // 检查是否是关键字
            let token_upper = token.to_uppercase();
            let keywords = [
                "FROM", "WHERE", "ORDER", "LIMIT", "GROUP", "HAVING", "JOIN", "ON", "IN", "AND",
                "OR", "NOT",
            ];
            if keywords.contains(&token_upper.as_str()) {
                // 是关键字，不是别名
                return Ok(None);
            }
        }

        // 检查是否有别名
        if self
            .peek_char()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        {
            let alias = self.parse_identifier()?;
            Ok(Some(alias))
        } else {
            Ok(None)
        }
    }

    /// 解析FROM子句和JOIN子句
    fn parse_from_and_join_clauses(
        &mut self,
    ) -> Result<(String, Option<String>, Vec<JoinClause>), QueryParseError> {
        self.skip_whitespace();

        // 检查是否有FROM关键字（FROM是可选的）
        if !self.match_keyword("FROM") {
            // 没有FROM子句，返回空值
            return Ok((String::new(), None, Vec::new()));
        }

        self.skip_whitespace();
        let table_name = self.parse_identifier()?;

        // 检查表名是否为空（FROM后面没有表名的情况）
        if table_name.is_empty() {
            return Err(QueryParseError::InvalidSyntax);
        }

        // 解析主表别名
        self.skip_whitespace();
        let table_alias = self.parse_alias()?;

        // 解析JOIN子句
        let mut joins = Vec::new();

        loop {
            self.skip_whitespace();

            // 检查是否有JOIN关键字
            let join_type = match self.peek_identifier() {
                Some(token) => {
                    let token_upper = token.to_uppercase();
                    match token_upper.as_str() {
                        "INNER" => {
                            self.parse_identifier()?;
                            self.skip_whitespace();
                            if !self.match_keyword("JOIN") {
                                return Err(QueryParseError::InvalidSyntax);
                            }
                            JoinType::Inner
                        }
                        "LEFT" => {
                            self.parse_identifier()?;
                            self.skip_whitespace();
                            if self.match_keyword("OUTER") {
                                self.skip_whitespace();
                            }
                            if !self.match_keyword("JOIN") {
                                return Err(QueryParseError::InvalidSyntax);
                            }
                            JoinType::Left
                        }
                        "RIGHT" => {
                            self.parse_identifier()?;
                            self.skip_whitespace();
                            if self.match_keyword("OUTER") {
                                self.skip_whitespace();
                            }
                            if !self.match_keyword("JOIN") {
                                return Err(QueryParseError::InvalidSyntax);
                            }
                            JoinType::Right
                        }
                        "FULL" => {
                            self.parse_identifier()?;
                            self.skip_whitespace();
                            if self.match_keyword("OUTER") {
                                self.skip_whitespace();
                            }
                            if !self.match_keyword("JOIN") {
                                return Err(QueryParseError::InvalidSyntax);
                            }
                            JoinType::Full
                        }
                        "JOIN" => {
                            self.parse_identifier()?;
                            JoinType::Inner
                        }
                        _ => break, // 不是JOIN关键字，退出循环
                    }
                }
                None => break, // 没有更多令牌，退出循环
            };

            // 解析连接表名
            self.skip_whitespace();
            let join_table_name = self.parse_identifier()?;

            // 解析连接表别名
            self.skip_whitespace();
            let join_table_alias = self.parse_alias()?;

            // 解析ON关键字
            self.skip_whitespace();
            self.expect_keyword("ON")?;

            // 解析连接条件
            self.skip_whitespace();
            let on_condition = self.parse_condition()?;

            // 创建JOIN子句
            let join_clause = JoinClause {
                join_type,
                table_name: join_table_name,
                table_alias: join_table_alias,
                on_condition,
            };

            joins.push(join_clause);
        }

        Ok((table_name, table_alias, joins))
    }

    /// 解析WHERE子句（可选）
    fn parse_where_clause(&mut self) -> Result<Option<WhereClause>, QueryParseError> {
        self.skip_whitespace();

        if self.match_keyword("WHERE") {
            self.skip_whitespace();
            let condition = self.parse_condition()?;
            Ok(Some(WhereClause { condition }))
        } else {
            Ok(None)
        }
    }

    /// 解析GROUP BY子句（可选）
    fn parse_group_by_clause(&mut self) -> Result<Option<GroupByClause>, QueryParseError> {
        self.skip_whitespace();

        if self.match_keyword("GROUP") {
            self.skip_whitespace();
            self.expect_keyword("BY")?;

            let mut expressions = Vec::new();
            let mut fields = Vec::new();

            loop {
                self.skip_whitespace();
                let expr = self.parse_expression()?;
                expressions.push(expr.clone());

                // 对于简单的字段表达式，同时添加到fields列表中以兼容旧版本
                if let Expression::Field { name, .. } = expr {
                    fields.push(name);
                }

                self.skip_whitespace();
                if !self.match_char(',') {
                    break;
                }
            }

            Ok(Some(GroupByClause {
                expressions,
                fields,
            }))
        } else {
            Ok(None)
        }
    }

}

    /// 将表达式转换为ORDER BY子句用的字符串表示
pub fn expression_to_order_by_string(expr: &Expression) -> String {
    match expr {
        Expression::Field { name, .. } => name.clone(),
        Expression::BinaryOp { left, op, right, .. } => {
            let left_name = match left.as_ref() {
                Expression::Field { name, .. } => name.clone(),
                _ => return String::new(),
            };
            let op_str = match op {
                BinaryOperator::VectorL2 => "<->",
                BinaryOperator::VectorIP => "<#>",
                BinaryOperator::VectorCosine => "<=>",
                _ => return String::new(),
            };
            let right_str = match right.as_ref() {
                Expression::Constant { value, .. } => {
                    match value {
                        crate::sql::Value::Json(json_str) => json_str.clone(),
                        crate::sql::Value::String(s) => s.clone(),
                        _ => format!("{:?}", value),
                    }
                }
                _ => format!("{:?}", right.as_ref()),
            };
            format!("{} {} {}", left_name, op_str, right_str)
        }
        Expression::Constant { value, .. } => format!("{:?}", value),
        _ => format!("{:?}", expr),
    }
}

/// 解析ORDER BY子句（可选）
impl SqlParser {
    fn parse_order_by_clause(&mut self) -> Result<Option<OrderByClause>, QueryParseError> {
        self.skip_whitespace();

        if self.match_keyword("ORDER") {
            self.skip_whitespace();
            self.expect_keyword("BY")?;

            self.skip_whitespace();
            // 解析ORDER BY子句中的字段，可以是位置索引（数字）、字段名或向量表达式
            let field = if let Some(c) = self.peek_char() {
                if c.is_ascii_digit() {
                    // 解析数字作为位置索引
                    self.parse_number()?.to_string()
                } else {
                    // 解析为向量表达式（支持简单字段名和向量距离表达式）
                    let expr = self.parse_vector_expression()?;
                    self::expression_to_order_by_string(&expr)
                }
            } else {
                return Err(QueryParseError::InvalidSyntax);
            };

            self.skip_whitespace();
            let direction = if self.match_keyword("DESC") {
                OrderDirection::Descending
            } else {
                // 默认升序
                self.match_keyword("ASC");
                OrderDirection::Ascending
            };

            Ok(Some(OrderByClause { field, direction }))
        } else {
            Ok(None)
        }
    }

    /// 解析LIMIT子句（可选）
    fn parse_limit_clause(&mut self) -> Result<Option<usize>, QueryParseError> {
        self.skip_whitespace();

        if self.match_keyword("LIMIT") {
            self.skip_whitespace();
            let limit = self.parse_number()? as usize;
            Ok(Some(limit))
        } else {
            Ok(None)
        }
    }

    /// 解析SAMPLE BY子句（可选）
    fn parse_sample_by_clause(&mut self) -> Result<Option<String>, QueryParseError> {
        self.skip_whitespace();

        if self.match_keyword("SAMPLE") {
            self.skip_whitespace();
            if !self.match_keyword("BY") {
                return Err(QueryParseError::InvalidSyntax);
            }
            self.skip_whitespace();
            // 解析时间间隔字符串，如"1h"、"5m"、"30s"
            // 时间间隔可以包含数字和字母，如"1h30m"
            let start = self.position;
            while let Some(c) = self.peek_char() {
                if c.is_ascii_alphanumeric() {
                    self.next_char();
                } else {
                    break;
                }
            }
            if self.position == start {
                return Err(QueryParseError::InvalidValue);
            }
            let interval = self.input[start..self.position].to_string();
            Ok(Some(interval))
        } else {
            Ok(None)
        }
    }

    /// 解析FILL子句（可选）
    fn parse_fill_clause(&mut self) -> Result<Option<FillClause>, QueryParseError> {
        self.skip_whitespace();

        if self.match_keyword("FILL") {
            self.skip_whitespace();
            // 解析填充策略
            if self.match_keyword("PREV") {
                Ok(Some(FillClause::Prev))
            } else if self.match_keyword("LINEAR") {
                Ok(Some(FillClause::Linear))
            } else if self.match_keyword("NEXT") {
                Ok(Some(FillClause::Next))
            } else {
                // 尝试解析固定值
                // 先检查是否是数字
                let saved_pos = self.position;
                let saved_col = self.column;
                match self.parse_number() {
                    Ok(num) => Ok(Some(FillClause::FixedValue(num as f64))),
                    Err(_) => {
                        // 恢复位置，返回错误
                        self.position = saved_pos;
                        self.column = saved_col;
                        Err(QueryParseError::InvalidValue)
                    }
                }
            }
        } else {
            Ok(None)
        }
    }

    /// 解析条件表达式
    fn parse_condition(&mut self) -> Result<Condition, QueryParseError> {
        // 解析WHERE条件
        self.parse_where_condition()
    }

    /// 解析WHERE条件，支持AND/OR/NOT组合
    fn parse_where_condition(&mut self) -> Result<Condition, QueryParseError> {
        // 解析NOT条件
        self.skip_whitespace();
        if self.match_keyword("NOT") {
            let inner_condition = self.parse_where_condition()?;
            return Ok(Condition::Not(Box::new(inner_condition)));
        }

        // 解析第一个条件
        let mut condition = self.parse_single_condition()?;

        // 处理AND/OR组合条件
        loop {
            self.skip_whitespace();

            if self.match_keyword("AND") {
                // 解析AND右侧的条件
                let right_condition = self.parse_where_condition()?;
                condition = Condition::And(Box::new(condition), Box::new(right_condition));
            } else if self.match_keyword("OR") {
                // 解析OR右侧的条件
                let right_condition = self.parse_where_condition()?;
                condition = Condition::Or(Box::new(condition), Box::new(right_condition));
            } else {
                // 没有更多的逻辑运算符，结束循环
                break;
            }
        }

        Ok(condition)
    }

    /// 解析单个条件（比较、BETWEEN、表达式、NOT、括号）
    fn parse_single_condition(&mut self) -> Result<Condition, QueryParseError> {
        self.skip_whitespace();

        // 检查是否是NOT条件
        if self.match_keyword("NOT") {
            let inner_condition = self.parse_single_condition()?;
            return Ok(Condition::Not(Box::new(inner_condition)));
        }

        // 检查是否是括号条件
        if self.match_char('(') {
            let inner_condition = self.parse_where_condition()?;
            self.skip_whitespace();
            self.expect_char(')')?;
            return Ok(inner_condition);
        }

        // 保存当前位置，用于回溯
        let saved_pos = self.position;
        let saved_col = self.column;

        // 尝试解析BETWEEN条件
        if let Ok(condition) = self.parse_between_condition() {
            return Ok(condition);
        }

        // 回溯，尝试解析比较条件
        self.position = saved_pos;
        self.column = saved_col;

        // 尝试解析比较条件
        let saved_pos_compare = self.position;
        let saved_col_compare = self.column;
        
        if let Ok(condition) = self.parse_comparison_condition() {
            return Ok(condition);
        }

        // 回溯，尝试解析表达式条件（如 "a" 或 "a + b"）
        self.position = saved_pos_compare;
        self.column = saved_col_compare;

        // 解析表达式
        let expr = self.parse_expression()?;
        
        // 对于表达式条件，我们将其视为与 TRUE 的比较
        // 例如 "a" 相当于 "a = TRUE"
        Ok(Condition::Comparison(ComparisonCondition {
            field: match expr {
                Expression::Field { name, alias: None } => name,
                _ => format!("{:?}", expr),
            },
            operator: ComparisonOperator::Equal,
            value: Value::Boolean(true),
        }))
    }

    /// 解析BETWEEN条件
    fn parse_between_condition(&mut self) -> Result<Condition, QueryParseError> {
        // 解析左侧表达式，支持向量表达式
        let left_expr = self.parse_vector_expression()?;

        self.skip_whitespace();

        // 检查是否是BETWEEN关键字
        if !self.match_keyword("BETWEEN") {
            return Err(QueryParseError::InvalidSyntax);
        }

        self.skip_whitespace();
        let min_value = self.parse_value()?;

        self.skip_whitespace();
        self.expect_keyword("AND")?;

        self.skip_whitespace();
        let max_value = self.parse_value()?;

        // 构建字段字符串，支持向量表达式
        let field = match left_expr {
            Expression::Field { name, alias: None } => name,
            Expression::BinaryOp {
                left,
                op,
                right,
                alias: None,
            } => {
                // 向量距离表达式，如 "vector <-> [5.0, 5.0]"
                let left_name = match *left {
                    Expression::Field { name, alias: None } => name,
                    _ => format!("{:?}", *left),
                };
                let right_str = match *right {
                    Expression::Constant { value, alias: None } => format!("{:?}", value),
                    _ => format!("{:?}", *right),
                };
                let op_str = match op {
                    BinaryOperator::Add => "+",
                    BinaryOperator::Subtract => "-",
                    BinaryOperator::Multiply => "*",
                    BinaryOperator::Equal => "=",
                    BinaryOperator::NotEqual => "!=",
                    BinaryOperator::GreaterThan => ">",
                    BinaryOperator::GreaterThanOrEqual => ">=",
                    BinaryOperator::LessThan => "<",
                    BinaryOperator::LessThanOrEqual => "<=",
                    BinaryOperator::VectorL2 => "<->",
                    BinaryOperator::VectorIP => "<#>",
                    BinaryOperator::VectorCosine => "<=>",
                    _ => "?",
                };
                format!("{} {} {}", left_name, op_str, right_str)
            }
            _ => format!("{:?}", left_expr),
        };

        Ok(Condition::Between(BetweenCondition {
            field,
            min_value,
            max_value,
        }))
    }

    /// 解析比较条件
    fn parse_comparison_condition(&mut self) -> Result<Condition, QueryParseError> {
        // 保存当前位置，用于回溯
        let _saved_pos = self.position;
        let _saved_col = self.column;

        // 解析左侧表达式，但不包含比较运算符
        let left_expr = self.parse_vector_expression()?;

        self.skip_whitespace();

        // 检查是否是LIKE操作符
        let operator = if self.match_keyword("LIKE") {
            ComparisonOperator::Like
        } else {
            // 解析普通比较运算符
            self.parse_comparison_operator()?
        };

        self.skip_whitespace();

        // 解析右侧值
        let right_value = self.parse_value()?;

        // 创建比较条件
        Ok(Condition::Comparison(ComparisonCondition {
            // 对于向量距离表达式，我们需要特殊处理
            field: match left_expr {
                Expression::Field { name, alias: None } => name,
                Expression::BinaryOp {
                    left,
                    op,
                    right,
                    alias: None,
                } => {
                    // 向量距离表达式，如 "vector <-> [5.0, 5.0]"
                    // 直接构建完整的向量距离表达式
                    let field_name = match *left {
                        Expression::Field { name, alias: None } => name,
                        _ => return Err(QueryParseError::InvalidSyntax),
                    };
                    
                    // 获取向量操作符的字符串表示
                    let op_str = match op {
                        BinaryOperator::VectorL2 => "<->",
                        BinaryOperator::VectorIP => "<#>",
                        BinaryOperator::VectorCosine => "<=>",
                        _ => return Err(QueryParseError::InvalidSyntax),
                    };
                    
                    // 从右侧表达式中提取实际向量值
                    let vector_str = match *right {
                        Expression::Constant { ref value, alias: None } => {
                            match value {
                                Value::String(ref vec_str) => vec_str.clone(),
                                Value::Json(ref json_str) => json_str.clone(),
                                _ => return Err(QueryParseError::InvalidSyntax),
                            }
                        },
                        _ => return Err(QueryParseError::InvalidSyntax),
                    };
                    
                    // 构建完整的向量距离表达式
                    format!("{field_name} {op_str} {vector_str}")
                }
                _ => format!("{:?}", left_expr),
            },
            operator,
            value: right_value,
        }))
    }

    /// 解析向量表达式，用于WHERE子句中的比较条件左侧
    fn parse_vector_expression(&mut self) -> Result<Expression, QueryParseError> {
        // 解析基本表达式
        let mut expr = self.parse_primary_expression()?;

        self.skip_whitespace();

        // 检查是否有向量操作符
        let saved_pos = self.position;
        let saved_col = self.column;

        // 只尝试解析一次向量操作符
        let op = match self.peek_char() {
            Some('<') => {
                self.next_char();
                match self.peek_char() {
                    Some('-') => {
                        self.next_char();
                        if self.peek_char() == Some('>') {
                            self.next_char();
                            Some(BinaryOperator::VectorL2) // <->
                        } else {
                            // 回退，不是向量操作符
                            self.position = saved_pos;
                            self.column = saved_col;
                            None
                        }
                    }
                    Some('#') => {
                        self.next_char();
                        if self.peek_char() == Some('>') {
                            self.next_char();
                            Some(BinaryOperator::VectorIP) // <#>
                        } else {
                            // 回退，不是向量操作符
                            self.position = saved_pos;
                            self.column = saved_col;
                            None
                        }
                    }
                    Some('=') => {
                        self.next_char();
                        if self.peek_char() == Some('>') {
                            self.next_char();
                            Some(BinaryOperator::VectorCosine) // <=>
                        } else {
                            // 回退，不是向量操作符
                            self.position = saved_pos;
                            self.column = saved_col;
                            None
                        }
                    }
                    _ => {
                        // 回退，不是向量操作符
                        self.position = saved_pos;
                        self.column = saved_col;
                        None
                    }
                }
            }
            _ => None,
        };

        // 如果是向量操作符，解析右侧操作数并构造新的表达式
        if let Some(op) = op {
            self.skip_whitespace();
            let right_expr = self.parse_primary_expression()?;

            // 更新表达式
            expr = Expression::BinaryOp {
                left: Box::new(expr),
                op,
                right: Box::new(right_expr),
                alias: None,
            };

            self.skip_whitespace();
        }

        Ok(expr)
    }

    /// 解析比较运算符（不包括向量操作符）
    fn parse_comparison_operator(&mut self) -> Result<ComparisonOperator, QueryParseError> {
        if self.match_str("=") {
            Ok(ComparisonOperator::Equal)
        } else if self.match_str("<>") || self.match_str("!=") {
            Ok(ComparisonOperator::NotEqual)
        } else if self.match_str(">=") {
            Ok(ComparisonOperator::GreaterThanOrEqual)
        } else if self.match_str(">") {
            Ok(ComparisonOperator::GreaterThan)
        } else if self.match_str("<=") {
            Ok(ComparisonOperator::LessThanOrEqual)
        } else if self.match_str("<") {
            Ok(ComparisonOperator::LessThan)
        } else {
            Err(QueryParseError::InvalidOperator)
        }
    }

    /// 解析比较运算符
    fn parse_operator(&mut self) -> Result<ComparisonOperator, QueryParseError> {
        if self.match_str("=") {
            Ok(ComparisonOperator::Equal)
        } else if self.match_str("<>") || self.match_str("!=") {
            Ok(ComparisonOperator::NotEqual)
        } else if self.match_str(">") {
            Ok(ComparisonOperator::GreaterThan)
        } else if self.match_str(">=") {
            Ok(ComparisonOperator::GreaterThanOrEqual)
        } else if self.match_str("<") {
            Ok(ComparisonOperator::LessThan)
        } else if self.match_str("<=") {
            Ok(ComparisonOperator::LessThanOrEqual)
        } else {
            Err(QueryParseError::InvalidOperator)
        }
    }

    /// 解析值
    fn parse_value(&mut self) -> Result<Value, QueryParseError> {
        // 保存当前位置，用于回溯
        let _saved_pos = self.position;

        if self.peek_char() == Some('"') || self.peek_char() == Some('\'') {
            // 字符串值
            let quote_char = self.next_char().ok_or(RemDbError::InvalidSqlQuery)?;
            let mut string_value = String::new();

            while let Some(c) = self.next_char() {
                if c == quote_char {
                    break;
                }
                string_value.push(c);
            }
            
            // 先检查是否是带类型提示的JSON字符串
            if string_value.starts_with("__JSON__:") {
                let json_str = string_value.trim_start_matches("__JSON__:");
                #[cfg(feature = "log")]
                debug!("parse_value: Parsed as JSON with prefix");
                Ok(Value::Json(json_str.to_string()))
            } else {
                // 检查是否是带引号的JSON字符串，去除引号后检查
                let unquoted = string_value.trim_start_matches('"').trim_end_matches('"').trim_start_matches('\'').trim_end_matches('\'');
                if unquoted.starts_with('{') || unquoted.starts_with('[') {
                    // 去除引号后是JSON格式
                    #[cfg(feature = "log")]
                    debug!("parse_value: Parsed as JSON (unquoted starts with {{ or [)");
                    Ok(Value::Json(unquoted.to_string()))
                } else {
                    // 尝试将字符串解析为时间值
                    if let Ok(timestamp) = parse_time_string(&string_value) {
                        #[cfg(feature = "log")]
                        debug!("parse_value: Parsed as timestamp");
                        Ok(Value::Integer(timestamp))
                    } else {
                        // 不是JSON格式，作为普通字符串处理
                        #[cfg(feature = "log")]
                        debug!("parse_value: Parsed as String");
                        Ok(Value::String(string_value))
                    }
                }
            }
        } else if self.match_keyword("NULL") {
            Ok(Value::Null)
        } else if self.match_keyword("TRUE") {
            Ok(Value::Boolean(true))
        } else if self.match_keyword("FALSE") {
            Ok(Value::Boolean(false))
        } else if self.match_keyword("NOW") {
            // 处理NOW()函数
            self.skip_whitespace();
            self.expect_char('(')?;
            self.skip_whitespace();
            self.expect_char(')')?;
            // 这里返回0作为占位符，实际执行时会替换为当前时间
            Ok(Value::Integer(0))
        } else if self.match_keyword("CURRENT_TIMESTAMP") {
            // 处理CURRENT_TIMESTAMP()函数
            self.skip_whitespace();
            if self.peek_char() == Some('(') {
                self.next_char();
                self.skip_whitespace();
                self.expect_char(')')?;
            }
            // 这里返回0作为占位符，实际执行时会替换为当前时间
            Ok(Value::Integer(0))
        } else if self.match_keyword("LOCALTIMESTAMP") {
            // 处理LOCALTIMESTAMP()函数
            self.skip_whitespace();
            if self.peek_char() == Some('(') {
                self.next_char();
                self.skip_whitespace();
                self.expect_char(')')?;
            }
            // 这里返回0作为占位符，实际执行时会替换为当前时间
            Ok(Value::Integer(0))
        } else if self.match_keyword("TIMEZONE") {
            // 处理TIMEZONE()函数 - 直接跳过内部内容，避免递归
            self.skip_whitespace();
            self.expect_char('(')?;
            self.skip_whitespace();
            // 直接跳过参数，不递归调用parse_value
            while self.peek_char() != Some(')') {
                self.next_char();
            }
            self.next_char();
            // 这里返回0作为占位符，实际执行时会处理
            Ok(Value::Integer(0))
        } else if self.match_keyword("TO_CHAR") {
            // 处理TO_CHAR()函数 - 直接跳过内部内容，避免递归
            self.skip_whitespace();
            self.expect_char('(')?;
            self.skip_whitespace();
            // 直接跳过参数，不递归调用parse_value
            while self.peek_char() != Some(')') {
                self.next_char();
            }
            self.next_char();
            // 这里返回0作为占位符，实际执行时会处理
            Ok(Value::Integer(0))
        } else if self.match_keyword("TO_ISO8601") {
            // 处理TO_ISO8601()函数 - 直接跳过内部内容，避免递归
            self.skip_whitespace();
            self.expect_char('(')?;
            self.skip_whitespace();
            // 直接跳过参数，不递归调用parse_value
            while self.peek_char() != Some(')') {
                self.next_char();
            }
            self.next_char();
            // 这里返回0作为占位符，实际执行时会处理
            Ok(Value::Integer(0))
        } else if self.match_keyword("TO_EPOCH") {
            // 处理TO_EPOCH()函数 - 直接跳过内部内容，避免递归
            self.skip_whitespace();
            self.expect_char('(')?;
            self.skip_whitespace();
            // 直接跳过参数，不递归调用parse_value
            while self.peek_char() != Some(')') {
                self.next_char();
            }
            self.next_char();
            // 这里返回0作为占位符，实际执行时会处理
            Ok(Value::Integer(0))
        } else if self.peek_char() == Some('[') {
            // JSON数组或向量字面量
            let start_pos = self.position;

            // 跳过左括号
            self.next_char();

            // 查找匹配的右括号
            let mut bracket_count = 1;
            let mut end_pos = start_pos + 1;
            let mut in_string = false;
            let mut quote_char = '"';

            while bracket_count > 0 {
                if self.is_eof() {
                    return Err(QueryParseError::InvalidSyntax);
                }

                let c = self.next_char().ok_or(RemDbError::InvalidSqlQuery)?;
                end_pos += 1;

                // 处理字符串中的括号
                if c == '"' || c == '\'' {
                    if !in_string {
                        in_string = true;
                        quote_char = c;
                    } else if c == quote_char {
                        in_string = false;
                    }
                }

                if !in_string {
                    if c == '[' {
                        bracket_count += 1;
                    } else if c == ']' {
                        bracket_count -= 1;
                    }
                }
            }

            // 提取完整的数组字符串
            let json_str = self.input[start_pos..end_pos].to_string();

            // 返回JSON值（JSON数组作为JSON处理）
            Ok(Value::Json(json_str))
        } else if self.peek_char() == Some('{') {
            // JSON对象
            let start_pos = self.position;

            // 跳过左大括号
            self.next_char();

            // 查找匹配的右大括号
            let mut brace_count = 1;
            let mut end_pos = start_pos + 1;
            let mut in_string = false;
            let mut quote_char = '"';

            while brace_count > 0 {
                if self.is_eof() {
                    return Err(QueryParseError::InvalidSyntax);
                }

                let c = self.next_char().ok_or(RemDbError::InvalidSqlQuery)?;
                end_pos += 1;

                // 处理字符串中的大括号
                if c == '"' || c == '\'' {
                    if !in_string {
                        in_string = true;
                        quote_char = c;
                    } else if c == quote_char {
                        in_string = false;
                    }
                }

                if !in_string {
                    if c == '{' {
                        brace_count += 1;
                    } else if c == '}' {
                        brace_count -= 1;
                    }
                }
            }

            // 提取完整的JSON对象字符串
            let json_str = self.input[start_pos..end_pos].to_string();

            // 返回JSON值
            Ok(Value::Json(json_str))
        } else if self
            .peek_char()
            .is_some_and(|c| c.is_ascii_digit() || c == '-')
        {
            // 数字值
            let number_str = self.parse_number_str()?;
            if number_str.contains('.') {
                // 浮点数
                let float_value = number_str
                    .parse::<f64>()
                    .map_err(|_| QueryParseError::InvalidValue)?;
                Ok(Value::Float(float_value))
            } else {
                // 整数
                let int_value = number_str
                    .parse::<i64>()
                    .map_err(|_| QueryParseError::InvalidValue)?;
                Ok(Value::Integer(int_value))
            }
        } else if self.peek_char().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') {
            // 标识符（字段名、表名等）
            let identifier = self.parse_identifier()?;
            Ok(Value::Identifier(identifier))
        } else {
            // 简化处理：只处理基本类型的值，不处理函数调用
            // 避免与其他函数形成循环调用，导致无限递归
            return Err(QueryParseError::InvalidValue);
        }
    }

    /// 解析标识符（表名、字段名，支持带点号的字段名如t.id）
    fn parse_identifier(&mut self) -> Result<String, QueryParseError> {
        let start = self.position;

        // 标识符必须以字母或下划线开头
        let c = self.peek_char().ok_or(QueryParseError::InvalidSyntax)?;
        if !c.is_ascii_alphabetic() && c != '_' {
            return Err(QueryParseError::InvalidSyntax);
        }

        self.next_char();

        // 后续字符可以是字母、数字、下划线、点号
        while let Some(c) = self.peek_char() {
            if c.is_ascii_alphanumeric() || c == '_' || c == '.' {
                self.next_char();
            } else {
                break;
            }
        }

        Ok(self.input[start..self.position].to_string())
    }

    /// 解析数字字符串
    fn parse_number_str(&mut self) -> Result<String, QueryParseError> {
        let start = self.position;

        // 允许以负号开头
        if self.match_char('-') {
            // 负号已被消耗
        }

        // 必须有数字
        if !self.peek_char().is_some_and(|c| c.is_ascii_digit()) {
            return Err(QueryParseError::InvalidValue);
        }

        // 解析数字部分
        while let Some(c) = self.peek_char() {
            if c.is_ascii_digit() || c == '.' {
                self.next_char();
            } else {
                break;
            }
        }

        Ok(self.input[start..self.position].to_string())
    }

    /// 解析数字值
    fn parse_number(&mut self) -> Result<usize, QueryParseError> {
        let number_str = self.parse_number_str()?;
        number_str
            .parse::<usize>()
            .map_err(|_| QueryParseError::InvalidValue)
    }

    /// 匹配关键字
    fn match_keyword(&mut self, keyword: &str) -> bool {
        // 调试：打印匹配尝试
        let start = self.position;
        let keyword_bytes = keyword.as_bytes();
        let end = start + keyword_bytes.len();

        if end <= self.input.as_bytes().len() {
            let actual_bytes = &self.input.as_bytes()[start..end];
            let expected_bytes = keyword_bytes;

            // 比较字节序列（忽略大小写）
            if actual_bytes.eq_ignore_ascii_case(expected_bytes) {
                // 检查是否是完整的关键字（后面跟着非字母数字字符）
                let next_char = self.input.as_bytes().get(end).map(|&b| b as char);
                if next_char.is_none() || !next_char.unwrap_or(' ').is_ascii_alphanumeric() {
                    self.position = end;
                    self.column += keyword.len();
                    return true;
                }
            }
        }

        false
    }

    /// 匹配字符串
    fn match_str(&mut self, s: &str) -> bool {
        let start = self.position;
        let s_bytes = s.as_bytes();
        let end = start + s_bytes.len();

        if end <= self.input.as_bytes().len() {
            if &self.input.as_bytes()[start..end] == s_bytes {
                self.position = end;
                self.column += s.len();
                return true;
            }
        }

        false
    }

    /// 匹配字符
    fn match_char(&mut self, c: char) -> bool {
        if self.peek_char() == Some(c) {
            self.next_char();
            true
        } else {
            false
        }
    }

    /// 期望关键字
    fn expect_keyword(&mut self, keyword: &str) -> Result<(), QueryParseError> {
        if self.match_keyword(keyword) {
            Ok(())
        } else {
            Err(QueryParseError::InvalidSyntax)
        }
    }

    /// 查看当前字符
    fn peek_char(&self) -> Option<char> {
        if self.position < self.input.as_bytes().len() {
            Some(self.input.as_bytes()[self.position] as char)
        } else {
            None
        }
    }

    /// 获取下一个字符
    fn next_char(&mut self) -> Option<char> {
        if let Some(c) = self.peek_char() {
            self.position += 1;
            if c == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
            Some(c)
        } else {
            None
        }
    }

    /// 跳过空白字符
    fn skip_whitespace(&mut self) {
        // 添加安全检查，防止无限循环
        let max_skips = self.input.len();
        let mut skips = 0;

        while let Some(c) = self.peek_char() {
            if skips > max_skips {
                break; // 防止无限循环
            }

            if c.is_whitespace() {
                self.next_char();
                skips += 1;
            } else {
                break;
            }
        }
    }

    /// 是否已到达输入末尾
    fn is_eof(&self) -> bool {
        self.position >= self.input.len()
    }

    /// 解析数据类型，支持复杂类型如 VARCHAR(255), INT UNSIGNED
    fn parse_data_type(&mut self) -> Result<String, QueryParseError> {
        // 解析基本数据类型
        let base_type = self.parse_identifier()?;

        // 保存完整类型，包括参数和修饰符
        let mut result = base_type.clone();

        // 检查是否有参数，如 VARCHAR(255), VECTOR(768) 或 TIMESTAMP(6)
        self.skip_whitespace();
        if self.match_char('(') {
            // 包含参数
            result.push('(');
            let mut depth = 1;
            while depth > 0 {
                let c = self.next_char().ok_or(QueryParseError::InvalidSyntax)?;
                result.push(c);
                if c == '(' {
                    depth += 1;
                } else if c == ')' {
                    depth -= 1;
                }
            }
        }

        // 检查是否有修饰符，如 UNSIGNED, WITH TIME ZONE 或 WITH DISTANCE=L2
        self.skip_whitespace();
        
        // 只尝试解析修饰符一次，避免无限循环
        if self.peek_char().is_some() {
            // 检查是否是约束关键字（这些应该在数据类型之后单独解析）
            let next_token = self.peek_identifier();
            if let Some(token) = next_token {
                let token_upper = token.to_uppercase();
                // 如果遇到约束关键字，停止解析数据类型
                if [
                    "PRIMARY",
                    "NOT",
                    "UNIQUE",
                    "AUTOINCREMENT",
                    "AUTO_INCREMENT",
                    "DEFAULT",
                ]
                .contains(&token_upper.as_str())
                {
                    return Ok(result);
                }
            }

            // 检查是否是修饰符（字母或下划线开头）
            let c = self.peek_char().ok_or(RemDbError::InvalidSqlQuery)?;
            if c.is_ascii_alphabetic() || c == '_' {
                // 解析修饰符
                let modifier = self.parse_identifier()?;
                result.push(' ');
                result.push_str(&modifier);

                // 检查是否是 WITH 修饰符，如 WITH TIME ZONE 或 WITH DISTANCE=L2
                if modifier.eq_ignore_ascii_case("WITH") {
                    self.skip_whitespace();
                    
                    if self.peek_char().is_some() {
                        // 检查是否是约束关键字
                        let next_token = self.peek_identifier();
                        if let Some(token) = next_token {
                            let token_upper = token.to_uppercase();
                            if !["PRIMARY",
                                "NOT",
                                "UNIQUE",
                                "AUTOINCREMENT",
                                "AUTO_INCREMENT",
                                "DEFAULT",
                            ]
                            .contains(&token_upper.as_str()) {
                                // 检查下一个字符是否是标识符的开始（字母或下划线）
                                let next_char = self.peek_char().ok_or(RemDbError::InvalidSqlQuery)?;
                                if next_char.is_ascii_alphabetic() || next_char == '_' {
                                    // 解析 WITH 后的修饰符
                                    let with_modifier = self.parse_identifier()?;
                                    result.push(' ');
                                    result.push_str(&with_modifier);

                                    // 检查是否有等号，如 DISTANCE=L2
                                    self.skip_whitespace();
                                    if self.match_char('=') {
                                        result.push('=');

                                        // 解析等号后的数值或标识符
                                        let value = self.parse_identifier()?;
                                        result.push_str(&value);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(result)
    }

    /// 预看下一个标识符
    fn peek_identifier(&self) -> Option<String> {
        let start = self.position;
        let mut pos = start;

        // 检查第一个字符是否是字母或下划线
        if let Some(c) = self.input.chars().nth(pos) {
            if !c.is_ascii_alphabetic() && c != '_' {
                return None;
            }
            pos += 1;

            // 继续读取直到遇到非字母数字或下划线
            while let Some(c) = self.input.chars().nth(pos) {
                if c.is_ascii_alphanumeric() || c == '_' {
                    pos += 1;
                } else {
                    break;
                }
            }

            // 返回预看的标识符
            Some(self.input[start..pos].to_string())
        } else {
            None
        }
    }

    /// 期望匹配指定字符
    fn expect_char(&mut self, c: char) -> Result<(), QueryParseError> {
        if self.match_char(c) {
            Ok(())
        } else {
            Err(QueryParseError::InvalidSyntax)
        }
    }

    /// 解析CREATE ROLE查询
    fn parse_create_role_query(&mut self) -> Result<SqlQuery, QueryParseError> {
        // 解析角色名称
        self.skip_whitespace();
        let role_name = self.parse_identifier()?;

        Ok(SqlQuery {
            query_type: QueryType::CreateRole,
            table_name: role_name,
            table_alias: None,
            joins: Vec::new(),
            columns: Vec::new(),
            select_all: false,
            distinct: false,
            where_clause: None,
            having_clause: None,
            group_by: None,
            order_by: None,
            limit: None,
            sample_by: None,
            fill_clause: None,
            window_functions: Vec::new(),
            insert_columns: Vec::new(),
            values: Vec::new(),
            table_def: Vec::new(),
            primary_key: None,
            index_column: None,
            index_type: None,
            index_params: HashMap::new(),
            index_online: true,
            update_pairs: Vec::new(),
            ignore_duplicates: false,
            if_not_exists: false,
            model_path: String::new(),
            model_inputs: Vec::new(),
            model_output: (String::new(), String::new()),
            table_config: HashMap::new(),
        })
    }

    fn parse_create_user_query(&mut self) -> Result<SqlQuery, QueryParseError> {
        // 解析用户名称
        self.skip_whitespace();
        let user_name = self.parse_identifier()?;

        Ok(SqlQuery {
            query_type: QueryType::CreateUser,
            table_name: user_name,
            table_alias: None,
            joins: Vec::new(),
            columns: Vec::new(),
            select_all: false,
            distinct: false,
            where_clause: None,
            having_clause: None,
            group_by: None,
            order_by: None,
            limit: None,
            sample_by: None,
            fill_clause: None,
            window_functions: Vec::new(),
            insert_columns: Vec::new(),
            values: Vec::new(),
            table_def: Vec::new(),
            primary_key: None,
            index_column: None,
            index_type: None,
            index_params: HashMap::new(),
            index_online: true,
            update_pairs: Vec::new(),
            ignore_duplicates: false,
            if_not_exists: false,
            model_path: String::new(),
            model_inputs: Vec::new(),
            model_output: (String::new(), String::new()),
            table_config: HashMap::new(),
        })
    }

    /// 解析GRANT PERMISSION查询
    fn parse_grant_permission_query(&mut self) -> Result<SqlQuery, QueryParseError> {
        // 解析权限列表
        let mut permissions = Vec::new();
        loop {
            self.skip_whitespace();
            let permission = self.parse_identifier()?;
            permissions.push(permission);
            self.skip_whitespace();
            if self.match_char(',') {
                continue;
            } else {
                break;
            }
        }

        // 解析ON关键字
        self.skip_whitespace();
        self.expect_keyword("ON")?;

        // 解析表名
        self.skip_whitespace();
        let table_name = self.parse_identifier()?;

        // 解析TO关键字
        self.skip_whitespace();
        self.expect_keyword("TO")?;

        // 解析角色名
        self.skip_whitespace();
        let role_name = self.parse_identifier()?;

        Ok(SqlQuery {
            query_type: QueryType::GrantPermission,
            table_name,
            table_alias: Some(role_name),
            joins: Vec::new(),
            columns: permissions.into_iter().map(|p| Expression::Field { name: p, alias: None }).collect(),
            select_all: false,
            distinct: false,
            where_clause: None,
            having_clause: None,
            group_by: None,
            order_by: None,
            limit: None,
            sample_by: None,
            fill_clause: None,
            window_functions: Vec::new(),
            insert_columns: Vec::new(),
            values: Vec::new(),
            table_def: Vec::new(),
            primary_key: None,
            index_column: None,
            index_type: None,
            index_params: HashMap::new(),
            index_online: true,
            update_pairs: Vec::new(),
            ignore_duplicates: false,
            if_not_exists: false,
            model_path: String::new(),
            model_inputs: Vec::new(),
            model_output: (String::new(), String::new()),
            table_config: HashMap::new(),
        })
    }

    /// 解析GRANT ROLE查询
    fn parse_grant_role_query(&mut self) -> Result<SqlQuery, QueryParseError> {
        // 解析角色名
        self.skip_whitespace();
        let role_name = self.parse_identifier()?;

        // 解析TO关键字
        self.skip_whitespace();
        self.expect_keyword("TO")?;

        // 解析用户名
        self.skip_whitespace();
        let user_name = self.parse_identifier()?;

        Ok(SqlQuery {
            query_type: QueryType::GrantRole,
            table_name: role_name,
            table_alias: Some(user_name),
            joins: Vec::new(),
            columns: Vec::new(),
            select_all: false,
            distinct: false,
            where_clause: None,
            having_clause: None,
            group_by: None,
            order_by: None,
            limit: None,
            sample_by: None,
            fill_clause: None,
            window_functions: Vec::new(),
            insert_columns: Vec::new(),
            values: Vec::new(),
            table_def: Vec::new(),
            primary_key: None,
            index_column: None,
            index_type: None,
            index_params: HashMap::new(),
            index_online: true,
            update_pairs: Vec::new(),
            ignore_duplicates: false,
            if_not_exists: false,
            model_path: String::new(),
            model_inputs: Vec::new(),
            model_output: (String::new(), String::new()),
            table_config: HashMap::new(),
        })
    }

    /// 解析REVOKE PERMISSION查询
    fn parse_revoke_permission_query(&mut self) -> Result<SqlQuery, QueryParseError> {
        // 解析权限列表
        let mut permissions = Vec::new();
        loop {
            self.skip_whitespace();
            let permission = self.parse_identifier()?;
            permissions.push(permission);
            self.skip_whitespace();
            if self.match_char(',') {
                continue;
            } else {
                break;
            }
        }

        // 解析ON关键字
        self.skip_whitespace();
        self.expect_keyword("ON")?;

        // 解析表名
        self.skip_whitespace();
        let table_name = self.parse_identifier()?;

        // 解析FROM关键字
        self.skip_whitespace();
        self.expect_keyword("FROM")?;

        // 解析角色名
        self.skip_whitespace();
        let role_name = self.parse_identifier()?;

        Ok(SqlQuery {
            query_type: QueryType::RevokePermission,
            table_name,
            table_alias: Some(role_name),
            joins: Vec::new(),
            columns: permissions.into_iter().map(|p| Expression::Field { name: p, alias: None }).collect(),
            select_all: false,
            distinct: false,
            where_clause: None,
            having_clause: None,
            group_by: None,
            order_by: None,
            limit: None,
            sample_by: None,
            fill_clause: None,
            window_functions: Vec::new(),
            insert_columns: Vec::new(),
            values: Vec::new(),
            table_def: Vec::new(),
            primary_key: None,
            index_column: None,
            index_type: None,
            index_params: HashMap::new(),
            index_online: true,
            update_pairs: Vec::new(),
            ignore_duplicates: false,
            if_not_exists: false,
            model_path: String::new(),
            model_inputs: Vec::new(),
            model_output: (String::new(), String::new()),
            table_config: HashMap::new(),
        })
    }

    /// 解析REVOKE ROLE查询
    fn parse_revoke_role_query(&mut self) -> Result<SqlQuery, QueryParseError> {
        // 解析角色名
        self.skip_whitespace();
        let role_name = self.parse_identifier()?;

        // 解析FROM关键字
        self.skip_whitespace();
        self.expect_keyword("FROM")?;

        // 解析用户名
        self.skip_whitespace();
        let user_name = self.parse_identifier()?;

        Ok(SqlQuery {
            query_type: QueryType::RevokeRole,
            table_name: role_name,
            table_alias: Some(user_name),
            joins: Vec::new(),
            columns: Vec::new(),
            select_all: false,
            distinct: false,
            where_clause: None,
            having_clause: None,
            group_by: None,
            order_by: None,
            limit: None,
            sample_by: None,
            fill_clause: None,
            window_functions: Vec::new(),
            insert_columns: Vec::new(),
            values: Vec::new(),
            table_def: Vec::new(),
            primary_key: None,
            index_column: None,
            index_type: None,
            index_params: HashMap::new(),
            index_online: true,
            update_pairs: Vec::new(),
            ignore_duplicates: false,
            if_not_exists: false,
            model_path: String::new(),
            model_inputs: Vec::new(),
            model_output: (String::new(), String::new()),
            table_config: HashMap::new(),
        })
    }

    /// 解析DROP ROLE查询
    fn parse_drop_role_query(&mut self) -> Result<SqlQuery, QueryParseError> {
        // 解析角色名
        self.skip_whitespace();
        let role_name = self.parse_identifier()?;

        Ok(SqlQuery {
            query_type: QueryType::DropRole,
            table_name: role_name,
            table_alias: None,
            joins: Vec::new(),
            columns: Vec::new(),
            select_all: false,
            distinct: false,
            where_clause: None,
            having_clause: None,
            group_by: None,
            order_by: None,
            limit: None,
            sample_by: None,
            fill_clause: None,
            window_functions: Vec::new(),
            insert_columns: Vec::new(),
            values: Vec::new(),
            table_def: Vec::new(),
            primary_key: None,
            index_column: None,
            index_type: None,
            index_params: HashMap::new(),
            index_online: true,
            update_pairs: Vec::new(),
            ignore_duplicates: false,
            if_not_exists: false,
            model_path: String::new(),
            model_inputs: Vec::new(),
            model_output: (String::new(), String::new()),
            table_config: HashMap::new(),
        })
    }

    /// 解析DROP USER查询
    fn parse_drop_user_query(&mut self) -> Result<SqlQuery, QueryParseError> {
        // 解析用户名
        self.skip_whitespace();
        let user_name = self.parse_identifier()?;

        Ok(SqlQuery {
            query_type: QueryType::DropUser,
            table_name: user_name,
            table_alias: None,
            joins: Vec::new(),
            columns: Vec::new(),
            select_all: false,
            distinct: false,
            where_clause: None,
            having_clause: None,
            group_by: None,
            order_by: None,
            limit: None,
            sample_by: None,
            fill_clause: None,
            window_functions: Vec::new(),
            insert_columns: Vec::new(),
            values: Vec::new(),
            table_def: Vec::new(),
            primary_key: None,
            index_column: None,
            index_type: None,
            index_params: HashMap::new(),
            index_online: true,
            update_pairs: Vec::new(),
            ignore_duplicates: false,
            if_not_exists: false,
            model_path: String::new(),
            model_inputs: Vec::new(),
            model_output: (String::new(), String::new()),
            table_config: HashMap::new(),
        })
    }
}

/// 解析SQL查询字符串
pub fn parse_sql_query(sql: &str) -> Result<SqlQuery, QueryParseError> {
    let mut parser = SqlParser::new(sql.to_string());
    parser.parse()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_create_table_composite_primary_key() {
        let sql = "CREATE TABLE IF NOT EXISTS test_composite_pk (id1 INTEGER, id2 INTEGER, name TEXT, PRIMARY KEY (id1, id2))";
        let result = parse_sql_query(sql);
        assert!(result.is_ok());
        let query = result.unwrap();
        match query.query_type {
            QueryType::CreateTable => {
                // 检查主键是否正确解析
                assert!(query.primary_key.is_some());
                let pk = query.primary_key.unwrap();
                assert_eq!(pk, vec!["id1", "id2"]);
            }
            _ => panic!("Expected CreateTable query"),
        }
    }
}
