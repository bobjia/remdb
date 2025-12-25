use remdb_macros::MemdbTable;

// 使用内联DDL定义表
#[derive(MemdbTable)]
#[memdb_schema(ddl = "CREATE TABLE user (id INTEGER PRIMARY KEY, name TEXT NOT NULL, age INTEGER, active BOOLEAN);")]
struct UserTable;

fn main() {
    // 测试生成的User结构体
    let user = User {
        id: 1,
        name: "Alice".to_string(),
        age: Some(30),
        active: Some(true),
    };
    
    println!("Generated User struct: {:?}", user);
    println!("User name: {}", user.name);
    println!("User age: {:?}", user.age);
    
    // 测试数据库配置
    println!("Database tables count: {}", DATABASE.tables.len());
    
    // 测试API函数（虽然是占位符）
    // user::insert(&mut db, user);
    // let result = user::get_by_id(&db, 1);
    
    println!("DDL macro example completed successfully!");
}