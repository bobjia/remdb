use syn::Ident;
use proc_macro2::Span;

/// 解析SQLite3语法兼容的DDL文件，生成表定义

/// SQL数据类型
#[derive(Debug, Clone, PartialEq)]
pub enum SqlType {
    Int8,
    Int16,
    Int32,
    Int64,
    UInt8,
    UInt16,
    UInt32,
    UInt64,
    Text,
    Real,
    Boolean,
    Timestamp,
}

impl SqlType {
    /// 将SQL类型转换为Rust类型
    pub fn to_rust_type(&self, not_null: bool) -> String {
        let base_type = match self {
            SqlType::Int8 => "i8",
            SqlType::Int16 => "i16",
            SqlType::Int32 => "i32",
            SqlType::Int64 => "i64",
            SqlType::UInt8 => "u8",
            SqlType::UInt16 => "u16",
            SqlType::UInt32 => "u32",
            SqlType::UInt64 => "u64",
            SqlType::Text => "String",
            SqlType::Real => "f64",
            SqlType::Boolean => "bool",
            SqlType::Timestamp => "u64",
        };
        
        if not_null {
            base_type.to_string()
        } else {
            format!("Option<{base_type}>")
        }
    }
    
    /// 将SQL类型转换为remdb DataType
    pub fn to_data_type(&self) -> String {
        match self {
            SqlType::Int8 => "Int8".to_string(),
            SqlType::Int16 => "Int16".to_string(),
            SqlType::Int32 => "Int32".to_string(),
            SqlType::Int64 => "Int64".to_string(),
            SqlType::UInt8 => "UInt8".to_string(),
            SqlType::UInt16 => "UInt16".to_string(),
            SqlType::UInt32 => "UInt32".to_string(),
            SqlType::UInt64 => "UInt64".to_string(),
            SqlType::Text => "String".to_string(),
            SqlType::Real => "Float64".to_string(),
            SqlType::Boolean => "Bool".to_string(),
            SqlType::Timestamp => "Timestamp".to_string(),
        }
    }
}

/// 列约束
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnConstraint {
    PrimaryKey,
    NotNull,
    Unique,
}

/// 列定义
#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub name: String,
    pub sql_type: SqlType,
    pub constraints: Vec<ColumnConstraint>,
    pub not_null: bool,
    pub is_primary_key: bool,
}

impl ColumnDef {
    pub fn new(name: String, sql_type: SqlType) -> Self {
        Self {
            name,
            sql_type,
            constraints: Vec::new(),
            not_null: false,
            is_primary_key: false,
        }
    }
    
    pub fn add_constraint(&mut self, constraint: ColumnConstraint) {
        // 先复制约束用于匹配
        let constraint_copy = constraint.clone();
        self.constraints.push(constraint);
        
        match constraint_copy {
            ColumnConstraint::PrimaryKey => {
                self.is_primary_key = true;
                self.not_null = true;
            },
            ColumnConstraint::NotNull => {
                self.not_null = true;
            },
            _ => {},
        }
    }
}

/// 索引定义
#[derive(Debug, Clone)]
pub struct IndexDef {
    pub name: String,
    pub table_name: String,
    pub index_type: String,
    pub column_name: String,
}

/// 表定义
#[derive(Debug, Clone)]
pub struct TableDef {
    pub name: String,
    pub columns: Vec<ColumnDef>,
    pub primary_key: Option<String>,
    pub indices: Vec<IndexDef>,
}

impl TableDef {
    pub fn new(name: String) -> Self {
        Self {
            name,
            columns: Vec::new(),
            primary_key: None,
            indices: Vec::new(),
        }
    }
    
    pub fn add_column(&mut self, column: ColumnDef) {
        // 设置主键
        if column.is_primary_key {
            self.primary_key = Some(column.name.clone());
        }
        self.columns.push(column);
    }
}

/// DDL解析器错误
#[derive(Debug, Clone)]
pub struct DdlParseError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl std::fmt::Display for DdlParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at line {} column {}", self.message, self.line, self.column)
    }
}

impl std::error::Error for DdlParseError {}

/// DDL解析器
pub struct DdlParser {
    input: String,
    position: usize,
    line: usize,
    column: usize,
}

impl DdlParser {
    pub fn new(input: String) -> Self {
        Self {
            input,
            position: 0,
            line: 1,
            column: 1,
        }
    }
    
    /// 解析整个DDL文件，返回表定义列表
    pub fn parse(&mut self) -> Result<Vec<TableDef>, DdlParseError> {
        let mut tables = Vec::new();
        
        while !self.is_eof() {
            self.skip_whitespace();
            if self.is_eof() {
                break;
            }
            
            // 解析CREATE语句
            if self.match_keyword("CREATE") {
                self.skip_whitespace();
                if self.match_keyword("TABLE") {
                    let table = self.parse_create_table()?;
                    tables.push(table);
                } else if self.match_keyword("INDEX") {
                    let index = self.parse_create_index()?;
                    self.add_index_to_table(&index, &mut tables)?;
                } else {
                    return Err(self.error("Expected 'TABLE' or 'INDEX' after 'CREATE'"));
                }
            } else {
                // 跳过未知语句
                self.skip_statement();
            }
        }
        
        Ok(tables)
    }
    
