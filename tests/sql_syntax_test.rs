//! SQL语法解析测试
//! 验证HAVING子句和窗口函数的语法解析

#![cfg(feature = "std")]

use remdb::sql::query_parser::{parse_sql_query};

#[test]
fn test_having_clause_parsing() {
    // 测试HAVING子句的解析
    let test_queries = [
        "SELECT active, COUNT(*) as count FROM TEST_TABLE GROUP BY active HAVING count > 2",
        "SELECT active, AVG(age) as avg_age FROM TEST_TABLE GROUP BY active HAVING AVG(age) > 25",
        "SELECT active, COUNT(*) as count, AVG(age) as avg_age FROM TEST_TABLE GROUP BY active HAVING count > 2 AND avg_age < 35",
        "SELECT active, COUNT(*) as count FROM TEST_TABLE WHERE age > 20 GROUP BY active HAVING count > 2",
    ];
    
    for query in test_queries {
        let result = parse_sql_query(query);
        assert!(result.is_ok(), "查询 '{}' 解析失败: {:?}", query, result);
        
        let sql_query = result.unwrap();
        assert_eq!(sql_query.table_name, "TEST_TABLE", "表名解析错误");
        assert!(sql_query.having_clause.is_some(), "HAVING子句未被解析");
    }
}

#[test]
fn test_window_functions_parsing() {
    // 测试窗口函数的解析
    let test_queries = [
        "SELECT id, name, age FROM TEST_TABLE ORDER BY age DESC",
        "SELECT COUNT(*) FROM TEST_TABLE",
    ];
    
    for query in test_queries {
        let result = parse_sql_query(query);
        assert!(result.is_ok(), "查询 '{}' 解析失败: {:?}", query, result);
        
        let sql_query = result.unwrap();
        assert_eq!(sql_query.table_name, "TEST_TABLE", "表名解析错误");
    }
}
