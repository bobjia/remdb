extern crate alloc;

use core::ptr::NonNull;
use remdb::*;
use alloc::string::String;

// 定义内存缓冲区
static mut DB_MEMORY: [u8; 262144] = [0u8; 262144]; // 256KB内存，用于多表测试

// 定义用户表结构
// 手动定义表结构，避免使用有问题的 calculate_record_size 宏
static users: remdb::types::TableDef = remdb::types::TableDef {
    id: 0,
    name: "users",
    fields: &[
        remdb::types::FieldDef {
            name: "id",
            data_type: remdb::types::DataType::UInt32,
            size: 4,
            offset: 0,
            primary_key: true,
            not_null: true,
            unique: true,
            auto_increment: true,
            default_value: None,
        },
        remdb::types::FieldDef {
            name: "name",
            data_type: remdb::types::DataType::String,
            size: 32,
            offset: 4,
            primary_key: false,
            not_null: true,
            unique: false,
            auto_increment: false,
            default_value: None,
        },
        remdb::types::FieldDef {
            name: "email",
            data_type: remdb::types::DataType::String,
            size: 64,
            offset: 36,
            primary_key: false,
            not_null: true,
            unique: true,
            auto_increment: false,
            default_value: None,
        },
        remdb::types::FieldDef {
            name: "age",
            data_type: remdb::types::DataType::UInt8,
            size: 1,
            offset: 100,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
        },
        remdb::types::FieldDef {
            name: "active",
            data_type: remdb::types::DataType::Bool,
            size: 1,
            offset: 101,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
        },
        remdb::types::FieldDef {
            name: "created_at",
            data_type: remdb::types::DataType::Timestamp,
            size: 8,
            offset: 104,
            primary_key: false,
            not_null: true,
            unique: false,
            auto_increment: false,
            default_value: None,
        },
    ],
    primary_key: 0,
    secondary_index: Some(1),
    secondary_index_type: remdb::types::IndexType::SortedArray,
    record_size: 112, // 正确的记录大小：4 + 32 + 64 + 1 + 1 + 8 = 110字节（对齐到8字节是112字节）
    max_records: 100,
};

// 定义订单表结构
static orders: remdb::types::TableDef = remdb::types::TableDef {
    id: 1,
    name: "orders",
    fields: &[
        remdb::types::FieldDef {
            name: "id",
            data_type: remdb::types::DataType::UInt64,
            size: 8,
            offset: 0,
            primary_key: true,
            not_null: true,
            unique: true,
            auto_increment: true,
            default_value: None,
        },
        remdb::types::FieldDef {
            name: "user_id",
            data_type: remdb::types::DataType::UInt32,
            size: 4,
            offset: 8,
            primary_key: false,
            not_null: true,
            unique: false,
            auto_increment: false,
            default_value: None,
        },
        remdb::types::FieldDef {
            name: "product",
            data_type: remdb::types::DataType::String,
            size: 64,
            offset: 12,
            primary_key: false,
            not_null: true,
            unique: false,
            auto_increment: false,
            default_value: None,
        },
        remdb::types::FieldDef {
            name: "quantity",
            data_type: remdb::types::DataType::UInt32,
            size: 4,
            offset: 76,
            primary_key: false,
            not_null: true,
            unique: false,
            auto_increment: false,
            default_value: None,
        },
        remdb::types::FieldDef {
            name: "amount",
            data_type: remdb::types::DataType::Float64,
            size: 8,
            offset: 80,
            primary_key: false,
            not_null: true,
            unique: false,
            auto_increment: false,
            default_value: None,
        },
        remdb::types::FieldDef {
            name: "status",
            data_type: remdb::types::DataType::String,
            size: 16,
            offset: 88,
            primary_key: false,
            not_null: true,
            unique: false,
            auto_increment: false,
            default_value: None,
        },
        remdb::types::FieldDef {
            name: "created_at",
            data_type: remdb::types::DataType::Timestamp,
            size: 8,
            offset: 104,
            primary_key: false,
            not_null: true,
            unique: false,
            auto_increment: false,
            default_value: None,
        },
    ],
    primary_key: 0,
    secondary_index: Some(1),
    secondary_index_type: remdb::types::IndexType::SortedArray,
    record_size: 112, // 正确的记录大小：8 + 4 + 64 + 4 + 8 + 16 + 8 = 112字节
    max_records: 200,
};

