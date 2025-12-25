use remdb::{database, table, init_global_db, Result}; use remdb::types::RecordHeader; use std::io::Read;

// 定义测试表
table!(
    TEST_TABLE,
    100, // 最大记录数
    primary_key: id,
    secondary_index: name,
    fields: {
        id: u64,
        name: str(20),
        value: u32
    }
);

// 定义数据库
database!(
    TEST_DB,
    tables: [TEST_TABLE]
);

// 手动计算表所需的总内存大小
// 记录大小：id(8字节) + name(20字节) + value(4字节) = 32字节
const RECORD_SIZE: usize = 8 + 20 + 4;
const TABLE_DATA_SIZE: usize = RECORD_SIZE * TEST_TABLE.max_records;
const STATUS_ARRAY_SIZE: usize = core::mem::size_of::<RecordHeader>() * TEST_TABLE.max_records;
const FREE_SLOTS_SIZE: usize = core::mem::size_of::<usize>() * TEST_TABLE.max_records;
const TABLE_MEM_SIZE: usize = TABLE_DATA_SIZE + STATUS_ARRAY_SIZE + FREE_SLOTS_SIZE;

// 静态变量，具有'static生命周期
static mut TABLE_MEM: [u8; TABLE_MEM_SIZE] = [0; TABLE_MEM_SIZE];

// 静态表数组，具有'static生命周期
static mut TABLES: [Option<remdb::MemoryTable>; 8] = [const { None }; 8];
static mut PRIMARY_INDICES: [Option<remdb::PrimaryIndex>; 8] = [const { None }; 8];
static mut SECONDARY_INDICES: [Option<remdb::AnySecondaryIndex>; 8] = [const { None }; 8];

