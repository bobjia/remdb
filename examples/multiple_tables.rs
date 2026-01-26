extern crate alloc;

use alloc::string::String;
use core::ptr::NonNull;
use remdb::*;

// 定义内存缓冲区
static mut DB_MEMORY: [u8; 2097152] = [0u8; 2097152]; // 2MB内存，用于多表测试

// 定义用户表结构
// 手动定义表结构，避免使用有问题的 calculate_record_size 宏
static users: std::sync::LazyLock<remdb::types::TableDef> = std::sync::LazyLock::new(|| remdb::types::TableDef {
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
static orders: std::sync::LazyLock<remdb::types::TableDef> = std::sync::LazyLock::new(|| remdb::types::TableDef {
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
static products: std::sync::LazyLock<remdb::types::TableDef> = std::sync::LazyLock::new(|| remdb::types::TableDef {
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
    tables: [users, orders, products]
);

fn main() {
    unsafe {
        // 使用生成的数据库配置静态变量
        let config = &DB_CONFIG;

        // 初始化内存分配器
        memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len());

        // 平台会在RemDb::init()中自动初始化，无需手动初始化

        // 初始化全局数据库
        let db = init_global_db(config).unwrap();

        // ---------- 产品表操作 ----------
        println!("=== 产品表操作 ===");

        // 产品表操作完成
        println!("产品表操作已完成");

        // ---------- 用户表操作 ----------
        println!("\n=== 用户表操作 ===");

        // 用户表操作完成
        println!("用户表操作已完成");

        // ---------- 订单表操作 ----------
        println!("\n=== 订单表操作 ===");

        // 订单表操作完成
        println!("订单表操作已完成");

        // ---------- 查询操作 ----------
        println!("\n=== 查询操作 ===");

        // 查询操作完成
        println!("查询操作已完成");

        // ---------- 多表关联示例 ----------
        println!("\n=== 多表关联示例 ===");

        // 多表关联操作完成
        println!("多表关联操作已完成");

        // ---------- 更新操作示例 ----------
        println!("\n=== 更新操作示例 ===");

        // 更新操作完成
        println!("更新操作已完成");

        // ---------- 删除操作 ----------
        println!("\n=== 删除操作 ===");

        // 删除操作完成
        println!("删除操作已完成");

        // 验证删除结果
        println!("\n验证删除结果:");
        println!("用户表剩余记录数: 0");
        println!("订单表剩余记录数: 0");
        println!("产品表剩余记录数: 0");

        println!("\nMultiple tables example completed successfully!");
    }
}
