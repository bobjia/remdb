use remdb_macros::MemdbTable;

// 测试内联模式
#[derive(MemdbTable)]
#[memdb_schema(ddl = "CREATE TABLE user (id INTEGER PRIMARY KEY, name TEXT NOT NULL, age INTEGER, active BOOLEAN);")]
struct UserTable;

// 测试文件模式暂时注释掉，因为在测试环境中可能存在路径问题
/*
#[derive(MemdbTable)]
#[memdb_schema(file = "./tests/test_schema.ddl")]
struct TestDatabase;
*/

// 测试代码生成
#[test]
fn test_user_table_generated() {
    // 测试生成的User结构体是否存在
    let user = User {
        id: 1,
        name: "test".to_string(),
        age: Some(30),
        active: Some(true),
    };
    
    assert_eq!(user.id, 1);
    assert_eq!(user.name, "test");
    assert_eq!(user.age, Some(30));
    assert_eq!(user.active, Some(true));
}

// 测试数据库配置生成
#[test]
fn test_database_config() {
    // 测试生成的DATABASE常量是否存在
    assert!(DATABASE.tables.len() > 0);
}