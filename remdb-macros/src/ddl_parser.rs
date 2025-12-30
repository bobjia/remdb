pub struct ColumnDef {
    pub name: String,
    pub typ: String,
    pub nullable: bool,
    pub primary_key: bool,
    pub unique: bool,
    pub auto_increment: bool,
}

pub struct IndexDef {
    pub name: String,
    pub table_name: String,
    pub field: String,
    pub index_type: String,
}

pub struct TableDef {
    pub name: String,
    pub columns: Vec<ColumnDef>,
    pub indices: Vec<IndexDef>,
}

pub fn parse_ddl(ddl: &str) -> Result<Vec<TableDef>, String> {
    let mut table_defs = Vec::new();
    let mut current_table: Option<TableDef> = None;
    let mut indices = Vec::new();
    
    // 将DDL语句按分号分割
    let statements = ddl.split(';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    
    for stmt in statements {
        let stmt_lower = stmt.to_lowercase();
        
        if stmt_lower.starts_with("create table") {
            // 解析CREATE TABLE语句
            let table = parse_create_table(stmt)?;
            if let Some(existing_table) = current_table.take() {
                // 保存之前的表
                table_defs.push(existing_table);
            }
            current_table = Some(table);
        } else if stmt_lower.starts_with("create index") {
            // 解析CREATE INDEX语句
            let index = parse_create_index(stmt)?;
            indices.push(index);
        }
    }
    
    // 保存最后一个表
    if let Some(table) = current_table.take() {
        table_defs.push(table);
    }
    
    // 将索引分配给对应的表
    for index in indices {
        if let Some(table) = table_defs.iter_mut().find(|t| t.name == index.table_name) {
            table.indices.push(index);
        }
    }
    
    Ok(table_defs)
}

fn parse_create_table(stmt: &str) -> Result<TableDef, String> {
    // 简化的CREATE TABLE解析
    // 格式：CREATE TABLE table_name (column1 type constraints, column2 type constraints, ...)
    let stmt = stmt.to_lowercase();
    let table_name_start = stmt.find("table").ok_or("Invalid CREATE TABLE statement")? + 6;
    
    // 找到左括号位置
    let left_paren = stmt.find('(').ok_or("Invalid CREATE TABLE statement")?;
    
    // 提取表名
    let table_name = stmt[table_name_start..left_paren].trim().to_string();
    
    // 提取列定义
    let columns_part = &stmt[left_paren + 1..stmt.rfind(')').ok_or("Invalid CREATE TABLE statement")?];
    let columns = parse_columns(columns_part)?;
    
    Ok(TableDef {
        name: table_name,
        columns,
        indices: Vec::new(),
    })
}

fn parse_columns(columns_part: &str) -> Result<Vec<ColumnDef>, String> {
    let mut columns = Vec::new();
    let column_defs = columns_part.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    
    for column_def in column_defs {
        let mut parts = column_def.split_whitespace();
        let name = parts.next().ok_or("Invalid column definition")?.to_string();
        
        let mut is_unsigned = false;
        let mut typ = String::new();
        
        // 解析类型，处理UNSIGNED修饰符
        while let Some(part) = parts.next() {
            match part {
                "unsigned" => {
                    is_unsigned = true;
                }
                _ => {
                    // 处理带括号的类型，如TEXT(32) -> TEXT
                    let typ_part = part;
                    if let Some(paren_pos) = typ_part.find('(') {
                        typ = typ_part[..paren_pos].to_string();
                    } else {
                        typ = typ_part.to_string();
                    }
                    break;
                }
            }
        }
        
        // 如果没有找到类型，返回错误
        if typ.is_empty() {
            return Err("Invalid column definition: missing type".to_string());
        }
        
        // 处理UNSIGNED修饰符
        let full_typ = if is_unsigned {
            format!("unsigned {}", typ)
        } else {
            typ
        };
        
        let mut nullable = true;
        let mut primary_key = false;
        let mut unique = false;
        let mut auto_increment = false;
        
        // 解析约束
        while let Some(part) = parts.next() {
            match part {
                "not" => {
                    let next = parts.next().unwrap_or("");
                    if next == "null" {
                        nullable = false;
                    }
                }
                "primary" => {
                    let next = parts.next().unwrap_or("");
                    if next == "key" {
                        primary_key = true;
                    }
                }
                "unique" => {
                    unique = true;
                }
                "autoincrement" | "auto_increment" => {
                    auto_increment = true;
                }
                _ => {}
            }
        }
        
        columns.push(ColumnDef {
            name,
            typ: full_typ,
            nullable,
            primary_key,
            unique,
            auto_increment,
        });
    }
    
    Ok(columns)
}

fn parse_create_index(stmt: &str) -> Result<IndexDef, String> {
    // 简化的CREATE INDEX解析
    // 格式：CREATE INDEX index_name ON table_name USING index_type (field)
    let stmt = stmt.to_lowercase();
    
    // 提取索引名
    let index_name_start = stmt.find("index").ok_or("Invalid CREATE INDEX statement")? + 6;
    let on_pos = stmt.find("on").ok_or("Invalid CREATE INDEX statement")?;
    let index_name = stmt[index_name_start..on_pos].trim().to_string();
    
    // 提取表名
    let table_name_start = on_pos + 3;
    let using_pos = stmt.find("using").unwrap_or_else(|| {
        // 如果没有USING子句，找到左括号位置
        stmt.find('(').unwrap_or(stmt.len())
    });
    let table_name = stmt[table_name_start..using_pos].trim().to_string();
    
    // 提取索引类型
    let mut index_type = "btree".to_string();
    let mut field_start = using_pos;
    
    if stmt.contains("using") {
        let index_type_start = using_pos + 6;
        let left_paren = stmt.find('(').ok_or("Invalid CREATE INDEX statement")?;
        index_type = stmt[index_type_start..left_paren].trim().to_string();
        field_start = left_paren;
    }
    
    // 提取字段名
    let left_paren = field_start;
    let right_paren = stmt.rfind(')').ok_or("Invalid CREATE INDEX statement")?;
    let field = stmt[left_paren + 1..right_paren].trim().to_string();
    
    Ok(IndexDef {
        name: index_name,
        table_name,
        field,
        index_type,
    })
}