    /// 解析CREATE INDEX语句
    fn parse_create_index(&mut self) -> Result<IndexDef, DdlParseError> {
        self.skip_whitespace();
        
        // 解析索引名称
        let index_name = self.parse_identifier()?;
        
        self.skip_whitespace();
        self.expect_keyword("ON")?;
        
        self.skip_whitespace();
        // 解析表名
        let table_name = self.parse_identifier()?;
        
        self.skip_whitespace();
        
        // 解析索引类型（可选）
        let mut index_type = "BTree".to_string();
        if self.match_keyword("USING") {
            self.skip_whitespace();
            let type_name = self.parse_word()?.to_uppercase();
            // 验证索引类型
            match type_name.as_str() {
                "HASH" | "SORTEDARRAY" | "BTREE" | "TTREE" => {
                    index_type = type_name;
                },
                _ => {
                    return Err(self.error(&format!("Unsupported index type: {}", type_name)));
                }
            }
        }
        
        self.skip_whitespace();
        self.expect_char('(')?;
        
        self.skip_whitespace();
        // 解析索引列
        let column_name = self.parse_identifier()?;
        
        self.skip_whitespace();
        self.expect_char(')')?;
        
        // 跳过语句结束符
        self.skip_statement();
        
        Ok(IndexDef {
            name: index_name,
            table_name: table_name,
            index_type: index_type,
            column_name: column_name,
        })
    }
    
    /// 将索引添加到对应的表
    fn add_index_to_table(&self, index: &IndexDef, tables: &mut Vec<TableDef>) -> Result<(), DdlParseError> {
        for table in tables {
            if table.name == index.table_name {
                table.indices.push(index.clone());
                return Ok(());
            }
        }
        
        Err(self.error(&format!("Table not found: {}", index.table_name)))
    }
    
    /// 解析CREATE TABLE语句
    fn parse_create_table(&mut self) -> Result<TableDef, DdlParseError> {
        self.skip_whitespace();
        
        // 解析表名
        let table_name = self.parse_identifier()?;
        let mut table = TableDef::new(table_name);
        
        self.skip_whitespace();
        self.expect_char('(')?;
        
        // 解析列定义列表
        loop {
            self.skip_whitespace();
            if self.peek_char() == Some(')') {
                self.next_char();
                break;
            }
            
            // 解析列定义
            let column = self.parse_column_def()?;
            table.add_column(column);
            
            self.skip_whitespace();
            if self.peek_char() == Some(',') {
                self.next_char();
            } else if self.peek_char() == Some(')') {
                // 结束列定义
            } else {
                return Err(self.error("Expected ',' or ')' after column definition"));
            }
        }
        
        // 跳过表级约束（暂时不支持）
        self.skip_statement();
        
        Ok(table)
    }
    
    /// 解析列定义
    fn parse_column_def(&mut self) -> Result<ColumnDef, DdlParseError> {
        // 解析列名
        let column_name = self.parse_identifier()?;
        
        self.skip_whitespace();
        
        // 解析数据类型
        let sql_type = self.parse_data_type()?;
        
        let mut column = ColumnDef::new(column_name, sql_type);
        
        // 解析列约束
        loop {
            self.skip_whitespace();
            let next = self.peek_char();
            
            if next == Some(',') || next == Some(')') {
                break;
            }
            
            if self.match_keyword("PRIMARY") {
                self.skip_whitespace();
                self.expect_keyword("KEY")?;
                column.add_constraint(ColumnConstraint::PrimaryKey.clone());
            } else if self.match_keyword("NOT") {
                self.skip_whitespace();
                self.expect_keyword("NULL")?;
                column.add_constraint(ColumnConstraint::NotNull.clone());
            } else if self.match_keyword("UNIQUE") {
                column.add_constraint(ColumnConstraint::Unique.clone());
            } else {
                // 未知约束，跳过
                self.skip_unknown();
            }
        }
        
        Ok(column)
    }
    
