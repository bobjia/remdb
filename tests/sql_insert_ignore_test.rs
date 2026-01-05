extern crate alloc;
use alloc::sync::Arc;
use remdb::{RemDb, RemDbConfig, sql::query_parser::{SqlParser, SqlQuery}};

#[test]
fn test_insert_ignore_parsing() {
    // 测试INSERT IGNORE语法解析
    let sql = "INSERT IGNORE INTO test_table (id, name) VALUES (1, 'test')";
    let mut parser = SqlParser::new(sql);
    let result = parser.parse();
    
    assert!(result.is_ok(), "INSERT IGNORE解析应该成功");
    
    if let Ok(query) = result {
        assert_eq!(query.query_type, remdb::sql::QueryType::Insert);
        assert!(query.ignore_duplicates, "INSERT IGNORE应该设置ignore_duplicates为true");
    }
    
    // 测试普通INSERT语法解析
    let sql = "INSERT INTO test_table (id, name) VALUES (1, 'test')";
    let mut parser = SqlParser::new(sql);
    let result = parser.parse();
    
    assert!(result.is_ok(), "普通INSERT解析应该成功");
    
    if let Ok(query) = result {
        assert_eq!(query.query_type, remdb::sql::QueryType::Insert);
        assert!(!query.ignore_duplicates, "普通INSERT应该设置ignore_duplicates为false");
    }
    
    println!("INSERT IGNORE解析测试通过!");
}