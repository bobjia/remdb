#![allow(static_mut_refs)]
extern crate alloc;
use alloc::boxed::Box;
use alloc::sync::Arc;
use remdb::platform::*;
use remdb::table::*;
use remdb::types::*;
use std::sync::Mutex;

mod common;
use common::{setup_test_db, setup_test_db_with_memory};

// 全局互斥锁，确保测试串行执行
static TEST_MUTEX: Mutex<()> = Mutex::new(());

// 简单的表定义用于测试
static TEST_TABLE_DEF: std::sync::LazyLock<TableDef> = std::sync::LazyLock::new(|| TableDef {
    id: 0,
    name: "test_table".to_string(),
    fields: vec![
        FieldDef {
            name: "id".to_string(),
            data_type: DataType::UInt32,
            size: 4,
            string_length: None,
            offset: 0,
            primary_key: true,
            not_null: true,
            unique: true,
            auto_increment: true,
            default_value: None,
            vector_metadata: None,
            json_metadata: None,
        },
        FieldDef {
            name: "value".to_string(),
            data_type: DataType::Float32,
            size: 4,
            string_length: None,
            offset: 4,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
            json_metadata: None,
        },
    ],
    primary_key: vec![0],
    secondary_index: None,
    secondary_index_type: IndexType::SortedArray,
    record_size: 8,
    max_records: 100,
    version: 1,
    created_at: 0,
    updated_at: 0,
});

#[test]
fn test_table_insert_delete() {
    let _guard = TEST_MUTEX.lock().unwrap();

    setup_test_db();

    // 创建表
    let table_def = Arc::new(TEST_TABLE_DEF.clone());
    let mut table = MemoryTable::new(table_def).unwrap();

    // 创建测试记录
    let mut record_data = [0u8; 8];
    let id: u32 = 1;
    let value: f32 = 3.14;

    unsafe {
        core::ptr::copy_nonoverlapping(&id as *const u32 as *const u8, record_data.as_mut_ptr(), 4);
        core::ptr::copy_nonoverlapping(
            &value as *const f32 as *const u8,
            record_data.as_mut_ptr().add(4),
            4,
        );
    }

    // 测试插入记录
    let record_id = table.insert(record_data.as_ptr()).unwrap();
    assert_eq!(record_id, 0);
    assert_eq!(table.record_count(), 1);

    // 测试获取记录
    let mut result_data = [0u8; 8];
    unsafe {
        table
            .get_by_id(record_id, result_data.as_mut_ptr())
            .unwrap();
    }

    let result_id = unsafe { core::ptr::read(result_data.as_ptr() as *const u32) };
    let result_value = unsafe { core::ptr::read(result_data.as_ptr().add(4) as *const f32) };

    assert_eq!(result_id, id);
    assert_eq!(result_value, value);

    // 测试删除记录
    unsafe {
        table.delete(record_id).unwrap();
    }
    assert_eq!(table.record_count(), 0);

    // 测试删除不存在的记录
    let result = unsafe { table.delete(record_id) };
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), RemDbError::RecordNotFound);
}

#[test]
fn test_table_get_field() {
    let _guard = TEST_MUTEX.lock().unwrap();

    setup_test_db();

    // 创建表
    let table_def = Arc::new(TEST_TABLE_DEF.clone());
    let mut table = MemoryTable::new(table_def).unwrap();

    // 创建测试记录
    let mut record_data = [0u8; 8];
    let id: u32 = 1;
    let value: f32 = 3.14;

    unsafe {
        core::ptr::copy_nonoverlapping(&id as *const u32 as *const u8, record_data.as_mut_ptr(), 4);
    }
    unsafe {
        core::ptr::copy_nonoverlapping(
            &value as *const f32 as *const u8,
            record_data.as_mut_ptr().add(4),
            4,
        );
    }

    // 插入记录
    let record_id = table.insert(record_data.as_ptr()).unwrap();

    // 获取记录数据
    let mut result_data = [0u8; 8];
    unsafe {
        table
            .get_by_id(record_id, result_data.as_mut_ptr())
            .unwrap();
    }

    // 测试获取字段值
    let id_value = unsafe { table.get_field(result_data.as_ptr(), 0) }.unwrap();
    unsafe {
        assert_eq!(id_value.u32, id);
    }

    let value_value = unsafe { table.get_field(result_data.as_ptr(), 1) }.unwrap();
    unsafe {
        assert_eq!(value_value.float32, value);
    }

    // 测试获取不存在的字段
    let result = unsafe { table.get_field(result_data.as_ptr(), 2) };
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), RemDbError::FieldNotFound);
}

