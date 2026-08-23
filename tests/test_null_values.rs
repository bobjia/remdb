//! 测试NULL值支持
//!
//! 该测试文件验证SQL中的NULL值处理功能。

#![cfg(feature = "std")]

use remdb::sql::query_parser::{QueryType, SqlParser};

#[test]
fn test_null_values_support() {
    // 测试SQL解析器对IS NULL语法的支持
    println!("=== 测试SQL解析器对IS NULL语法的支持 ===");

    // 测试SELECT语句，包含IS NULL检查和ORDER BY子句
    let select_sql = "SELECT int_val IS NULL as int_null, text_val IS NULL as text_null FROM test_null_values ORDER BY id";
    let mut parser = SqlParser::new(select_sql.to_string());
    match parser.parse() {
        Ok(query) => {
            println!("✓ 成功解析IS NULL查询: {}", select_sql);
            // 验证查询结构
            assert_eq!(query.query_type, QueryType::Select);
            assert_eq!(query.table_name, "test_null_values");
            assert_eq!(query.columns.len(), 2);
            assert!(query.order_by.is_some());
            if let Some(order_by) = &query.order_by {
                assert_eq!(order_by.field, "id");
            }
        }
        Err(e) => {
            panic!("✗ 解析IS NULL查询失败: {:?}", e);
        }
    }

    // 测试INSERT语句，包含TRUE布尔值
    let insert_sql = "INSERT INTO test_null_values (id, int_val, text_val, bool_val) VALUES (2, 100, 'test', TRUE)";
    let mut parser = SqlParser::new(insert_sql.to_string());
    match parser.parse() {
        Ok(query) => {
            println!("✓ 成功解析INSERT查询: {}", insert_sql);
            // 验证查询结构
            assert_eq!(query.query_type, QueryType::Insert);
            assert_eq!(query.table_name, "test_null_values");
            assert_eq!(query.insert_columns.len(), 4);
            assert_eq!(query.values.len(), 1);
        }
        Err(e) => {
            panic!("✗ 解析INSERT查询失败: {:?}", e);
        }
    }

    println!("=== 所有测试通过 ===");
}
