pub struct ColumnDef {
    pub name: String,
    pub typ: String,
    pub nullable: bool,
    pub primary_key: bool,
}

pub struct TableDef {
    pub name: String,
    pub columns: Vec<ColumnDef>,
}

pub fn parse_ddl(_ddl: &str) -> Result<Vec<TableDef>, String> {
    // 简化的DDL解析实现
    // 实际项目中应该使用更完善的解析器
    Ok(vec![])
}