#[test]
fn test_table_set_field() {
    let _guard = TEST_MUTEX.lock().unwrap();

    setup_test_db();

    // 创建表
    let table_def = Arc::new(TEST_TABLE_DEF.clone());
    let mut table = MemoryTable::new(table_def).unwrap();

    // 创建测试记录
    let mut record_data = [0u8; 8];
    let id: u32 = 1;
    let value: f32 = 3.14;

    unsafe {
        core::ptr::copy_nonoverlapping(&id as *const u32 as *const u8, record_data.as_mut_ptr(), 4);
    }
    unsafe {
        core::ptr::copy_nonoverlapping(
            &value as *const f32 as *const u8,
            record_data.as_mut_ptr().add(4),
            4,
        );
    }

    // 插入记录
    let record_id = table.insert(record_data.as_ptr()).unwrap();

    // 获取记录数据
    let mut result_data = [0u8; 8];
    unsafe {
        table
            .get_by_id(record_id, result_data.as_mut_ptr())
            .unwrap();
    }

    // 测试更新字段值
    let new_value = Value { float32: 6.28 };
    unsafe {
        table
            .set_field(result_data.as_mut_ptr(), 1, &new_value)
            .unwrap();
    }

    // 验证更新
    let updated_value = unsafe { table.get_field(result_data.as_ptr(), 1) }.unwrap();
    unsafe {
        assert_eq!(updated_value.float32, 6.28);
    }

    // 测试更新不存在的字段
    let result = unsafe { table.set_field(result_data.as_mut_ptr(), 2, &new_value) };
    assert!(result.is_err());
    unsafe {
        assert_eq!(result.unwrap_err(), RemDbError::FieldNotFound);
    }
}

#[test]
fn test_table_iterate() {
    let _guard = TEST_MUTEX.lock().unwrap();

    setup_test_db();

    // 创建表
    let table_def = Arc::new(TEST_TABLE_DEF.clone());
    let mut table = MemoryTable::new(table_def).unwrap();

    // 插入多条记录
    for i in 0..5 {
        let mut record_data = [0u8; 8];
        let id: u32 = (i + 1) as u32;
        let value: f32 = (i as f32) * 1.0;

        unsafe {
            core::ptr::copy_nonoverlapping(
                &id as *const u32 as *const u8,
                record_data.as_mut_ptr(),
                4,
            );

            core::ptr::copy_nonoverlapping(
                &value as *const f32 as *const u8,
                record_data.as_mut_ptr().add(4),
                4,
            );
        }

        table.insert(record_data.as_ptr()).unwrap();
    }

    // 测试遍历记录
    let mut count = 0;
    let mut sum = 0.0;

    unsafe {
        table
            .iterate(|_id, data_ptr| {
                let id = core::ptr::read(data_ptr as *const u32);
                let value = core::ptr::read(data_ptr.add(4) as *const f32);

                count += 1;
                sum += value;

                true // 继续遍历
            })
            .unwrap();
    }

    assert_eq!(count, 5);
    assert_eq!(sum, 10.0); // 0+1+2+3+4 = 10
}

// 小表定义用于测试
static SMALL_TABLE_DEF: std::sync::LazyLock<TableDef> = std::sync::LazyLock::new(|| TableDef {
    id: 1,
    name: "small_table".to_string(),
    fields: vec![FieldDef {
        name: "id".to_string(),
        data_type: DataType::UInt32,
        size: 4,
        string_length: None,
        offset: 0,
        primary_key: true,
        not_null: true,
        unique: true,
        auto_increment: true,
        default_value: None,
        vector_metadata: None,
        json_metadata: None,
    }],
    primary_key: vec![0],
    secondary_index: None,
    secondary_index_type: IndexType::SortedArray,
    record_size: 4,
    max_records: 2,
    version: 1,
    created_at: 0,
    updated_at: 0,
});

#[test]
fn test_table_full() {
    let _guard = TEST_MUTEX.lock().unwrap();

    setup_test_db();

    // 创建表
    let table_def = Arc::new(SMALL_TABLE_DEF.clone());
    let mut table = MemoryTable::new(table_def).unwrap();

    // 创建测试记录
    let mut record_data = [0u8; 4];
    let id: u32 = 1;

    unsafe {
        core::ptr::copy_nonoverlapping(&id as *const u32 as *const u8, record_data.as_mut_ptr(), 4);
    }

    // 插入两条记录（表满）
    let record_id1 = table.insert(record_data.as_ptr()).unwrap();
    assert_eq!(record_id1, 0);

    // 创建第二条记录，使用不同的id
    let mut record_data2 = [0u8; 4];
    let id2: u32 = 2;
    unsafe {
        core::ptr::copy_nonoverlapping(
            &id2 as *const u32 as *const u8,
            record_data2.as_mut_ptr(),
            4,
        );
    }

    let record_id2 = table.insert(record_data2.as_ptr()).unwrap();
    assert_eq!(record_id2, 1);

    // 尝试插入第三条记录（应该失败）
    // 使用新的id=3，这样会触发OutOfMemory错误，而不是DuplicateKey错误
    let mut record_data3 = [0u8; 4];
    let id3: u32 = 3;
    unsafe {
        core::ptr::copy_nonoverlapping(
            &id3 as *const u32 as *const u8,
            record_data3.as_mut_ptr(),
            4,
        );
    }
    let result = table.insert(record_data3.as_ptr());
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), RemDbError::OutOfMemory);

    assert_eq!(table.record_count(), 2);
    assert!(table.is_full());
}

