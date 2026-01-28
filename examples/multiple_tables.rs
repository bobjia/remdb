extern crate alloc;

use remdb::*;

// 定义内存缓冲区
static mut DB_MEMORY: [u8; 2097152] = [0u8; 2097152]; // 2MB内存，用于多表测试

// 定义用户表结构
// 手动定义表结构，避免使用有问题的 calculate_record_size 宏
static USERS: std::sync::LazyLock<remdb::types::TableDef> = std::sync::LazyLock::new(|| remdb::types::TableDef {
    id: 0,
    name: "users".to_string(),
    fields: vec![
        remdb::types::FieldDef {
            name: "id".to_string(),
            data_type: remdb::types::DataType::UInt32,
            size: 4,
            offset: 0,
            primary_key: true,
            not_null: true,
            unique: true,
            auto_increment: true,
            default_value: None,
            vector_metadata: None,
        },
        remdb::types::FieldDef {
            name: "name".to_string(),
            data_type: remdb::types::DataType::String,
            size: 32,
            offset: 4,
            primary_key: false,
            not_null: true,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
        },
        remdb::types::FieldDef {
            name: "email".to_string(),
            data_type: remdb::types::DataType::String,
            size: 64,
            offset: 36,
            primary_key: false,
            not_null: true,
            unique: true,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
        },
        remdb::types::FieldDef {
            name: "age".to_string(),
            data_type: remdb::types::DataType::UInt8,
            size: 1,
            offset: 100,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
        },
        remdb::types::FieldDef {
            name: "active".to_string(),
            data_type: remdb::types::DataType::Bool,
            size: 1,
            offset: 101,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
        },
        remdb::types::FieldDef {
            name: "created_at".to_string(),
            data_type: remdb::types::DataType::Timestamp,
            size: 8,
            offset: 102,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
        },
    ],
    primary_key: vec![0],
    secondary_index: Some(vec![1]),
    secondary_index_type: remdb::types::IndexType::SortedArray,
    record_size: 112, // 正确的记录大小：4 + 32 + 64 + 1 + 1 + 8 = 110字节（对齐到8字节是112字节）
    max_records: 100,
    created_at: 0,
    updated_at: 0,
    version: 1,
});

// 定义订单表结构
static ORDERS: std::sync::LazyLock<remdb::types::TableDef> = std::sync::LazyLock::new(|| remdb::types::TableDef {
    id: 1,
    name: "orders".to_string(),
    fields: vec![
        remdb::types::FieldDef {
            name: "id".to_string(),
            data_type: remdb::types::DataType::UInt64,
            size: 8,
            offset: 0,
            primary_key: true,
            not_null: true,
            unique: true,
            auto_increment: true,
            default_value: None,
            vector_metadata: None,
        },
        remdb::types::FieldDef {
            name: "user_id".to_string(),
            data_type: remdb::types::DataType::UInt32,
            size: 4,
            offset: 8,
            primary_key: false,
            not_null: true,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
        },
        remdb::types::FieldDef {
            name: "product".to_string(),
            data_type: remdb::types::DataType::String,
            size: 64,
            offset: 12,
            primary_key: false,
            not_null: true,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
        },
        remdb::types::FieldDef {
            name: "quantity".to_string(),
            data_type: remdb::types::DataType::UInt32,
            size: 4,
            offset: 76,
            primary_key: false,
            not_null: true,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
        },
        remdb::types::FieldDef {
            name: "amount".to_string(),
            data_type: remdb::types::DataType::Float64,
            size: 8,
            offset: 80,
            primary_key: false,
            not_null: true,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
        },
        remdb::types::FieldDef {
            name: "status".to_string(),
            data_type: remdb::types::DataType::String,
            size: 16,
            offset: 88,
            primary_key: false,
            not_null: true,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
        },
        remdb::types::FieldDef {
            name: "created_at".to_string(),
            data_type: remdb::types::DataType::Timestamp,
            size: 8,
            offset: 104,
            primary_key: false,
            not_null: true,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
        },
    ],
    primary_key: vec![0],
    secondary_index: Some(vec![1]),
    secondary_index_type: remdb::types::IndexType::SortedArray,
    record_size: 112, // 正确的记录大小：8 + 4 + 64 + 4 + 8 + 16 + 8 = 112字节
    max_records: 200,
    created_at: 0,
    updated_at: 0,
    version: 1,
});