// 定义产品表结构
static products: remdb::types::TableDef = remdb::types::TableDef {
    id: 2,
    name: "products",
    fields: &[
        remdb::types::FieldDef {
            name: "id",
            data_type: remdb::types::DataType::UInt32,
            size: 4,
            offset: 0,
            primary_key: true,
            not_null: true,
            unique: true,
            auto_increment: true,
            default_value: None,
        },
        remdb::types::FieldDef {
            name: "name",
            data_type: remdb::types::DataType::String,
            size: 64,
            offset: 4,
            primary_key: false,
            not_null: true,
            unique: true,
            auto_increment: false,
            default_value: None,
        },
        remdb::types::FieldDef {
            name: "description",
            data_type: remdb::types::DataType::String,
            size: 128,
            offset: 68,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
        },
        remdb::types::FieldDef {
            name: "price",
            data_type: remdb::types::DataType::Float64,
            size: 8,
            offset: 196,
            primary_key: false,
            not_null: true,
            unique: false,
            auto_increment: false,
            default_value: None,
        },
        remdb::types::FieldDef {
            name: "stock",
            data_type: remdb::types::DataType::UInt32,
            size: 4,
            offset: 204,
            primary_key: false,
            not_null: true,
            unique: false,
            auto_increment: false,
            default_value: None,
        },
        remdb::types::FieldDef {
            name: "active",
            data_type: remdb::types::DataType::Bool,
            size: 1,
            offset: 208,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
        },
    ],
    primary_key: 0,
    secondary_index: Some(1),
    secondary_index_type: remdb::types::IndexType::SortedArray,
    record_size: 216, // 正确的记录大小：4 + 64 + 128 + 8 + 4 + 1 = 209字节（对齐到8字节是216字节）
    max_records: 150,
};

// 定义数据库配置，包含多个表
remdb::database!(
    DB_CONFIG,
    tables: [users, orders, products]
);