#[test]
fn test_not_null_constraint() {
    let _guard = TEST_MUTEX.lock().unwrap();

    setup_test_db();

    // 创建表
    let table_def = Arc::new(TEST_TABLE_DEF.clone());
    let mut table = MemoryTable::new(table_def).unwrap();

    // 测试1：插入id为0的记录，应该成功，因为0是合法的整数值
    let mut zero_id_record = [0u8; 8]; // id为0，是合法值
    let id: u32 = 0;
    let value: f32 = 3.14;

    unsafe {
        core::ptr::copy_nonoverlapping(
            &id as *const u32 as *const u8,
            zero_id_record.as_mut_ptr(),
            4,
        );
        core::ptr::copy_nonoverlapping(
            &value as *const f32 as *const u8,
            zero_id_record.as_mut_ptr().add(4),
            4,
        );
    }

    let record_id = table.insert(zero_id_record.as_ptr()).unwrap();
    assert_eq!(record_id, 0);
    assert_eq!(table.record_count(), 1);

    // 测试2：创建一个包含不同数据类型的表定义
    let table_with_nullable = TableDef {
        id: 2,
        name: "test_nullable_table".to_string(),
        fields: vec![
            FieldDef {
                name: "id".to_string(),
                data_type: DataType::UInt32,
                size: 4,
                string_length: None,
                offset: 0,
                primary_key: true,
                not_null: true,
                unique: true,
                auto_increment: true,
                default_value: None,
                vector_metadata: None,
                json_metadata: None,
            },
            FieldDef {
                name: "name".to_string(),
                data_type: DataType::VarChar,
                size: 16,
                string_length: Some(16),
                offset: 4,
                primary_key: false,
                not_null: true,
                unique: false,
                auto_increment: false,
                default_value: None,
                vector_metadata: None,
                json_metadata: None,
            },
            FieldDef {
                name: "value_float".to_string(),
                data_type: DataType::Float32,
                size: 4,
                string_length: None,
                offset: 20,
                primary_key: false,
                not_null: true,
                unique: false,
                auto_increment: false,
                default_value: None,
                vector_metadata: None,
                json_metadata: None,
            },
            FieldDef {
                name: "value_int".to_string(),
                data_type: DataType::Int32,
                size: 4,
                string_length: None,
                offset: 24,
                primary_key: false,
                not_null: true,
                unique: false,
                auto_increment: false,
                default_value: None,
                vector_metadata: None,
                json_metadata: None,
            },
        ],
        primary_key: vec![0],
        secondary_index: None,
        secondary_index_type: IndexType::SortedArray,
        record_size: 28,
        max_records: 100,
        version: 1,
        created_at: 0,
        updated_at: 0,
    };

    let table_def2 = Arc::new(table_with_nullable);
    let mut table2 = MemoryTable::new(table_def2).unwrap();

    // 测试3：尝试插入null字符串字段，应该失败
    let mut null_string_record = [0u8; 28];
    let id2: u32 = 1;
    let value_int: i32 = 42;

    unsafe {
        core::ptr::copy_nonoverlapping(
            &id2 as *const u32 as *const u8,
            null_string_record.as_mut_ptr(),
            4,
        );
    }
    // 字符串字段保持全0（null）
    // value_float字段保持全0（合法的0.0值）
    unsafe {
        core::ptr::copy_nonoverlapping(
            &value_int as *const i32 as *const u8,
            null_string_record.as_mut_ptr().add(24),
            4,
        );
    }

    let result2 = table2.insert(null_string_record.as_ptr());
    assert!(result2.is_err());
    assert_eq!(result2.unwrap_err(), RemDbError::NotNullViolation);

    // 测试4：尝试插入NaN作为浮点数，应该失败
    let mut nan_float_record = [0u8; 28];
    let id3: u32 = 2;
    let nan_value = f32::NAN; // 不是一个数
    let value_int3: i32 = 42;

    unsafe {
        core::ptr::copy_nonoverlapping(
            &id3 as *const u32 as *const u8,
            nan_float_record.as_mut_ptr(),
            4,
        );
    }
    // 设置非空字符串
    let name = "test_name";
    unsafe {
        core::ptr::copy_nonoverlapping(
            name.as_ptr(),
            nan_float_record.as_mut_ptr().add(4),
            name.len(),
        );
    }
    // 设置NaN值
    unsafe {
        core::ptr::copy_nonoverlapping(
            &nan_value as *const f32 as *const u8,
            nan_float_record.as_mut_ptr().add(20),
            4,
        );
    }
    unsafe {
        core::ptr::copy_nonoverlapping(
            &value_int3 as *const i32 as *const u8,
            nan_float_record.as_mut_ptr().add(24),
            4,
        );
    }

    let result3 = table2.insert(nan_float_record.as_ptr());
    assert!(result3.is_err());
    assert_eq!(result3.unwrap_err(), RemDbError::TypeMismatch);

    // 测试5：插入有效记录，应该成功
    let mut valid_record = [0u8; 28];
    let id4: u32 = 3;
    let value_float4: f32 = 3.14;
    let value_int4: i32 = 0; // 0是合法的整数值

    unsafe {
        core::ptr::copy_nonoverlapping(
            &id4 as *const u32 as *const u8,
            valid_record.as_mut_ptr(),
            4,
        );
    }
    // 设置非空字符串
    let name4 = "test_name_4";
    unsafe {
        core::ptr::copy_nonoverlapping(
            name4.as_ptr(),
            valid_record.as_mut_ptr().add(4),
            name4.len(),
        );
    }
    // 设置有效浮点数
    unsafe {
        core::ptr::copy_nonoverlapping(
            &value_float4 as *const f32 as *const u8,
            valid_record.as_mut_ptr().add(20),
            4,
        );
    }
    // 设置整数值0（合法值）
    unsafe {
        core::ptr::copy_nonoverlapping(
            &value_int4 as *const i32 as *const u8,
            valid_record.as_mut_ptr().add(24),
            4,
        );
    }

    let record_id4 = table2.insert(valid_record.as_ptr()).unwrap();
    assert_eq!(record_id4, 0);
    assert_eq!(table2.record_count(), 1);
}