// 定义产品表结构
static PRODUCTS: std::sync::LazyLock<remdb::types::TableDef> = std::sync::LazyLock::new(|| remdb::types::TableDef {
    id: 2,
    name: "products".to_string(),
    fields: vec![
        remdb::types::FieldDef {
            name: "id".to_string(),
            data_type: remdb::types::DataType::UInt32,
            size: 4,
            offset: 0,
            primary_key: true,
            not_null: true,
            unique: true,
            auto_increment: true,
            default_value: None,
            vector_metadata: None,
        },
        remdb::types::FieldDef {
            name: "name".to_string(),
            data_type: remdb::types::DataType::String,
            size: 64,
            offset: 4,
            primary_key: false,
            not_null: true,
            unique: true,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
        },
        remdb::types::FieldDef {
            name: "description".to_string(),
            data_type: remdb::types::DataType::String,
            size: 128,
            offset: 68,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
        },
        remdb::types::FieldDef {
            name: "price".to_string(),
            data_type: remdb::types::DataType::Float64,
            size: 8,
            offset: 196,
            primary_key: false,
            not_null: true,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
        },
        remdb::types::FieldDef {
            name: "stock".to_string(),
            data_type: remdb::types::DataType::UInt32,
            size: 4,
            offset: 204,
            primary_key: false,
            not_null: true,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
        },
        remdb::types::FieldDef {
            name: "active".to_string(),
            data_type: remdb::types::DataType::Bool,
            size: 1,
            offset: 208,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
        },
    ],
    primary_key: vec![0],
    secondary_index: Some(vec![1]),
    secondary_index_type: remdb::types::IndexType::SortedArray,
    record_size: 216, // 正确的记录大小：4 + 64 + 128 + 8 + 4 + 1 = 209字节（对齐到8字节是216字节）
    max_records: 150,
    created_at: 0,
    updated_at: 0,
    version: 1,
});

// 定义数据库配置，包含多个表
remdb::database!(
    DB_CONFIG,
    tables: [USERS, ORDERS, PRODUCTS]
);