fn main() -> Result<()> {
    unsafe {
        // 初始化平台抽象层
        #[cfg(feature = "posix")]
        remdb::platform::init_platform(remdb::platform::posix::get_posix_platform());
        
        // 初始化表
        let table_ptr = TABLE_MEM.as_mut_ptr();
        let status_ptr = table_ptr.add(TABLE_DATA_SIZE);
        let free_slots_ptr = status_ptr.add(STATUS_ARRAY_SIZE);
        
        // 初始化表，MemoryTable::new返回Option<MemoryTable>
        TABLES[0] = remdb::MemoryTable::new(
            &TEST_TABLE,
            table_ptr,
            status_ptr as *mut RecordHeader,
            free_slots_ptr as *mut usize
        );
        
        // 初始化数据库
        let db = init_global_db(
            &TEST_DB,
            &mut TABLES,
            &mut PRIMARY_INDICES,
            &mut SECONDARY_INDICES
        )?;
        
        // 插入测试数据
        println!("插入测试数据...");
        for i in 0..10 {
            // 创建对齐的记录数据
            #[repr(align(8))]
            struct AlignedRecord([u8; 32]); // 手动指定32字节大小（id:8 + name:20 + value:4）
            let mut record = AlignedRecord([0; 32]);
            
            // 获取表引用
            let table = db.get_table_mut(0)?;
            
            // 设置字段值
            let value = remdb::Value { u64: i as u64 };
            table.set_field(record.0.as_mut_ptr(), 0, &value)?
            
            let name = format!("item_{}", i);
            let name_value = remdb::Value { string: { 
                let mut s = [0u8; 64];
                // 填充name，剩余空间用0填充
                for (i, c) in name.as_bytes().iter().enumerate() {
                    if i < s.len() {
                        s[i] = *c;
                    } else {
                        break;
                    }
                }
                s
            } };
            table.set_field(record.0.as_mut_ptr(), 1, &name_value)?;
            
            let value_value = remdb::Value { u32: (i * 100) as u32 };
            table.set_field(record.0.as_mut_ptr(), 2, &value_value)?
            
            // 插入记录
            let record_id = table.insert(record.0.as_ptr())?;
            println!("插入记录ID: {}", record_id);
        }
        
        // 保存快照
        println!("保存快照到 snapshot.bin...");
        db.save_snapshot("snapshot.bin")?;
        println!("快照保存成功");
        
        // 修改数据
        println!("修改数据...");
        {
            let table = db.get_table_mut(0)?;
            
            // 创建对齐的记录数据
            #[repr(align(8))]
            struct AlignedRecord([u8; 32]); // 手动指定32字节大小（id:8 + name:20 + value:4）
            let mut record = AlignedRecord([0; 32]);
            
            // 获取记录
            table.get_by_id(0, record.0.as_mut_ptr())?;
            
            // 修改值字段
            let value_value = remdb::Value { u32: 999 };
            table.set_field(record.0.as_mut_ptr(), 2, &value_value)?
            
            // 重新插入（模拟更新）
            table.delete(0)?;
            table.insert(record.0.as_ptr())?;
        }
        
        // 恢复快照
        println!("恢复快照...");
        db.restore_snapshot("snapshot.bin")?;
        println!("快照恢复成功");
        
        // 验证数据 - 所有记录的value应该恢复到保存快照前的状态
        println!("验证数据...");
        let table = db.get_table(0)?;
        
        // 遍历所有记录，检查已使用的记录
        let mut used_records = 0;
        let mut total_value = 0;
        
        for i in 0..table.max_records() {
            let status_ptr = table.get_status_ptr(i);
            if (*status_ptr).status == remdb::types::RecordStatus::Used {
                // 创建对齐的记录数据
                #[repr(align(8))]
                struct AlignedRecord([u8; 32]); // 手动指定32字节大小（id:8 + name:20 + value:4）
                let mut record = AlignedRecord([0; 32]);
                
                let record_ptr = table.get_record_ptr(i);
                remdb::platform::memcpy(record.0.as_mut_ptr(), record_ptr, 32);
                
                // 获取字段值
                let id_value = table.get_field(record.0.as_ptr(), 0)?;
                let name_value = table.get_field(record.0.as_ptr(), 1)?;
                let value_value = table.get_field(record.0.as_ptr(), 2)?;
                
                // 提取值
                let id = id_value.u64;
                let name_bytes = name_value.string.as_slice();
                let name_len = name_bytes.iter().position(|&c| c == 0).unwrap_or(name_bytes.len());
                let name = std::str::from_utf8(&name_bytes[..name_len]).unwrap_or("invalid_utf8");
                let value = value_value.u32;
                
                println!("记录索引 {}: id={}, name={:?}, value={}", i, id, name, value);
                
                // 累加value，用于验证总和
                total_value += value;
                used_records += 1;
            }
        }
        
        // 验证已使用的记录数是否为10
        assert_eq!(used_records, 10);
        
        // 验证value总和是否为4500（0+100+200+...+900=4500）
        assert_eq!(total_value, 4500);
        
        // 增量快照测试
        println!("\n=== 增量快照测试 ===");
        
        // 修改部分数据
        println!("修改部分数据...");
        {
            let table = db.get_table_mut(0)?;
            
            // 修改第5条记录
            #[repr(align(8))]
            struct AlignedRecord([u8; 32]);
            let mut record = AlignedRecord([0; 32]);
            
            table.get_by_id(5, record.0.as_mut_ptr())?;
            let value_value = remdb::Value { u32: 5555 };
            table.set_field(record.0.as_mut_ptr(), 2, &value_value)?
            
            table.delete(5)?;
            table.insert(record.0.as_ptr())?;
            
            // 新增一条记录
            let mut new_record = AlignedRecord([0; 32]);
            let id_value = remdb::Value { u64: 10 };
            table.set_field(new_record.0.as_mut_ptr(), 0, &id_value)?
            
            let name = "item_10";
            let name_value = remdb::Value { string: {
                let mut s = [0u8; 64];
                for (i, c) in name.as_bytes().iter().enumerate() {
                    if i < s.len() {
                        s[i] = *c;
                    } else {
                        break;
                    }
                }
                s
            } };
            table.set_field(new_record.0.as_mut_ptr(), 1, &name_value)?;
            
            let value_value = remdb::Value { u32: 1000 };
            table.set_field(new_record.0.as_mut_ptr(), 2, &value_value)?
            
            table.insert(new_record.0.as_ptr())?;
        }
        
        // 保存增量快照
        println!("保存增量快照...");
        db.save_incremental_snapshot("incremental_snapshot.bin")?;
        println!("增量快照保存成功");
        
        // 再次修改数据
        println!("再次修改数据...");
        {
            let table = db.get_table_mut(0)?;
            
            // 修改第6条记录
            #[repr(align(8))]
            struct AlignedRecord([u8; 32]);
            let mut record = AlignedRecord([0; 32]);
            
            table.get_by_id(6, record.0.as_mut_ptr())?;
            let value_value = remdb::Value { u32: 6666 };
            table.set_field(record.0.as_mut_ptr(), 2, &value_value)?
            
            table.delete(6)?;
            table.insert(record.0.as_ptr())?;
        }
        
        // 恢复增量快照
        println!("恢复增量快照...");
        db.restore_snapshot("incremental_snapshot.bin")?;
        println!("增量快照恢复成功");
        
        // 验证增量快照恢复后的数据
        println!("验证增量快照恢复后的数据...");
        let table = db.get_table(0)?;
        
        // 遍历所有记录，检查已使用的记录
        let mut used_records_after_inc = 0;
        let mut total_value_after_inc = 0;
        let mut found_modified_record = false;
        let mut found_new_record = false;
        
        for i in 0..table.max_records() {
            let status_ptr = table.get_status_ptr(i);
            if (*status_ptr).status == remdb::types::RecordStatus::Used {
                // 创建对齐的记录数据
                #[repr(align(8))]
                struct AlignedRecord([u8; 32]); // 手动指定32字节大小（id:8 + name:20 + value:4）
                let mut record = AlignedRecord([0; 32]);
                
                let record_ptr = table.get_record_ptr(i);
                remdb::platform::memcpy(record.0.as_mut_ptr(), record_ptr, 32);
                
                // 获取字段值
                let id_value = table.get_field(record.0.as_ptr(), 0)?;
                let name_value = table.get_field(record.0.as_ptr(), 1)?;
                let value_value = table.get_field(record.0.as_ptr(), 2)?;
                
                // 提取值
                let id = id_value.u64;
                let name_bytes = name_value.string.as_slice();
                let name_len = name_bytes.iter().position(|&c| c == 0).unwrap_or(name_bytes.len());
                let name = std::str::from_utf8(&name_bytes[..name_len]).unwrap_or("invalid_utf8");
                let value = value_value.u32;
                
                println!("记录索引 {}: id={}, name={:?}, value={}", i, id, name, value);
                
                // 检查修改后的记录
                if id == 5 && value == 5555 {
                    found_modified_record = true;
                }
                
                // 检查新增的记录
                if id == 10 && value == 1000 {
                    found_new_record = true;
                }
                
                // 累加value，用于验证总和
                total_value_after_inc += value;
                used_records_after_inc += 1;
            }
        }
        
        // 验证已使用的记录数是否为11（10条原始记录+1条新增记录）
        assert_eq!(used_records_after_inc, 11);
        
        // 验证修改的记录是否恢复
        assert!(found_modified_record, "修改后的记录未找到");
        
        // 验证新增的记录是否恢复
        assert!(found_new_record, "新增的记录未找到");
        
        println!("所有测试通过！");
        Ok(())
    }
}