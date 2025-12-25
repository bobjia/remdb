use remdb_macros::MemdbTable;

// 使用内联DDL定义表，包含不同类型的索引
#[derive(MemdbTable)]
#[memdb_schema(ddl = "CREATE TABLE user (id INTEGER PRIMARY KEY, name TEXT NOT NULL, age INTEGER, active BOOLEAN);
CREATE INDEX idx_user_name ON user USING btree (name);
CREATE INDEX idx_user_age ON user USING hash (age);
CREATE INDEX idx_user_active ON user USING sortedarray (active);

CREATE TABLE product (id INTEGER PRIMARY KEY, name TEXT NOT NULL, price REAL NOT NULL, category TEXT);
CREATE INDEX idx_product_price ON product USING ttree (price);
CREATE INDEX idx_product_category ON product (category); -- 默认BTree")]
struct Database;

fn main() {
    println!("=== remdb DDL Index Example ===");
    
    // 测试生成的User结构体
    println!("\n1. Testing User struct:");
    let user = User {
        id: 1,
        name: "Alice".to_string(),
        age: Some(30),
        active: Some(true),
    };
    
    println!("   Generated User struct: {:?}", user);
    println!("   User name: {}", user.name);
    println!("   User age: {:?}", user.age);
    
    // 测试生成的Product结构体
    println!("\n2. Testing Product struct:");
    let product = Product {
        id: 1,
        name: "Laptop".to_string(),
        price: 999.99,
        category: Some("Electronics".to_string()),
    };
    
    println!("   Generated Product struct: {:?}", product);
    println!("   Product name: {}", product.name);
    println!("   Product price: {}", product.price);
    
    // 测试数据库配置
    println!("\n3. Testing Database Configuration:");
    println!("   Database tables count: {}", DATABASE.tables.len());
    
    // 测试索引信息
    println!("\n4. Testing Index Information:");
    
    // 获取user表元数据
    let user_table = &DATABASE.tables[0];
    println!("   User table name: {}", user_table.name);
    println!("   User table primary key: {}", user_table.primary_key);
    println!("   User table secondary index: {:?}", user_table.secondary_index);
    println!("   User table secondary index type: {:?}", user_table.secondary_index_type);
    
    // 获取product表元数据
    let product_table = &DATABASE.tables[1];
    println!("   Product table name: {}", product_table.name);
    println!("   Product table primary key: {}", product_table.primary_key);
    println!("   Product table secondary index: {:?}", product_table.secondary_index);
    println!("   Product table secondary index type: {:?}", product_table.secondary_index_type);
    
    println!("\nDDL Index example completed successfully!");
}