extern crate alloc;

use core::ptr::NonNull;
use remdb::*;

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
        
        // 准备产品数据
        let mut product1_data = [0u8; 216]; // 计算记录大小：i32(4) + str(64) + str(128) + f64(8) + i32(4) + bool(1) = 209字节（对齐到8字节）
        let product1_id: i32 = 1;
        let product1_name = "Laptop Pro";
        let product1_desc = "High performance laptop";
        let product1_price: f64 = 999.99;
        let product1_stock: i32 = 50;
        let product1_active = true;
        
        // 设置产品1字段值
        core::ptr::copy_nonoverlapping(&product1_id as *const i32 as *const u8, product1_data.as_mut_ptr(), 4);
        core::ptr::copy_nonoverlapping(product1_name.as_ptr(), product1_data.as_mut_ptr().add(4), product1_name.len());
        core::ptr::copy_nonoverlapping(product1_desc.as_ptr(), product1_data.as_mut_ptr().add(68), product1_desc.len());
        core::ptr::copy_nonoverlapping(&product1_price as *const f64 as *const u8, product1_data.as_mut_ptr().add(196), 8);
        core::ptr::copy_nonoverlapping(&product1_stock as *const i32 as *const u8, product1_data.as_mut_ptr().add(204), 4);
        core::ptr::write(product1_data.as_mut_ptr().add(208) as *mut bool, product1_active);
        
        let mut product1_record_id = 0;
        {
            let products_table_mut = db.get_table_mut(2).unwrap();
            product1_record_id = products_table_mut.insert(product1_data.as_ptr()).unwrap();
            println!("Inserted product: ID={}, RecordID={}", product1_id, product1_record_id);
        }
        
        // ---------- 用户表操作 ----------
        println!("\n=== 用户表操作 ===");
        
        // 准备用户数据
        let mut user_data = [0u8; 112]; // 计算记录大小：i32(4) + str(32) + str(64) + i8(1) + bool(1) + u64(8) = 110字节（对齐到8字节）
        let user_id: i32 = 1;
        let user_name = "test_user";
        let user_email = "test@example.com";
        let user_age: i8 = 30;
        let user_active = true;
        let user_created_at: u64 = 1234567890;
        
        // 设置用户字段值
        core::ptr::copy_nonoverlapping(&user_id as *const i32 as *const u8, user_data.as_mut_ptr(), 4);
        core::ptr::copy_nonoverlapping(user_name.as_ptr(), user_data.as_mut_ptr().add(4), user_name.len());
        core::ptr::copy_nonoverlapping(user_email.as_ptr(), user_data.as_mut_ptr().add(36), user_email.len());
        core::ptr::write(user_data.as_ptr().add(100) as *mut i8, user_age);
        core::ptr::write(user_data.as_ptr().add(101) as *mut bool, user_active);
        core::ptr::copy_nonoverlapping(&user_created_at as *const u64 as *const u8, user_data.as_mut_ptr().add(104), 8);
        
        let mut user_record_id = 0;
        {
            let users_table_mut = db.get_table_mut(0).unwrap();
            user_record_id = users_table_mut.insert(user_data.as_ptr()).unwrap();
            println!("Inserted user: ID={}, Name={}, RecordID={}", user_id, user_name, user_record_id);
        }
        
        // ---------- 订单表操作 ----------
        println!("\n=== 订单表操作 ===");
        
        // 准备订单数据
        let mut order_data = [0u8; 160]; // 计算记录大小：i64(8) + i32(4) + str(64) + i32(4) + f64(8) + str(16) + u64(8) = 112字节（对齐到8字节）
        let order_id: i64 = 1001;
        let order_user_id: i32 = 1; // 关联用户ID
        let order_product = "Laptop Pro";
        let order_quantity: i32 = 1;
        let order_amount: f64 = 999.99;
        let order_status = "pending";
        let order_created_at: u64 = 1234567890;
        
        // 设置订单字段值
        core::ptr::copy_nonoverlapping(&order_id as *const i64 as *const u8, order_data.as_mut_ptr(), 8);
        core::ptr::copy_nonoverlapping(&order_user_id as *const i32 as *const u8, order_data.as_mut_ptr().add(8), 4);
        core::ptr::copy_nonoverlapping(order_product.as_ptr(), order_data.as_mut_ptr().add(12), order_product.len());
        core::ptr::copy_nonoverlapping(&order_quantity as *const i32 as *const u8, order_data.as_mut_ptr().add(76), 4);
        core::ptr::copy_nonoverlapping(&order_amount as *const f64 as *const u8, order_data.as_mut_ptr().add(80), 8);
        core::ptr::copy_nonoverlapping(order_status.as_ptr(), order_data.as_mut_ptr().add(88), order_status.len());
        core::ptr::copy_nonoverlapping(&order_created_at as *const u64 as *const u8, order_data.as_mut_ptr().add(104), 8);
        
        let mut order_record_id = 0;
        {
            let orders_table_mut = db.get_table_mut(1).unwrap();
            order_record_id = orders_table_mut.insert(order_data.as_ptr()).unwrap();
            println!("Inserted order: ID={}, UserID={}, Product={}, RecordID={}", 
                     order_id, order_user_id, order_product, order_record_id);
        }
        
        // ---------- 查询操作 ----------
        println!("\n=== 查询操作 ===");
        
        // 查询用户
        let mut retrieved_user_id = 0;
        let mut retrieved_user_name = String::new();
        {
            let mut retrieved_user = [0u8; 112];
            let users_table_mut = db.get_table_mut(0).unwrap();
            users_table_mut.get_by_id(user_record_id, retrieved_user.as_mut_ptr()).unwrap();
            retrieved_user_id = core::ptr::read(retrieved_user.as_ptr() as *const i32);
            retrieved_user_name = core::str::from_utf8(&retrieved_user[4..36]).unwrap().trim_end_matches(char::from(0)).to_string();
            println!("Retrieved user: ID={}, Name={}", retrieved_user_id, retrieved_user_name);
        }
        
        // 查询订单
        let mut retrieved_order_id = 0;
        let mut retrieved_order_user_id = 0;
        {
            let mut retrieved_order = [0u8; 160];
            let orders_table_mut = db.get_table_mut(1).unwrap();
            orders_table_mut.get_by_id(order_record_id, retrieved_order.as_mut_ptr()).unwrap();
            retrieved_order_id = core::ptr::read(retrieved_order.as_ptr() as *const i64);
            retrieved_order_user_id = core::ptr::read(retrieved_order.as_ptr().add(8) as *const i32);
            println!("Retrieved order: ID={}, UserID={}", retrieved_order_id, retrieved_order_user_id);
        }
        
        // ---------- 多表关联示例 ----------
        println!("\n=== 多表关联示例 ===");
        println!("User {} (ID: {}) placed order {} for product {}", 
                 retrieved_user_name, retrieved_user_id, retrieved_order_id, order_product);
        
        // ---------- 删除操作 ----------
        println!("\n=== 删除操作 ===");
        
        // 删除订单
        {
            let orders_table_mut = db.get_table_mut(1).unwrap();
            orders_table_mut.delete(order_record_id).unwrap();
            println!("Deleted order: RecordID={}", order_record_id);
        }
        
        // 删除用户
        {
            let users_table_mut = db.get_table_mut(0).unwrap();
            users_table_mut.delete(user_record_id).unwrap();
            println!("Deleted user: RecordID={}", user_record_id);
        }
        
        // 删除产品
        {
            let products_table_mut = db.get_table_mut(2).unwrap();
            products_table_mut.delete(product1_record_id).unwrap();
            println!("Deleted product: RecordID={}", product1_record_id);
        }
        
        println!("\nMultiple tables example completed successfully!");
    }
}