#[test]
fn test_table_record_ref_and_scan_ref() {
    let _guard = TEST_MUTEX.lock().unwrap();

    setup_test_db();

    let fields = vec![
        FieldDef {
            name: "id".to_string(),
            data_type: DataType::UInt32,
            size: 4,
            string_length: None,
            offset: 0,
            primary_key: true,
            not_null: true,
            unique: true,
            auto_increment: true,
            default_value: None,
            vector_metadata: None,
            json_metadata: None,
        },
        FieldDef {
            name: "name".to_string(),
            data_type: DataType::VarChar,
            size: 8,
            string_length: Some(8),
            offset: 4,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
            json_metadata: None,
        },
    ];

    let table_def = TableDef {
        id: 3,
        name: "ref_table".to_string(),
        fields,
        primary_key: vec![0],
        secondary_index: None,
        secondary_index_type: IndexType::SortedArray,
        record_size: 12,
        max_records: 10,
        version: 1,
        created_at: 0,
        updated_at: 0,
    };

    let table_def = Arc::new(table_def);
    let mut table = MemoryTable::new(table_def).unwrap();

    let mut record1 = [0u8; 12];
    let id1: u32 = 1;
    unsafe {
        core::ptr::copy_nonoverlapping(&id1 as *const u32 as *const u8, record1.as_mut_ptr(), 4);
    }
    let name1 = b"alice";
    unsafe {
        core::ptr::copy_nonoverlapping(name1.as_ptr(), record1.as_mut_ptr().add(4), name1.len());
    }
    table.insert(record1.as_ptr()).unwrap();

    let mut record2 = [0u8; 12];
    let id2: u32 = 2;
    unsafe {
        core::ptr::copy_nonoverlapping(&id2 as *const u32 as *const u8, record2.as_mut_ptr(), 4);
    }
    let name2 = b"bob";
    unsafe {
        core::ptr::copy_nonoverlapping(name2.as_ptr(), record2.as_mut_ptr().add(4), name2.len());
    }
    table.insert(record2.as_ptr()).unwrap();

    let record_ref = table.get_by_id_ref(0).expect("record not found");
    assert_eq!(record_ref.get_u32(0).unwrap(), 1);
    assert_eq!(record_ref.get_str(1).unwrap(), "alice");

    let count = table.scan_ref().count();
    assert_eq!(count, 2);
}