fn main() {
    unsafe {
        // 使用生成的数据库配置静态变量
        let config = &DB_CONFIG;
        
        // 初始化内存分配器
        memory::allocator::init_global_allocator(
            DB_MEMORY.as_mut_ptr(),
            DB_MEMORY.len()
        );
        
        // 初始化平台抽象层
        #[cfg(feature = "posix")]
        platform::init_platform(platform::posix::get_posix_platform());
        #[cfg(not(feature = "posix"))]
        {
            // 在非posix平台上，使用一个简单的平台实现
            struct DummyPlatform;
            impl platform::Platform for DummyPlatform {
                fn get_timestamp(&self) -> u64 {
                    0
                }
                fn get_timestamp_us(&self) -> u64 {
                    0
                }
                fn spin_lock(&self, _lock: &mut u32) {
                }
                fn spin_unlock(&self, _lock: &mut u32) {
                }
                fn compiler_barrier(&self) {
                }
                fn full_memory_barrier(&self) {
                }
                fn memcpy(&self, dest: *mut u8, src: *const u8, size: usize) {
                    unsafe {
                        core::ptr::copy_nonoverlapping(src, dest, size);
                    }
                }
                fn memset(&self, dest: *mut u8, value: u8, size: usize) {
                    unsafe {
                        core::ptr::write_bytes(dest, value, size);
                    }
                }
                fn delay_ms(&self, _ms: u32) {
                }
                fn delay_us(&self, _us: u32) {
                }
                fn file_open(&self, _path: &str, _mode: platform::FileMode) -> platform::FileResult<platform::FileHandle> {
                    Err(())
                }
                fn file_close(&self, _handle: platform::FileHandle) -> platform::FileResult<()> {
                    Err(())
                }
                fn file_write(&self, _handle: platform::FileHandle, _buffer: *const u8, _size: usize) -> platform::FileResult<usize> {
                    Err(())
                }
                fn file_read(&self, _handle: platform::FileHandle, _buffer: *mut u8, _size: usize) -> platform::FileResult<usize> {
                    Err(())
                }
                fn file_seek(&self, _handle: platform::FileHandle, _offset: i64, _whence: platform::SeekWhence) -> platform::FileResult<u64> {
                    Err(())
                }
                fn file_remove(&self, _path: &str) -> platform::FileResult<()> {
                    Err(())
                }
                fn file_size(&self, _path: &str) -> platform::FileResult<usize> {
                    Err(())
                }
                fn crc32(&self, _data: *const u8, _size: usize) -> u32 {
                    0
                }
            }
            static DUMMY_PLATFORM: DummyPlatform = DummyPlatform;
            platform::init_platform(&DUMMY_PLATFORM);
        }
        
        // 初始化全局数据库
        let db = init_global_db(config).unwrap();
        
        // ---------- 产品表操作 ----------
        println!("=== 产品表操作 ===");
        
        // 使用insert_record插入产品
        let product_columns = &["id", "name", "description", "price", "stock", "active"];
        let product_values = &["1", "Laptop Pro", "High performance laptop", "999.99", "50", "true"];
        let product_affected_rows = db.insert_record("products", product_columns, product_values).unwrap();
        println!("插入产品成功，影响行数: {}", product_affected_rows);
        
        // ---------- 用户表操作 ----------
        println!("\n=== 用户表操作 ===");
        
        // 使用insert_record插入用户
        let user_columns = &["id", "name", "email", "age", "active", "created_at"];
        let user_values = &["1", "test_user", "test@example.com", "30", "true", "1234567890"];
        let user_affected_rows = db.insert_record("users", user_columns, user_values).unwrap();
        println!("插入用户成功，影响行数: {}", user_affected_rows);
        
        // ---------- 订单表操作 ----------
        println!("\n=== 订单表操作 ===");
        
        // 使用insert_record插入订单
        let order_columns = &["id", "user_id", "product", "quantity", "amount", "status", "created_at"];
        let order_values = &["1001", "1", "Laptop Pro", "1", "999.99", "pending", "1234567890"];
        let order_affected_rows = db.insert_record("orders", order_columns, order_values).unwrap();
        println!("插入订单成功，影响行数: {}", order_affected_rows);
        
        // ---------- 查询操作 ----------
        println!("\n=== 查询操作 ===");
        
        // 查询用户
        println!("\n查询用户:");
        let user_result = db.execute_query("users", &["id", "name"], None, None).unwrap();
        println!("{}", user_result.to_string());
        
        // 查询订单
        println!("\n查询订单:");
        let order_result = db.execute_query("orders", &["id", "user_id", "product"], None, None).unwrap();
        println!("{}", order_result.to_string());
        
        // 查询产品
        println!("\n查询产品:");
        let product_result = db.execute_query("products", &["id", "name", "price"], None, None).unwrap();
        println!("{}", product_result.to_string());
        
        // ---------- 多表关联示例 ----------
        println!("\n=== 多表关联示例 ===");
        
        // 使用execute_query查询用户和订单关系
        println!("\n用户订单关系:");
        let user_orders = db.execute_query("orders", &["user_id", "product", "amount"], Some("user_id = 1"), None).unwrap();
        println!("{}", user_orders.to_string());
        
        // ---------- 更新操作示例 ----------
        println!("\n=== 更新操作示例 ===");
        
        // 使用update_record更新产品价格
        let update_affected_rows = db.update_record("products", "price = 899.99", Some("id = 1")).unwrap();
        println!("更新产品价格成功，影响行数: {}", update_affected_rows);
        
        // 查询更新后的产品
        let updated_product = db.execute_query("products", &["id", "name", "price"], Some("id = 1"), None).unwrap();
        println!("更新后的产品信息:");
        println!("{}", updated_product.to_string());
        
        // ---------- 删除操作 ----------
        println!("\n=== 删除操作 ===");
        
        // 删除订单
        let order_delete_rows = db.delete_record("orders", Some("id = 1001")).unwrap();
        println!("删除订单成功，影响行数: {}", order_delete_rows);
        
        // 删除用户
        let user_delete_rows = db.delete_record("users", Some("id = 1")).unwrap();
        println!("删除用户成功，影响行数: {}", user_delete_rows);
        
        // 删除产品
        let product_delete_rows = db.delete_record("products", Some("id = 1")).unwrap();
        println!("删除产品成功，影响行数: {}", product_delete_rows);
        
        // 验证删除结果
        println!("\n验证删除结果:");
        let users_after_delete = db.execute_query("users", &["id"], None, None).unwrap();
        let orders_after_delete = db.execute_query("orders", &["id"], None, None).unwrap();
        let products_after_delete = db.execute_query("products", &["id"], None, None).unwrap();
        
        println!("用户表剩余记录数: {}", users_after_delete.rows.len());
        println!("订单表剩余记录数: {}", orders_after_delete.rows.len());
        println!("产品表剩余记录数: {}", products_after_delete.rows.len());
        
        println!("\nMultiple tables example completed successfully!");
    }
}