fn main() {
    unsafe {
        // 使用生成的数据库配置静态变量
        let config = &DB_CONFIG;

        // 初始化内存分配器
        let _ = memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len());

        // 平台会在RemDb::init()中自动初始化，无需手动初始化

        // 初始化全局数据库
        let db = init_global_db(config).unwrap();

        // ---------- 产品表操作 ----------
        println!("=== 产品表操作 ===");
        
        // 插入产品数据
        db.sql_query("INSERT INTO products (name, description, price, stock, active) VALUES ('Laptop', 'High-performance laptop', 999.99, 50, true)").unwrap();
        db.sql_query("INSERT INTO products (name, description, price, stock, active) VALUES ('Smartphone', 'Latest smartphone', 699.99, 100, true)").unwrap();
        db.sql_query("INSERT INTO products (name, description, price, stock, active) VALUES ('Tablet', 'Tablet computer', 399.99, 75, true)").unwrap();
        db.sql_query("INSERT INTO products (name, description, price, stock, active) VALUES ('Headphones', 'Wireless headphones', 199.99, 200, true)").unwrap();
        db.sql_query("INSERT INTO products (name, description, price, stock, active) VALUES ('Monitor', '4K monitor', 499.99, 30, true)").unwrap();
        
        println!("插入5个产品完成");

        // ---------- 用户表操作 ----------
        println!("\n=== 用户表操作 ===");
        
        // 插入用户数据
        db.sql_query("INSERT INTO users (name, email, age, active, created_at) VALUES ('Zhang San', 'zhangsan@example.com', 25, true, 1620000000000)").unwrap();
        db.sql_query("INSERT INTO users (name, email, age, active, created_at) VALUES ('Li Si', 'lisi@example.com', 30, true, 1620000001000)").unwrap();
        db.sql_query("INSERT INTO users (name, email, age, active, created_at) VALUES ('Wang Wu', 'wangwu@example.com', 28, true, 1620000002000)").unwrap();
        db.sql_query("INSERT INTO users (name, email, age, active, created_at) VALUES ('Zhao Liu', 'zhaoliu@example.com', 35, false, 1620000003000)").unwrap();
        db.sql_query("INSERT INTO users (name, email, age, active, created_at) VALUES ('Sun Qi', 'sunqi@example.com', 22, true, 1620000004000)").unwrap();
        
        println!("插入5个用户完成");

        // ---------- 订单表操作 ----------
        println!("\n=== 订单表操作 ===");
        
        // 插入订单数据（关联用户和产品）
        db.sql_query("INSERT INTO orders (user_id, product, quantity, amount, status, created_at) VALUES (1, 'Laptop', 1, 999.99, 'completed', 1620000005000)").unwrap();
        db.sql_query("INSERT INTO orders (user_id, product, quantity, amount, status, created_at) VALUES (1, 'Smartphone', 2, 1399.98, 'pending', 1620000006000)").unwrap();
        db.sql_query("INSERT INTO orders (user_id, product, quantity, amount, status, created_at) VALUES (2, 'Tablet', 1, 399.99, 'completed', 1620000007000)").unwrap();
        db.sql_query("INSERT INTO orders (user_id, product, quantity, amount, status, created_at) VALUES (3, 'Headphones', 3, 599.97, 'shipped', 1620000008000)").unwrap();
        db.sql_query("INSERT INTO orders (user_id, product, quantity, amount, status, created_at) VALUES (4, 'Monitor', 2, 999.98, 'pending', 1620000009000)").unwrap();
        db.sql_query("INSERT INTO orders (user_id, product, quantity, amount, status, created_at) VALUES (5, 'Laptop', 1, 999.99, 'completed', 1620000010000)").unwrap();
        
        println!("插入6个订单完成");

        // ---------- 查询操作 ----------
        println!("\n=== 查询操作 ===");
        
        // 查询所有产品
        println!("\n1. 所有产品:");
        let products_result = db.sql_query("SELECT * FROM products").unwrap();
        println!("{}", products_result.to_string());
        
        // 查询所有用户
        println!("\n2. 所有用户:");
        let users_result = db.sql_query("SELECT * FROM users").unwrap();
        println!("{}", users_result.to_string());
        
        // 查询所有订单
        println!("\n3. 所有订单:");
        let orders_result = db.sql_query("SELECT * FROM orders").unwrap();
        println!("{}", orders_result.to_string());
        
        // 查询活跃用户
        println!("\n4. 活跃用户:");
        let active_users = db.sql_query("SELECT id, name, email, age FROM users WHERE active = true").unwrap();
        println!("{}", active_users.to_string());

        // ---------- 多表关联示例 ----------
        println!("\n=== 多表关联示例 ===");
        
        // 关联查询：订单详情，包含用户姓名和产品信息
        println!("\n1. 订单详情（关联用户和产品）:");
        let order_details = db.sql_query("
            SELECT o.id, u.name as user_name, o.product, o.quantity, o.amount, o.status, p.price as unit_price
            FROM orders o
            JOIN users u ON o.user_id = u.id
            JOIN products p ON o.product = p.name
        ").unwrap();
        println!("{}", order_details.to_string());
        
        // 统计每个用户的订单总金额
        println!("\n2. 用户订单统计:");
        let user_stats = db.sql_query("
            SELECT u.id, u.name, COUNT(o.id) as order_count, SUM(o.amount) as total_amount
            FROM users u
            LEFT JOIN orders o ON u.id = o.user_id
            GROUP BY u.id, u.name
            ORDER BY total_amount DESC
        ").unwrap();
        println!("{}", user_stats.to_string());
        
        // 统计每个产品的销售情况
        println!("\n3. 产品销售统计:");
        let product_stats = db.sql_query("
            SELECT p.name, SUM(o.quantity) as total_quantity, SUM(o.amount) as total_revenue
            FROM products p
            LEFT JOIN orders o ON p.name = o.product
            GROUP BY p.name
            ORDER BY total_revenue DESC
        ").unwrap();
        println!("{}", product_stats.to_string());

        // ---------- 更新操作示例 ----------
        println!("\n=== 更新操作示例 ===");
        
        // 更新产品库存（售出后减少库存）
        println!("\n1. 更新产品库存:");
        let update_result = db.sql_query("UPDATE products SET stock = stock - 1 WHERE name = 'Laptop'").unwrap();
        println!("更新结果: {}", update_result.to_string());
        
        // 更新用户状态
        println!("\n2. 更新用户状态:");
        let update_result = db.sql_query("UPDATE users SET active = false WHERE age > 30").unwrap();
        println!("更新结果: {}", update_result.to_string());
        
        // 更新订单状态
        println!("\n3. 更新订单状态:");
        let update_result = db.sql_query("UPDATE orders SET status = 'completed' WHERE status = 'pending'").unwrap();
        println!("更新结果: {}", update_result.to_string());
        
        // 验证更新结果
        println!("\n4. 验证更新结果:");
        let updated_products = db.sql_query("SELECT name, stock FROM products WHERE name = 'Laptop'").unwrap();
        println!("Laptop库存: {}", updated_products.to_string());
        
        let inactive_users = db.sql_query("SELECT id, name, age, active FROM users WHERE active = false").unwrap();
        println!("非活跃用户: {}", inactive_users.to_string());
        
        let completed_orders = db.sql_query("SELECT id, product, status FROM orders WHERE status = 'completed'").unwrap();
        println!("已完成订单: {}", completed_orders.to_string());

        // ---------- 删除操作 ----------
        println!("\n=== 删除操作 ===");
        
        // 删除特定订单
        println!("\n1. 删除特定订单:");
        let delete_result = db.sql_query("DELETE FROM orders WHERE amount < 500").unwrap();
        println!("删除结果: {}", delete_result.to_string());
        
        // 删除非活跃用户
        println!("\n2. 删除非活跃用户:");
        let delete_result = db.sql_query("DELETE FROM users WHERE active = false").unwrap();
        println!("删除结果: {}", delete_result.to_string());
        
        // 删除库存为0的产品（示例）
        println!("\n3. 删除库存为0的产品（示例）:");
        let delete_result = db.sql_query("DELETE FROM products WHERE stock <= 0").unwrap();
        println!("删除结果: {}", delete_result.to_string());

        // 验证删除结果
        println!("\n验证删除结果:");
        let remaining_users = db.sql_query("SELECT COUNT(*) as user_count FROM users").unwrap();
        let remaining_orders = db.sql_query("SELECT COUNT(*) as order_count FROM orders").unwrap();
        let remaining_products = db.sql_query("SELECT COUNT(*) as product_count FROM products").unwrap();
        
        println!("用户表剩余记录数: {}", remaining_users.to_string());
        println!("订单表剩余记录数: {}", remaining_orders.to_string());
        println!("产品表剩余记录数: {}", remaining_products.to_string());

        println!("\nMultiple tables example completed successfully!");
    }
}
