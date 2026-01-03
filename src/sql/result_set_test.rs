use alloc::string::String;
use crate::{DataType, types::TypedValue};

#[test]
fntest_value_to_string_repr() {
    // 创建不同数据类型的TypedValue实例
    let int32_val = TypedValue {
        value_type: DataType::Int32,
        value: unsafe { core::mem::transmute(42i32) },
    };
    
    let uint64_val = TypedValue {
        value_type: DataType::UInt64,
        value: unsafe { core::mem::transmute(1000000u64) },
    };
    
    let float64_val = TypedValue {
        value_type: DataType::Float64,
        value: unsafe { core::mem::transmute(3.14159f64) },
    };
    
    let bool_val = TypedValue {
        value_type: DataType::Bool,
        value: unsafe { core::mem::transmute(true) },
    };
    
    let mut string_val = TypedValue {
        value_type: DataType::String,
        value: unsafe { core::mem::zeroed() },
    };
    let test_str = "hello world";
    let str_bytes = test_str.as_bytes();
    let str_len = core::cmp::min(str_bytes.len(), 64);
    unsafe {
        core::ptr::copy_nonoverlapping(str_bytes.as_ptr(), &mut string_val.value.string as *mut [u8; 64] as *mut u8, str_len);
    }
    
    let timestamp_val = TypedValue {
        value_type: DataType::Timestamp,
        value: unsafe { core::mem::transmute(1609459200000u64) }, // 2021-01-01 00:00:00 UTC
    };
    
    // 测试字符串表示
    assert_eq!(crate::sql::result_set::value_to_string_repr(&int32_val), "42");
    assert_eq!(crate::sql::result_set::value_to_string_repr(&uint64_val), "1000000");
    assert_eq!(crate::sql::result_set::value_to_string_repr(&float64_val), "3.14159");
    assert_eq!(crate::sql::result_set::value_to_string_repr(&bool_val), "true");
    assert_eq!(crate::sql::result_set::value_to_string_repr(&string_val), "hello world");
    assert_eq!(crate::sql::result_set::value_to_string_repr(&timestamp_val), "1609459200000");
}