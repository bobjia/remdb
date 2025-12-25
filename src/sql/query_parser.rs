//! SQL查询解析器
//! 
//! 该模块负责将SQL查询字符串解析为结构化的查询对象。

use alloc::string::String;
use alloc::vec::Vec;

/// SQL查询结构
#[derive(Debug, Clone, PartialEq)]
pub struct SqlQuery {
    /// 查询类型
    pub query_type: QueryType,
    /// 要查询的表名
    pub table_name: String,
    /// 要选择的字段列表
    pub columns: Vec<String>,
    /// 是否选择所有字段（*）
    pub select_all: bool,
    /// 查询条件
    pub where_clause: Option<WhereClause>,
    /// 排序条件
    pub order_by: Option<OrderByClause>,
    /// 结果限制
    pub limit: Option<usize>,
}

/// 查询类型
#[derive(Debug, Clone, PartialEq)]
pub enum QueryType {
    /// SELECT查询
    Select,
    /// DESCRIBE TABLE查询
    Describe,
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
    /// AND条件组合
    And(Box<Condition>, Box<Condition>),
    /// OR条件组合
    Or(Box<Condition>, Box<Condition>),
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
        
        match query_type {
            QueryType::Select => self.parse_select_query(),
            QueryType::Describe => self.parse_describe_query(),
            QueryType::Other => Err(QueryParseError::UnsupportedKeyword),
        }
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
            columns: Vec::new(),
            select_all: false,
            where_clause: None,
            order_by: None,
            limit: None,
        })
    }

    /// 解析查询类型
    fn parse_query_type(&mut self) -> Result<QueryType, QueryParseError> {
        if self.match_keyword("SELECT") {
            Ok(QueryType::Select)
        } else if self.match_keyword("DESCRIBE") {
            Ok(QueryType::Describe)
        } else {
            Ok(QueryType::Other)
        }
    }

    /// 解析SELECT查询
    fn parse_select_query(&mut self) -> Result<SqlQuery, QueryParseError> {
        // 解析SELECT子句
        let (columns, select_all) = self.parse_select_clause()?;
        
        // 解析FROM子句
        let table_name = self.parse_from_clause()?;
        
        // 解析WHERE子句（可选）
        let where_clause = self.parse_where_clause()?;
        
        // 解析ORDER BY子句（可选）
        let order_by = self.parse_order_by_clause()?;
        
        // 解析LIMIT子句（可选）
        let limit = self.parse_limit_clause()?;
        
        Ok(SqlQuery {
            query_type: QueryType::Select,
            table_name,
            columns,
            select_all,
            where_clause,
            order_by,
            limit,
        })
    }

    /// 解析SELECT子句
    fn parse_select_clause(&mut self) -> Result<(Vec<String>, bool), QueryParseError> {
        self.skip_whitespace();
        
        // 检查是否选择所有字段（*）
        if self.match_char('*') {
            Ok((Vec::new(), true))
        } else {
            // 解析字段列表
            let mut columns = Vec::new();
            
            loop {
                self.skip_whitespace();
                let column = self.parse_identifier()?;
                columns.push(column);
                
                self.skip_whitespace();
                if !self.match_char(',') {
                    break;
                }
            }
            
            Ok((columns, false))
        }
    }

    /// 解析FROM子句
    fn parse_from_clause(&mut self) -> Result<String, QueryParseError> {
        self.skip_whitespace();
        self.expect_keyword("FROM")?;
        
        self.skip_whitespace();
        let table_name = self.parse_identifier()?;
        
        Ok(table_name)
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

    /// 解析ORDER BY子句（可选）
    fn parse_order_by_clause(&mut self) -> Result<Option<OrderByClause>, QueryParseError> {
        self.skip_whitespace();
        
        if self.match_keyword("ORDER") {
            self.skip_whitespace();
            self.expect_keyword("BY")?;
            
            self.skip_whitespace();
            let field = self.parse_identifier()?;
            
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
        // 简单实现：只支持比较条件
        let comparison = self.parse_comparison()?;
        Ok(Condition::Comparison(comparison))
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
            
            Ok(Value::String(string_value))
        } else if self.match_keyword("NULL") {
            Ok(Value::Null)
        } else if self.match_keyword("TRUE") {
            Ok(Value::Boolean(true))
        } else if self.match_keyword("FALSE") {
            Ok(Value::Boolean(false))
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
            Err(QueryParseError::InvalidValue)
        }
    }

    /// 解析标识符（表名、字段名）
    fn parse_identifier(&mut self) -> Result<String, QueryParseError> {
        let start = self.position;
        
        // 标识符必须以字母或下划线开头
        let c = self.peek_char().ok_or(QueryParseError::InvalidSyntax)?;
        if !c.is_ascii_alphabetic() && c != '_' {
            return Err(QueryParseError::InvalidSyntax);
        }
        
        self.next_char();
        
        // 后续字符可以是字母、数字或下划线
        while let Some(c) = self.peek_char() {
            if c.is_ascii_alphanumeric() || c == '_' {
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
        while let Some(c) = self.peek_char() {
            if c.is_whitespace() {
                self.next_char();
            } else {
                break;
            }
        }
    }

    /// 是否已到达输入末尾
    fn is_eof(&self) -> bool {
        self.position >= self.input.len()
    }
}

/// 解析SQL查询字符串
pub fn parse_sql_query(sql: &str) -> Result<SqlQuery, QueryParseError> {
    let mut parser = SqlParser::new(sql.to_string());
    parser.parse()
}