use remdb::sql::{parse_sql_query, QueryParseError};

#[test]
fn test_create_table_parse() {
    let sql =
        "CREATE TABLE products (id UINT32 PRIMARY KEY, name STRING, price FLOAT32, in_stock BOOL);";

    println!("Testing SQL: {}", sql);

    match parse_sql_query(sql) {
        Ok(query) => {
            println!("✓ Successfully parsed query: {:?}", query);
            assert_eq!(query.query_type, remdb::sql::QueryType::CreateTable);
            assert_eq!(query.if_not_exists, false);
        }
        Err(err) => {
            println!("✗ Parse error: {:?}", err);
            // 获取更详细的错误信息
            match err {
                QueryParseError::InvalidSyntax => println!("  Error type: Invalid syntax"),
                QueryParseError::UnsupportedKeyword => {
                    println!("  Error type: Unsupported keyword")
                }
                QueryParseError::InvalidTableName => println!("  Error type: Invalid table name"),
                QueryParseError::InvalidFieldName => println!("  Error type: Invalid field name"),
                QueryParseError::InvalidCondition => println!("  Error type: Invalid condition"),
                QueryParseError::InvalidOperator => println!("  Error type: Invalid operator"),
                QueryParseError::InvalidValue => println!("  Error type: Invalid value"),
                QueryParseError::MissingClause => println!("  Error type: Missing clause"),
            }
            panic!("Failed to parse CREATE TABLE statement");
        }
    }
}

#[test]
fn test_create_table_if_not_exists_parse() {
    let sql =
        "CREATE TABLE IF NOT EXISTS products (id UINT32 PRIMARY KEY, name STRING, price FLOAT32, in_stock BOOL);";

    println!("Testing SQL: {}", sql);

    match parse_sql_query(sql) {
        Ok(query) => {
            println!("✓ Successfully parsed query: {:?}", query);
            assert_eq!(query.query_type, remdb::sql::QueryType::CreateTable);
            assert_eq!(query.if_not_exists, true);
        }
        Err(err) => {
            println!("✗ Parse error: {:?}", err);
            // 获取更详细的错误信息
            match err {
                QueryParseError::InvalidSyntax => println!("  Error type: Invalid syntax"),
                QueryParseError::UnsupportedKeyword => {
                    println!("  Error type: Unsupported keyword")
                }
                QueryParseError::InvalidTableName => println!("  Error type: Invalid table name"),
                QueryParseError::InvalidFieldName => println!("  Error type: Invalid field name"),
                QueryParseError::InvalidCondition => println!("  Error type: Invalid condition"),
                QueryParseError::InvalidOperator => println!("  Error type: Invalid operator"),
                QueryParseError::InvalidValue => println!("  Error type: Invalid value"),
                QueryParseError::MissingClause => println!("  Error type: Missing clause"),
            }
            panic!("Failed to parse CREATE TABLE IF NOT EXISTS statement");
        }
    }
}

#[test]
fn test_update_parse() {
    let sql = "UPDATE products SET price = 9.99, in_stock = true WHERE id = 1;";

    println!("Testing SQL: {}", sql);

    match parse_sql_query(sql) {
        Ok(query) => {
            println!("✓ Successfully parsed query: {:?}", query);
            assert_eq!(query.query_type, remdb::sql::QueryType::Update);
            assert_eq!(query.table_name, "products");
            assert_eq!(query.update_pairs.len(), 2);
            assert_eq!(query.update_pairs[0].0, "price");
            assert_eq!(query.update_pairs[1].0, "in_stock");
        }
        Err(err) => {
            println!("✗ Parse error: {:?}", err);
            // 获取更详细的错误信息
            match err {
                QueryParseError::InvalidSyntax => println!("  Error type: Invalid syntax"),
                QueryParseError::UnsupportedKeyword => {
                    println!("  Error type: Unsupported keyword")
                }
                QueryParseError::InvalidTableName => println!("  Error type: Invalid table name"),
                QueryParseError::InvalidFieldName => println!("  Error type: Invalid field name"),
                QueryParseError::InvalidCondition => println!("  Error type: Invalid condition"),
                QueryParseError::InvalidOperator => println!("  Error type: Invalid operator"),
                QueryParseError::InvalidValue => println!("  Error type: Invalid value"),
                QueryParseError::MissingClause => println!("  Error type: Missing clause"),
            }
            panic!("Failed to parse UPDATE statement");
        }
    }
}
