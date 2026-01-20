//! SQL查询解析器
//! 
//! 该模块负责将SQL查询字符串解析为结构化的查询对象。

use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;
use alloc::boxed::Box;

/// 解析时间字符串为微秒时间戳
/// 支持的格式：
/// - '2024-01-15 10:30:45'
/// - '2024-01-15T10:30:45.123Z'
/// - '2024-01-15 10:30:45.123+08'
/// - 1673778645123456 (微秒时间戳)
fn parse_time_string(time_str: &str) -> Result<i64, ()> {
    // 简单的实现，实际应该支持更多格式
    // 这里只做一个示例，解析ISO 8601格式
    // 检查是否是实际的时间格式，而不是格式字符串
    // 格式字符串通常包含Y, M, D, H, I, S等格式说明符
    if time_str.contains(|c| c == 'Y' || c == 'M' || c == 'D' || c == 'H' || c == 'I' || c == 'S') {
        // 这是一个格式字符串，不是时间值
        Err(())
    } else if (time_str.contains('T') || time_str.contains(' ')) && 
              time_str.chars().any(|c| c.is_digit(10)) {
        // 尝试解析为ISO 8601格式，必须包含数字
        // 这里使用简化的实现，实际应该使用更完整的解析
        Ok(0) // 占位符，实际实现需要完整的时间解析
    } else {
        // 尝试解析为数字时间戳
        time_str.parse::<i64>().map_err(|_| ())
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
    /// 分组条件
    pub group_by: Option<GroupByClause>,
    /// 排序条件
    pub order_by: Option<OrderByClause>,
    /// 结果限制
    pub limit: Option<usize>,
    /// 要插入的字段列表
    pub insert_columns: Vec<String>,
    /// 要插入的值列表
    pub values: Vec<Vec<Value>>,
    /// 表字段定义（用于CREATE TABLE）：(字段名, 类型, 主键, 非空, 唯一, 自增, 默认值)
    pub table_def: Vec<(String, String, bool, bool, bool, bool, Option<Value>)>,
    /// 主键字段名（用于CREATE TABLE）
    pub primary_key: Option<String>,
    /// 索引字段名（用于CREATE INDEX）
    pub index_column: Option<String>,
    /// 索引类型（用于CREATE INDEX）
    pub index_type: Option<String>,
    /// 更新的字段值对（用于UPDATE）：(字段名, 新值表达式)
    pub update_pairs: Vec<(String, Expression)>,
    /// 是否忽略重复键
    pub ignore_duplicates: bool,
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
            QueryType::CreateTimeSeriesTable => self.parse_create_table_query(),
            QueryType::CreateIndex => self.parse_create_index_query(),
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
            insert_columns: Vec::new(),
            values: Vec::new(),
            table_def: Vec::new(),
            primary_key: None,
            index_column: None,
            index_type: None,
            update_pairs,
            ignore_duplicates: false,
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
            insert_columns: Vec::new(),
            values: Vec::new(),
            table_def: Vec::new(),
            primary_key: None,
            index_column: None,
            index_type: None,
            update_pairs: Vec::new(),
            ignore_duplicates: false,
        })
    }
    
    /// 解析INSERT查询
    fn parse_insert_query(&mut self) -> Result<SqlQuery, QueryParseError> {
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
            group_by: None,
            order_by: None,
            limit: None,
            insert_columns,
            values,
            table_def: Vec::new(),
            primary_key: None,
            index_column: None,
            index_type: None,
            update_pairs: Vec::new(),
            ignore_duplicates,
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
                let value = self.parse_value()?;
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
            group_by: None,
            order_by: None,
            limit: None,
            insert_columns: Vec::new(),
            values: Vec::new(),
            table_def: Vec::new(),
            primary_key: None,
            index_column: None,
            index_type: None,
            update_pairs: Vec::new(),
            ignore_duplicates: false,
        })
    }
    
    /// 解析CREATE TABLE查询
    fn parse_create_table_query(&mut self) -> Result<SqlQuery, QueryParseError> {
        // 解析表名
        self.skip_whitespace();
        let table_name = self.parse_identifier()?;
        
        // 解析左括号
        self.skip_whitespace();
        self.expect_char('(')?;
        
        // 解析字段定义
        let mut table_def = Vec::new();
        let mut primary_key = None;
        
        loop {
            self.skip_whitespace();
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
                    primary_key = Some(field_name.clone());
                } else if self.match_keyword("NOT") {
                    self.skip_whitespace();
                    self.expect_keyword("NULL")?;
                    is_not_null = true;
                } else if self.match_keyword("UNIQUE") {
                    is_unique = true;
                } else if self.match_keyword("AUTOINCREMENT") || self.match_keyword("AUTO_INCREMENT") {
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
            
            table_def.push((field_name, data_type, is_primary_key, is_not_null, is_unique, is_auto_increment, default_value));
            
            self.skip_whitespace();
            if self.match_char(')') {
                break;
            }
            
            if !self.match_char(',') {
                return Err(QueryParseError::InvalidSyntax);
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
            group_by: None,
            order_by: None,
            limit: None,
            insert_columns: Vec::new(),
            values: Vec::new(),
            table_def,
            primary_key,
            index_column: None,
            index_type: None,
            update_pairs: Vec::new(),
            ignore_duplicates: false,
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
        
        // 解析索引字段
        self.skip_whitespace();
        let index_column = self.parse_identifier()?;
        
        // 解析右括号
        self.skip_whitespace();
        self.expect_char(')')?;
        
        // 解析索引类型（可选）
        let mut index_type = None;
        self.skip_whitespace();
        if self.match_keyword("USING") {
            self.skip_whitespace();
            index_type = Some(self.parse_identifier()?.to_uppercase());
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
            group_by: None,
            order_by: None,
            limit: None,
            insert_columns: Vec::new(),
            values: Vec::new(),
            table_def: Vec::new(),
            primary_key: None,
            index_column: Some(index_column),
            index_type,
            update_pairs: Vec::new(),
            ignore_duplicates: false,
        })
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
            if self.match_keyword("TIMESERIES") {
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
            } else {
                Ok(QueryType::Other)
            }
        } else {
            Ok(QueryType::Other)
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
        
        // 解析ORDER BY子句（可选）
        let order_by = self.parse_order_by_clause()?;
        
        // 解析LIMIT子句（可选）
        let limit = self.parse_limit_clause()?;
        
        Ok(SqlQuery {
            query_type: QueryType::Select,
            table_name,
            table_alias,
            joins,
            columns,
            select_all,
            distinct,
            where_clause,
            group_by,
            order_by,
            limit,
            insert_columns: Vec::new(),
            values: Vec::new(),
            table_def: Vec::new(),
            primary_key: None,
            index_column: None,
            index_type: None,
            update_pairs: Vec::new(),
            ignore_duplicates: false,
        })
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
                Some('<') => {
                    self.next_char();
                    match self.peek_char() {
                        Some('-') => {
                            self.next_char();
                            BinaryOperator::VectorL2 // <->
                        }
                        Some('#') => {
                            self.next_char();
                            BinaryOperator::VectorIP // <#>
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
                        _ => BinaryOperator::LessThan
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
        
        self.skip_whitespace();
        let alias = self.parse_alias()?;
        
        // 如果有别名，需要更新表达式的别名
        match left_expr {
            Expression::Field { alias: mut expr_alias, name, .. } => {
                Ok(Expression::Field {
                    name,
                    alias: alias.or(expr_alias),
                })
            },
            Expression::FunctionCall { alias: mut expr_alias, name, args, .. } => {
                Ok(Expression::FunctionCall {
                    name,
                    args,
                    alias: alias.or(expr_alias),
                })
            },
            Expression::Constant { alias: mut expr_alias, value, .. } => {
                Ok(Expression::Constant {
                    value,
                    alias: alias.or(expr_alias),
                })
            },
            Expression::BinaryOp { left, op, right, .. } => {
                Ok(Expression::BinaryOp {
                    left,
                    op,
                    right,
                    alias,
                })
            },
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
        
        // 尝试解析标识符
        if let Ok(identifier) = self.parse_identifier() {
            // 检查下一个字符是否是左括号
            self.skip_whitespace();
            if self.peek_char() == Some('(') {
                // 回退到标识符开始位置
                self.position = saved_pos;
                self.column = saved_col;
                
                // 解析函数调用
                return self.parse_function_call();
            } else if identifier.eq_ignore_ascii_case("INTERVAL") {
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
            } else if identifier.eq_ignore_ascii_case("TRUE") {
                // 布尔常量TRUE
                return Ok(Expression::Constant {
                    value: Value::Boolean(true),
                    alias: None,
                });
            } else if identifier.eq_ignore_ascii_case("FALSE") {
                // 布尔常量FALSE
                return Ok(Expression::Constant {
                    value: Value::Boolean(false),
                    alias: None,
                });
            } else {
                // 不是函数调用，返回字段表达式
                return Ok(Expression::Field {
                    name: identifier,
                    alias: None,
                });
            }
        }
        
        // 回溯，尝试解析常量值
        self.position = saved_pos;
        self.column = saved_col;
        
        // 尝试解析常量值
        let value = self.parse_value()?;
        return Ok(Expression::Constant {
            value,
            alias: None,
        });
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
        let saved_pos = self.position;
        let saved_col = self.column;
        
        // 检查是否有AS关键字
        if self.match_keyword("AS") {
            self.skip_whitespace();
        }
        
        // 检查下一个字符是否是关键字
        let next_token = self.peek_identifier();
        if let Some(token) = next_token {
            // 检查是否是关键字
            let token_upper = token.to_uppercase();
            let keywords = ["FROM", "WHERE", "ORDER", "LIMIT", "GROUP", "HAVING", "JOIN", "ON", "IN", "AND", "OR", "NOT"];
            if keywords.contains(&token_upper.as_str()) {
                // 是关键字，不是别名
                return Ok(None);
            }
        }
        
        // 检查是否有别名
        if self.peek_char().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') {
            let alias = self.parse_identifier()?;
            Ok(Some(alias))
        } else {
            Ok(None)
        }
    }

    /// 解析FROM子句和JOIN子句
    fn parse_from_and_join_clauses(&mut self) -> Result<(String, Option<String>, Vec<JoinClause>), QueryParseError> {
        self.skip_whitespace();
        self.expect_keyword("FROM")?;
        
        self.skip_whitespace();
        let table_name = self.parse_identifier()?;
        
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
                        },
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
                        },
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
                        },
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
                        },
                        "JOIN" => {
                            self.parse_identifier()?;
                            JoinType::Inner
                        },
                        _ => break, // 不是JOIN关键字，退出循环
                    }
                },
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
            
            Ok(Some(GroupByClause { expressions, fields }))
        } else {
            Ok(None)
        }
    }
    
    /// 解析ORDER BY子句（可选）
    fn parse_order_by_clause(&mut self) -> Result<Option<OrderByClause>, QueryParseError> {
        self.skip_whitespace();
        
        if self.match_keyword("ORDER") {
            self.skip_whitespace();
            self.expect_keyword("BY")?;
            
            self.skip_whitespace();
            // 解析ORDER BY子句中的字段，可以是标识符或位置索引（数字）
            let field = if let Some(c) = self.peek_char() {
                if c.is_ascii_digit() {
                    // 解析数字作为位置索引
                    self.parse_number()?.to_string()
                } else {
                    // 解析标识符作为字段名
                    self.parse_identifier()?
                }
            } else {
                // 解析标识符作为字段名
                self.parse_identifier()?
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

    /// 解析条件表达式
    fn parse_condition(&mut self) -> Result<Condition, QueryParseError> {
        // 保存当前位置，用于回溯
        let saved_pos = self.position;
        let saved_col = self.column;
        
        // 尝试解析字段名
        let field = self.parse_identifier()?;
        
        self.skip_whitespace();
        
        // 检查是否是BETWEEN条件
        if self.match_keyword("BETWEEN") {
            self.skip_whitespace();
            let min_value = self.parse_value()?;
            
            self.skip_whitespace();
            self.expect_keyword("AND")?;
            
            self.skip_whitespace();
            let max_value = self.parse_value()?;
            
            Ok(Condition::Between(BetweenCondition {
                field,
                min_value,
                max_value,
            }))
        } else {
            // 不是BETWEEN条件，回溯并解析为普通比较条件
            self.position = saved_pos;
            self.column = saved_col;
            
            let comparison = self.parse_comparison()?;
            Ok(Condition::Comparison(comparison))
        }
    }

    /// 解析比较条件
    fn parse_comparison(&mut self) -> Result<ComparisonCondition, QueryParseError> {
        // 解析字段名
        let field = self.parse_identifier()?;
        
        self.skip_whitespace();
        
        // 解析比较运算符
        let operator = self.parse_operator()?;
        
        self.skip_whitespace();
        
        // 解析值
        let value = self.parse_value()?;
        
        Ok(ComparisonCondition {
            field,
            operator,
            value,
        })
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
        let saved_pos = self.position;
        
        if self.peek_char() == Some('"') || self.peek_char() == Some('\'') {
            // 字符串值
            let quote_char = self.next_char().unwrap();
            let mut string_value = String::new();
            
            while let Some(c) = self.next_char() {
                if c == quote_char {
                    break;
                }
                string_value.push(c);
            }
            
            // 尝试将字符串解析为时间值
            if let Ok(timestamp) = parse_time_string(&string_value) {
                Ok(Value::Integer(timestamp))
            } else {
                // 不是时间格式，作为普通字符串处理
                Ok(Value::String(string_value))
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
            // 处理TIMEZONE()函数
            self.skip_whitespace();
            self.expect_char('(')?;
            self.skip_whitespace();
            // 解析时区名称或偏移量
            let tz_param = self.parse_value()?;
            self.skip_whitespace();
            self.expect_char(')')?;
            // 这里返回0作为占位符，实际执行时会处理
            Ok(Value::Integer(0))
        } else if self.match_keyword("TO_CHAR") {
            // 处理TO_CHAR()函数
            self.skip_whitespace();
            self.expect_char('(')?;
            self.skip_whitespace();
            // 解析时间值参数
            let time_param = self.parse_value()?;
            self.skip_whitespace();
            self.expect_char(',')?;
            self.skip_whitespace();
            // 解析格式字符串参数
            let format_param = self.parse_value()?;
            self.skip_whitespace();
            self.expect_char(')')?;
            // 这里返回0作为占位符，实际执行时会处理
            Ok(Value::Integer(0))
        } else if self.match_keyword("TO_ISO8601") {
            // 处理TO_ISO8601()函数
            self.skip_whitespace();
            self.expect_char('(')?;
            self.skip_whitespace();
            // 解析时间值参数
            let time_param = self.parse_value()?;
            self.skip_whitespace();
            self.expect_char(')')?;
            // 这里返回0作为占位符，实际执行时会处理
            Ok(Value::Integer(0))
        } else if self.match_keyword("TO_EPOCH") {
            // 处理TO_EPOCH()函数
            self.skip_whitespace();
            self.expect_char('(')?;
            self.skip_whitespace();
            // 解析时间值参数
            let time_param = self.parse_value()?;
            self.skip_whitespace();
            self.expect_char(')')?;
            // 这里返回0作为占位符，实际执行时会处理
            Ok(Value::Integer(0))
        } else if self.peek_char() == Some('[') {
            // 数组字面量（用于向量操作符）
            self.next_char(); // 跳过'['
            self.skip_whitespace();
            
            let mut values = Vec::new();
            
            // 解析数组元素
            loop {
                self.skip_whitespace();
                
                // 检查是否到达数组末尾
                if self.peek_char() == Some(']') {
                    break;
                }
                
                // 解析数组元素
                let value = self.parse_value()?;
                values.push(value);
                
                self.skip_whitespace();
                
                // 检查是否还有下一个元素
                if self.match_char(',') {
                    continue;
                } else if self.peek_char() == Some(']') {
                    break;
                } else {
                    return Err(QueryParseError::InvalidSyntax);
                }
            }
            
            self.next_char(); // 跳过']'
            
            // 暂时返回第一个值作为占位符，实际执行时会处理数组
            if let Some(first_value) = values.first() {
                Ok(first_value.clone())
            } else {
                // 空数组，返回NULL
                Ok(Value::Null)
            }
        } else if self.peek_char().is_some_and(|c| c.is_ascii_digit() || c == '-') {
            // 数字值
            let number_str = self.parse_number_str()?;
            if number_str.contains('.') {
                // 浮点数
                let float_value = number_str.parse::<f64>().map_err(|_| QueryParseError::InvalidValue)?;
                Ok(Value::Float(float_value))
            } else {
                // 整数
                let int_value = number_str.parse::<i64>().map_err(|_| QueryParseError::InvalidValue)?;
                Ok(Value::Integer(int_value))
            }
        } else {
            // 回溯，尝试解析为标识符
            self.position = saved_pos;
            let identifier = self.parse_identifier()?;
            
            // 检查是否是带有AT TIME ZONE修饰符的时间表达式
            self.skip_whitespace();
            if self.match_keyword("AT") {
                self.skip_whitespace();
                self.expect_keyword("TIME")?;
                self.skip_whitespace();
                self.expect_keyword("ZONE")?;
                self.skip_whitespace();
                
                // 解析时区名称或偏移量
                let tz_value = self.parse_value()?;
                // 这里返回0作为占位符，实际执行时会处理
                Ok(Value::Integer(0))
            } else {
                // 不是AT TIME ZONE表达式，返回标识符
                Ok(Value::Identifier(identifier))
            }
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
        
        // 后续字符可以是字母、数字、下划线或点号（支持表别名，如t.id）
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
        number_str.parse::<usize>().map_err(|_| QueryParseError::InvalidValue)
    }

    /// 匹配关键字
    fn match_keyword(&mut self, keyword: &str) -> bool {
        let start = self.position;
        let end = start + keyword.len();
        
        if end <= self.input.len() {
            let actual = &self.input[start..end];
            let expected = keyword;
            
            if actual.eq_ignore_ascii_case(expected) {
                // 检查是否是完整的关键字（后面跟着非字母数字字符）
                let next_char = self.input.chars().nth(end);
                if next_char.is_none() || !next_char.unwrap().is_ascii_alphanumeric() {
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
        let end = start + s.len();
        
        if end <= self.input.len() {
            if &self.input[start..end] == s {
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
        self.input.chars().nth(self.position)
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
        while self.peek_char().is_some() {
            // 检查是否是约束关键字（这些应该在数据类型之后单独解析）
            let next_token = self.peek_identifier();
            if let Some(token) = next_token {
                let token_upper = token.to_uppercase();
                // 如果遇到约束关键字，停止解析数据类型
                if ["PRIMARY", "NOT", "UNIQUE", "AUTOINCREMENT", "AUTO_INCREMENT", "DEFAULT"].contains(&token_upper.as_str()) {
                    break;
                }
            }
            
            // 检查是否是修饰符（字母或下划线开头）
            let c = self.peek_char().unwrap();
            if c.is_ascii_alphabetic() || c == '_' {
                // 解析修饰符
                let modifier = self.parse_identifier()?;
                result.push(' ');
                result.push_str(&modifier);
                
                // 检查是否是 WITH 修饰符，如 WITH TIME ZONE 或 WITH DISTANCE=L2
                if modifier.eq_ignore_ascii_case("WITH") {
                    self.skip_whitespace();
                    while self.peek_char().is_some() {
                        // 检查是否是约束关键字
                        let next_token = self.peek_identifier();
                        if let Some(token) = next_token {
                            let token_upper = token.to_uppercase();
                            if ["PRIMARY", "NOT", "UNIQUE", "AUTOINCREMENT", "AUTO_INCREMENT", "DEFAULT"].contains(&token_upper.as_str()) {
                                break;
                            }
                        }
                        
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
                        
                        self.skip_whitespace();
                    }
                }
                
                self.skip_whitespace();
            } else {
                break;
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
}

/// 解析SQL查询字符串
pub fn parse_sql_query(sql: &str) -> Result<SqlQuery, QueryParseError> {
    let mut parser = SqlParser::new(sql.to_string());
    parser.parse()
}