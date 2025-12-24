extern crate alloc;

use core::ptr::NonNull;
use remdb::*;

// 定义内存缓冲区
static mut DB_MEMORY: [u8; 65536] = [0u8; 65536];

// 定义表结构
remdb::table!(
    users,
    100, // 最大记录数
    primary_key: id,
    secondary_index: name,
    fields: {
        id: i32,
        name: str(32), // 32字节定长字符串
        age: i8,
        active: bool,
        created_at: u64
    }
);

// 定义数据库配置
remdb::database!(
    DB_CONFIG,
    tables: [users]
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
        platform::init_platform(platform::posix::get_posix_platform());
        
        // 计算所需内存大小
        let table_size = MemoryTable::calculate_memory_size(&config.tables[0]);
        let primary_index_size = PrimaryIndex::calculate_memory_size(
            &config.tables[0],
            128, // 哈希表大小
            100  // 最大索引项数量
        );
        let secondary_index_size = SecondaryIndex::calculate_memory_size(100);
        
        // 分配内存
        let table_ptr = memory::allocator::alloc(table_size).unwrap().as_ptr() as *mut u8;
        let status_ptr = memory::allocator::alloc(
            core::mem::size_of::<types::RecordHeader>() * config.tables[0].max_records
        ).unwrap().as_ptr() as *mut types::RecordHeader;
        
        let free_slots_ptr = memory::allocator::alloc(
            core::mem::size_of::<usize>() * config.tables[0].max_records
        ).unwrap().as_ptr() as *mut usize;
        
        let hash_table_ptr = memory::allocator::alloc(
            128 * core::mem::size_of::<Option<NonNull<index::PrimaryIndexItem>>>()
        ).unwrap().as_ptr() as *mut Option<NonNull<index::PrimaryIndexItem>>;
        
        let primary_index_items_ptr = memory::allocator::alloc(
            100 * core::mem::size_of::<index::PrimaryIndexItem>()
        ).unwrap().as_ptr() as *mut index::PrimaryIndexItem;
        
        let secondary_index_items_ptr = memory::allocator::alloc(
            100 * core::mem::size_of::<index::SecondaryIndexItem>()
        ).unwrap().as_ptr() as *mut index::SecondaryIndexItem;
        
        // 创建表和索引
        let mut table = MemoryTable::new(&config.tables[0], table_ptr, status_ptr, free_slots_ptr);
        let mut primary_index = PrimaryIndex::new(
            &config.tables[0],
            hash_table_ptr,
            primary_index_items_ptr,
            128,
            100
        );
        let mut secondary_index = SecondaryIndex::new(&config.tables[0], secondary_index_items_ptr, 100);
        
        // 初始化表和索引数组
        static mut TABLES: [Option<MemoryTable>; 1] = [None; 1];
        static mut PRIMARY_INDICES: [Option<PrimaryIndex>; 1] = [None; 1];
        static mut SECONDARY_INDICES: [Option<SecondaryIndex>; 1] = [None; 1];
        
        TABLES[0] = Some(table);
        PRIMARY_INDICES[0] = Some(primary_index);
        SECONDARY_INDICES[0] = Some(secondary_index);
        
        // 初始化全局数据库
        let db = init_global_db(
            config,
            &mut TABLES,
            &mut PRIMARY_INDICES,
            &mut SECONDARY_INDICES
        ).unwrap();
        
        // 创建测试记录
        let mut record_data = [0u8; 44]; // 计算记录大小：i32(4) + str(32) + i8(1) + bool(1) + u64(8) = 46字节（对齐到8字节）
        
        // 设置字段值
        let id: i32 = 1;
        let name = "test_user";
        let age: i8 = 30;
        let active = true;
        let created_at: u64 = 1234567890;
        
        // 手动填充记录数据（实际应用中应该使用更安全的方式）
        core::ptr::copy_nonoverlapping(
            &id as *const i32 as *const u8,
            record_data.as_mut_ptr(),
            4
        );
        
        core::ptr::copy_nonoverlapping(
            name.as_ptr(),
            record_data.as_mut_ptr().add(4),
            name.len()
        );
        
        core::ptr::write(record_data.as_mut_ptr().add(36) as *mut i8, age);
        core::ptr::write(record_data.as_mut_ptr().add(37) as *mut bool, active);
        core::ptr::copy_nonoverlapping(
            &created_at as *const u64 as *const u8,
            record_data.as_mut_ptr().add(40),
            8
        );
        
        // 插入记录
        let table_mut = db.get_table_mut(0).unwrap();
        let record_id = table_mut.insert(record_data.as_ptr()).unwrap();
        
        println!("Inserted record with ID: {}", record_id);
        
        // 获取记录
        let mut result_data = [0u8; 44];
        table_mut.get_by_id(record_id, result_data.as_mut_ptr()).unwrap();
        
        // 读取字段值（简化示例）
        let result_id = core::ptr::read(result_data.as_ptr() as *const i32);
        let result_name = core::str::from_utf8(&result_data[4..36]).unwrap().trim_end_matches(char::from(0));
        let result_age = core::ptr::read(result_data.as_ptr().add(36) as *const i8);
        let result_active = core::ptr::read(result_data.as_ptr().add(37) as *const bool);
        let result_created_at = core::ptr::read(result_data.as_ptr().add(40) as *const u64);
        
        println!("Retrieved record: ID={}, Name={}, Age={}, Active={}, CreatedAt={}",
                 result_id, result_name, result_age, result_active, result_created_at);
        
        // 删除记录
        table_mut.delete(record_id).unwrap();
        println!("Deleted record with ID: {}", record_id);
        
        // 测试事务
        // 创建事务缓冲区（未初始化）
        let mut tx_buffer: transaction::Transaction = core::mem::MaybeUninit::uninit().assume_init();
        
        let mut log_buffer = [transaction::LogItem {
            op_type: transaction::LogOperation::Insert,
            table_id: 0,
            record_id: 0,
            data_size: 0,
            checksum: 0,
            timestamp: 0,
            tx_id: 0,
            old_data: [0u8; 512],
            new_data: [0u8; 512],
        }; 10];
        
        let tx = transaction::begin(
            transaction::TransactionType::ReadWrite,
            transaction::IsolationLevel::ReadCommitted,
            &mut tx_buffer,
            log_buffer.as_mut_ptr(),
            10
        ).unwrap();
        
        println!("Started transaction with ID: {}", tx.as_ref().id);
        
        // 在事务中插入记录
        let tx_record_id = table_mut.insert(record_data.as_ptr()).unwrap();
        println!("Inserted record in transaction with ID: {}", tx_record_id);
        
        // 提交事务
        transaction::commit().unwrap();
        println!("Committed transaction");
        
        // 清理
        table_mut.delete(tx_record_id).unwrap();
        
        println!("Basic usage example completed successfully!");
    }
}
