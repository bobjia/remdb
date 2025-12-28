use remdb::sql::{parse_sql_query, QueryParseError};

fn main() {
    let sql = "CREATE TABLE products (id UINT32 PRIMARY KEY, name STRING, price FLOAT32, in_stock BOOL);";
    
    println!("Testing SQL: {}", sql);
    
    match parse_sql_query(sql) {
        Ok(query) => {
            println!("✓ Successfully parsed query: {:?}", query);
        },
        Err(err) => {
            println!("✗ Parse error: {:?}", err);
            // 获取更详细的错误信息
            match err {
                QueryParseError::InvalidSyntax => println!("  Error type: Invalid syntax"),
                QueryParseError::UnsupportedKeyword => println!("  Error type: Unsupported keyword"),
                QueryParseError::InvalidTableName => println!("  Error type: Invalid table name"),
                QueryParseError::InvalidFieldName => println!("  Error type: Invalid field name"),
                QueryParseError::InvalidCondition => println!("  Error type: Invalid condition"),
                QueryParseError::InvalidOperator => println!("  Error type: Invalid operator"),
                QueryParseError::InvalidValue => println!("  Error type: Invalid value"),
                QueryParseError::MissingClause => println!("  Error type: Missing clause"),
            }
        }
    }
}