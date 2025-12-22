use remdb::table::*;
use remdb::types::*;

// 简单的表定义用于测试
static TEST_TABLE_DEF: TableDef = TableDef {
    name: "test_table",
    fields: &[
        FieldDef {
            name: "id",
            data_type: DataType::Int32,
            size: 4,
            offset: 0,
        },
        FieldDef {
            name: "value",
            data_type: DataType::Float32,
            size: 4,
            offset: 4,
        },
    ],
    primary_key: 0,
    secondary_index: None,
    record_size: 8,
    max_records: 100,
};

#[test]
fn test_table_insert_delete() {
    // 分配内存缓冲区
    let mut data_buffer = [0u8; 8 * 100]; // 8字节记录 * 100条
    let mut status_buffer = [RecordHeader { 
        status: RecordStatus::Free, 
        version: 0 
    }; 100];
    
    unsafe {
        // 创建表
        let mut table = MemoryTable::new(
            &TEST_TABLE_DEF,
            data_buffer.as_mut_ptr(),
            status_buffer.as_mut_ptr()
        );
        
        // 创建测试记录
        let mut record_data = [0u8; 8];
        let id: i32 = 1;
        let value: f32 = 3.14;
        
        core::ptr::copy_nonoverlapping(
            &id as *const i32 as *const u8,
            record_data.as_mut_ptr(),
            4
        );
        core::ptr::copy_nonoverlapping(
            &value as *const f32 as *const u8,
            record_data.as_mut_ptr().add(4),
            4
        );
        
        // 测试插入记录
        let record_id = table.insert(record_data.as_ptr()).unwrap();
        assert_eq!(record_id, 0);
        assert_eq!(table.record_count(), 1);
        
        // 测试获取记录
        let mut result_data = [0u8; 8];
        table.get_by_id(record_id, result_data.as_mut_ptr()).unwrap();
        
        let result_id = core::ptr::read(result_data.as_ptr() as *const i32);
        let result_value = core::ptr::read(result_data.as_ptr().add(4) as *const f32);
        
        assert_eq!(result_id, id);
        assert_eq!(result_value, value);
        
        // 测试删除记录
        table.delete(record_id).unwrap();
        assert_eq!(table.record_count(), 0);
        
        // 测试删除不存在的记录
        let result = table.delete(record_id);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), RemDbError::RecordNotFound);
    }
}

#[test]
fn test_table_get_field() {
    // 分配内存缓冲区
    let mut data_buffer = [0u8; 8 * 100];
    let mut status_buffer = [RecordHeader { 
        status: RecordStatus::Free, 
        version: 0 
    }; 100];
    
    unsafe {
        // 创建表
        let mut table = MemoryTable::new(
            &TEST_TABLE_DEF,
            data_buffer.as_mut_ptr(),
            status_buffer.as_mut_ptr()
        );
        
        // 创建测试记录
        let mut record_data = [0u8; 8];
        let id: i32 = 1;
        let value: f32 = 3.14;
        
        core::ptr::copy_nonoverlapping(
            &id as *const i32 as *const u8,
            record_data.as_mut_ptr(),
            4
        );
        core::ptr::copy_nonoverlapping(
            &value as *const f32 as *const u8,
            record_data.as_mut_ptr().add(4),
            4
        );
        
        // 插入记录
        let record_id = table.insert(record_data.as_ptr()).unwrap();
        
        // 获取记录数据
        let mut result_data = [0u8; 8];
        table.get_by_id(record_id, result_data.as_mut_ptr()).unwrap();
        
        // 测试获取字段值
        let id_value = table.get_field(result_data.as_ptr(), 0).unwrap();
        assert_eq!(id_value.int32, id);
        
        let value_value = table.get_field(result_data.as_ptr(), 1).unwrap();
        assert_eq!(value_value.float32, value);
        
        // 测试获取不存在的字段
        let result = table.get_field(result_data.as_ptr(), 2);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), RemDbError::FieldNotFound);
    }
}