    /// 解析数据类型
    fn parse_data_type(&mut self) -> Result<SqlType, DdlParseError> {
        let type_name = self.parse_word()?.to_uppercase();
        
        match type_name.as_str() {
            "INT8" => Ok(SqlType::Int8),
            "INT16" => Ok(SqlType::Int16),
            "INT32" => Ok(SqlType::Int32),
            "INT64" => Ok(SqlType::Int64),
            "UINT8" => Ok(SqlType::UInt8),
            "UINT16" => Ok(SqlType::UInt16),
            "UINT32" => Ok(SqlType::UInt32),
            "UINT64" => Ok(SqlType::UInt64),
            "INTEGER" => Ok(SqlType::Int64), // 向后兼容，INTEGER映射为Int64
            "TEXT" => Ok(SqlType::Text),
            "REAL" => Ok(SqlType::Real),
            "BOOLEAN" => Ok(SqlType::Boolean),
            "TIMESTAMP" => Ok(SqlType::Timestamp),
            _ => Err(self.error(&format!("Unsupported data type: {}", type_name))),
        }
    }
    
    /// 解析标识符（表名、列名）
    fn parse_identifier(&mut self) -> Result<String, DdlParseError> {
        let start = self.position;
        
        // 标识符必须以字母或下划线开头
        let c = self.peek_char().ok_or_else(|| self.error("Unexpected end of input"))?;
        if !c.is_alphabetic() && c != '_' {
            return Err(self.error(&format!("Invalid identifier start: {:?}", c)));
        }
        
        self.next_char();
        
        // 后续字符可以是字母、数字或下划线
        while let Some(c) = self.peek_char() {
            if c.is_alphanumeric() || c == '_' {
                self.next_char();
            } else {
                break;
            }
        }
        
        Ok(self.input[start..self.position].to_string())
    }
    
    /// 解析单词（关键字或类型）
    fn parse_word(&mut self) -> Result<String, DdlParseError> {
        let start = self.position;
        
        while let Some(c) = self.peek_char() {
            if c.is_alphanumeric() {
                self.next_char();
            } else {
                break;
            }
        }
        
        if self.position == start {
            return Err(self.error("Expected word"));
        }
        
        Ok(self.input[start..self.position].to_string())
    }
    
    /// 匹配关键字（不区分大小写）
    fn match_keyword(&mut self, keyword: &str) -> bool {
        let start = self.position;
        let end = start + keyword.len();
        
        if end <= self.input.len() {
            let actual = self.input[start..end].to_uppercase();
            let expected = keyword.to_uppercase();
            if actual == expected {
                self.position = end;
                self.column += keyword.len();
                true
            } else {
                false
            }
        } else {
            false
        }
    }
    
    /// 跳过未知单词
    fn skip_unknown(&mut self) {
        while let Some(c) = self.peek_char() {
            if c.is_alphanumeric() {
                self.next_char();
            } else {
                break;
            }
        }
    }
    
    /// 期望关键字，不区分大小写
    fn expect_keyword(&mut self, keyword: &str) -> Result<(), DdlParseError> {
        if self.match_keyword(keyword) {
            Ok(())
        } else {
            Err(self.error(&format!("Expected keyword: {}", keyword)))
        }
    }
    
    /// 期望字符
    fn expect_char(&mut self, c: char) -> Result<(), DdlParseError> {
        if self.peek_char() == Some(c) {
            self.next_char();
            Ok(())
        } else {
            Err(self.error(&format!("Expected character: {:?}", c)))
        }
    }
    
    /// 跳过当前语句
    fn skip_statement(&mut self) {
        while let Some(c) = self.next_char() {
            if c == ';' {
                break;
            }
        }
    }
    
    /// 跳过空白字符、注释
    fn skip_whitespace(&mut self) {
        loop {
            while let Some(c) = self.peek_char() {
                if c.is_whitespace() {
                    self.next_char();
                } else {
                    break;
                }
            }
            
            // 跳过注释
            if self.peek_char() == Some('/') && self.peek_nth_char(1) == Some('*') {
                // 多行注释 /* */
                self.next_char();
                self.next_char();
                while let Some(c) = self.next_char() {
                    if c == '*' && self.peek_char() == Some('/') {
                        self.next_char();
                        break;
                    }
                }
            } else if self.peek_char() == Some('-') && self.peek_nth_char(1) == Some('-') {
                // 单行注释 --
                self.next_char();
                self.next_char();
                while let Some(c) = self.next_char() {
                    if c == '\n' {
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }
    
    /// 查看当前字符
    fn peek_char(&self) -> Option<char> {
        self.input.as_bytes().get(self.position).map(|&b| b as char)
    }
    
    /// 查看第n个字符
    fn peek_nth_char(&self, n: usize) -> Option<char> {
        self.input.as_bytes().get(self.position + n).map(|&b| b as char)
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
    
    /// 判断是否到达文件末尾
    fn is_eof(&self) -> bool {
        self.position >= self.input.len()
    }
    
    /// 创建错误信息
    fn error(&self, message: &str) -> DdlParseError {
        DdlParseError {
            message: message.to_string(),
            line: self.line,
            column: self.column,
        }
    }
}

/// 解析标识符为syn::Ident
pub fn parse_ident(name: &str) -> Ident {
    Ident::new(name, Span::call_site())
}