#[test]
fn test_table_set_field() {
    // 分配内存缓冲区
    let mut data_buffer = [0u8; 8 * 100];
    let mut status_buffer = [RecordHeader { 
        status: RecordStatus::Free, 
        version: 0 
    }; 100];
    
    unsafe {
        // 创建表
        let mut table = MemoryTable::new(
            &TEST_TABLE_DEF,
            data_buffer.as_mut_ptr(),
            status_buffer.as_mut_ptr()
        );
        
        // 创建测试记录
        let mut record_data = [0u8; 8];
        let id: i32 = 1;
        let value: f32 = 3.14;
        
        core::ptr::copy_nonoverlapping(
            &id as *const i32 as *const u8,
            record_data.as_mut_ptr(),
            4
        );
        core::ptr::copy_nonoverlapping(
            &value as *const f32 as *const u8,
            record_data.as_mut_ptr().add(4),
            4
        );
        
        // 插入记录
        let record_id = table.insert(record_data.as_ptr()).unwrap();
        
        // 获取记录数据
        let mut result_data = [0u8; 8];
        table.get_by_id(record_id, result_data.as_mut_ptr()).unwrap();
        
        // 测试更新字段值
        let new_value = Value { float32: 6.28 };
        table.set_field(result_data.as_mut_ptr(), 1, &new_value).unwrap();
        
        // 验证更新
        let updated_value = table.get_field(result_data.as_ptr(), 1).unwrap();
        assert_eq!(updated_value.float32, 6.28);
        
        // 测试更新不存在的字段
        let result = table.set_field(result_data.as_mut_ptr(), 2, &new_value);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), RemDbError::FieldNotFound);
    }
}

#[test]
fn test_table_iterate() {
    // 分配内存缓冲区
    let mut data_buffer = [0u8; 8 * 100];
    let mut status_buffer = [RecordHeader { 
        status: RecordStatus::Free, 
        version: 0 
    }; 100];
    
    unsafe {
        // 创建表
        let mut table = MemoryTable::new(
            &TEST_TABLE_DEF,
            data_buffer.as_mut_ptr(),
            status_buffer.as_mut_ptr()
        );
        
        // 插入多条记录
        for i in 0..5 {
            let mut record_data = [0u8; 8];
            let id: i32 = (i + 1) as i32;
            let value: f32 = (i as f32) * 1.0;
            
            core::ptr::copy_nonoverlapping(
                &id as *const i32 as *const u8,
                record_data.as_mut_ptr(),
                4
            );
            core::ptr::copy_nonoverlapping(
                &value as *const f32 as *const u8,
                record_data.as_mut_ptr().add(4),
                4
            );
            
            table.insert(record_data.as_ptr()).unwrap();
        }
        
        // 测试遍历记录
        let mut count = 0;
        let mut sum = 0.0;
        
        table.iterate(|_id, data_ptr| {
            let id = core::ptr::read(data_ptr as *const i32);
            let value = core::ptr::read(data_ptr.add(4) as *const f32);
            
            count += 1;
            sum += value;
            
            true // 继续遍历
        }).unwrap();
        
        assert_eq!(count, 5);
        assert_eq!(sum, 10.0); // 0+1+2+3+4 = 10
    }
}

#[test]
fn test_table_full() {
    // 创建一个小表用于测试
    let small_table_def = TableDef {
        name: "small_table",
        fields: &[
            FieldDef {
                name: "id",
                data_type: DataType::Int32,
                size: 4,
                offset: 0,
            },
        ],
        primary_key: 0,
        secondary_index: None,
        record_size: 4,
        max_records: 2,
    };
    
    // 分配内存缓冲区
    let mut data_buffer = [0u8; 4 * 2]; // 4字节记录 * 2条
    let mut status_buffer = [RecordHeader { 
        status: RecordStatus::Free, 
        version: 0 
    }; 2];
    
    unsafe {
        // 创建表
        let mut table = MemoryTable::new(
            &small_table_def,
            data_buffer.as_mut_ptr(),
            status_buffer.as_mut_ptr()
        );
        
        // 创建测试记录
        let mut record_data = [0u8; 4];
        let id: i32 = 1;
        
        core::ptr::copy_nonoverlapping(
            &id as *const i32 as *const u8,
            record_data.as_mut_ptr(),
            4
        );
        
        // 插入两条记录（表满）
        let record_id1 = table.insert(record_data.as_ptr()).unwrap();
        assert_eq!(record_id1, 0);
        
        let record_id2 = table.insert(record_data.as_ptr()).unwrap();
        assert_eq!(record_id2, 1);
        
        // 尝试插入第三条记录（应该失败）
        let result = table.insert(record_data.as_ptr());
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), RemDbError::OutOfMemory);
        
        assert_eq!(table.record_count(), 2);
        assert!(table.is_full());
    }
}