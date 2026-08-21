//! Integration tests for RawRecordView — zero-copy record access.
//!
//! Tests exercise every typed accessor, error paths, and the backwards-compatible
//! `to_typed_value()` fallback.

use remdb::record_view::RawRecordView;
use remdb::types::{DataType, FieldDef, TableDef, Value, MAX_STRING_LEN};

/// Build a small table definition with one field of each type family.
fn make_test_table_def() -> TableDef {
    let fields: &[FieldDef] = &[
        FieldDef {
            name: "id",
            data_type: DataType::UInt32,
            size: 4,
            offset: 0,
            primary_key: true,
            not_null: true,
            unique: true,
            auto_increment: false,
            default_value: None,
        },
        FieldDef {
            name: "name",
            data_type: DataType::String,
            size: 64,
            offset: 4,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
        },
        FieldDef {
            name: "score",
            data_type: DataType::Float64,
            size: 8,
            offset: 68,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
        },
        FieldDef {
            name: "active",
            data_type: DataType::Bool,
            size: 1,
            offset: 76,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
        },
        FieldDef {
            name: "age",
            data_type: DataType::UInt8,
            size: 1,
            offset: 77,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
        },
        FieldDef {
            name: "count",
            data_type: DataType::UInt16,
            size: 2,
            offset: 78,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
        },
        FieldDef {
            name: "big_val",
            data_type: DataType::UInt64,
            size: 8,
            offset: 80,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
        },
        FieldDef {
            name: "temp",
            data_type: DataType::Int32,
            size: 4,
            offset: 88,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
        },
        FieldDef {
            name: "ratio",
            data_type: DataType::Float32,
            size: 4,
            offset: 92,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
        },
    ];
    TableDef {
        id: 0,
        name: "test",
        fields,
        primary_key: 0,
        secondary_index: None,
        secondary_index_type: remdb::types::IndexType::SortedArray,
        record_size: 96,
        max_records: 100,
    }
}

fn make_record_data() -> Vec<u8> {
    let mut data = vec![0u8; 96];
    // id (UInt32 LE) = 42
    data[0..4].copy_from_slice(&42u32.to_le_bytes());
    // name = "hello"
    let name_bytes = b"hello";
    data[4..4 + name_bytes.len()].copy_from_slice(name_bytes);
    // score (Float64 LE) = 3.14
    data[68..76].copy_from_slice(&3.14f64.to_le_bytes());
    // active (Bool) = true
    data[76] = 1;
    // age (UInt8) = 25
    data[77] = 25;
    // count (UInt16 LE) = 1000
    data[78..80].copy_from_slice(&1000u16.to_le_bytes());
    // big_val (UInt64 LE) = u64::MAX
    data[80..88].copy_from_slice(&u64::MAX.to_le_bytes());
    // temp (Int32 LE) = -42
    data[88..92].copy_from_slice(&(-42i32).to_le_bytes());
    // ratio (Float32 LE) = 2.5
    data[92..96].copy_from_slice(&2.5f32.to_le_bytes());
    data
}

#[test]
fn test_read_u8() {
    let table_def = make_test_table_def();
    let data = make_record_data();
    let view = RawRecordView::new(&data, &table_def);
    assert_eq!(view.read_u8(4).unwrap(), 25);
}

#[test]
fn test_read_u16() {
    let table_def = make_test_table_def();
    let data = make_record_data();
    let view = RawRecordView::new(&data, &table_def);
    assert_eq!(view.read_u16(5).unwrap(), 1000);
}

#[test]
fn test_read_u32() {
    let table_def = make_test_table_def();
    let data = make_record_data();
    let view = RawRecordView::new(&data, &table_def);
    assert_eq!(view.read_u32(0).unwrap(), 42);
}

#[test]
fn test_read_u64() {
    let table_def = make_test_table_def();
    let data = make_record_data();
    let view = RawRecordView::new(&data, &table_def);
    assert_eq!(view.read_u64(6).unwrap(), u64::MAX);
}

#[test]
fn test_read_i32() {
    let table_def = make_test_table_def();
    let data = make_record_data();
    let view = RawRecordView::new(&data, &table_def);
    assert_eq!(view.read_i32(7).unwrap(), -42);
}

#[test]
fn test_read_f32() {
    let table_def = make_test_table_def();
    let data = make_record_data();
    let view = RawRecordView::new(&data, &table_def);
    let val = view.read_f32(8).unwrap();
    assert!((val - 2.5).abs() < 1e-6);
}

#[test]
fn test_read_f64() {
    let table_def = make_test_table_def();
    let data = make_record_data();
    let view = RawRecordView::new(&data, &table_def);
    let val = view.read_f64(2).unwrap();
    assert!((val - 3.14).abs() < 1e-10);
}

#[test]
fn test_read_bool() {
    let table_def = make_test_table_def();
    let data = make_record_data();
    let view = RawRecordView::new(&data, &table_def);
    assert!(view.read_bool(3).unwrap());
}

#[test]
fn test_read_bool_false() {
    let table_def = make_test_table_def();
    let mut data = make_record_data();
    data[76] = 0; // active = false
    let view = RawRecordView::new(&data, &table_def);
    assert!(!view.read_bool(3).unwrap());
}

#[test]
fn test_read_str() {
    let table_def = make_test_table_def();
    let data = make_record_data();
    let view = RawRecordView::new(&data, &table_def);
    assert_eq!(view.read_str(1).unwrap(), "hello");
}

#[test]
fn test_read_str_empty() {
    let table_def = make_test_table_def();
    let mut data = make_record_data();
    // Zero out the name field
    data[4..68].fill(0);
    let view = RawRecordView::new(&data, &table_def);
    assert_eq!(view.read_str(1).unwrap(), "");
}

#[test]
fn test_read_str_max_length() {
    let table_def = make_test_table_def();
    let mut data = make_record_data();
    // Fill name field with a 63-byte string (max without null terminator ambiguity)
    let long_str = "a".repeat(63);
    let bytes = long_str.as_bytes();
    data[4..4 + bytes.len()].copy_from_slice(bytes);
    data[4 + bytes.len()] = 0; // null terminator
    let view = RawRecordView::new(&data, &table_def);
    assert_eq!(view.read_str(1).unwrap(), long_str);
}

#[test]
fn test_read_raw() {
    let table_def = make_test_table_def();
    let data = make_record_data();
    let view = RawRecordView::new(&data, &table_def);
    let raw = view.read_raw(0).unwrap();
    assert_eq!(raw, &42u32.to_le_bytes());
}

#[test]
fn test_to_typed_value_u32() {
    let table_def = make_test_table_def();
    let data = make_record_data();
    let view = RawRecordView::new(&data, &table_def);
    let val = view.to_typed_value(0).unwrap();
    match val {
        Value::U32(v) => assert_eq!(v, 42),
        _ => panic!("Expected U32"),
    }
}

#[test]
fn test_to_typed_value_string() {
    let table_def = make_test_table_def();
    let data = make_record_data();
    let view = RawRecordView::new(&data, &table_def);
    let val = view.to_typed_value(1).unwrap();
    match val {
        Value::String(v) => {
            let s = core::str::from_utf8(&v).unwrap().trim_end_matches(char::from(0));
            assert_eq!(s, "hello");
        }
        _ => panic!("Expected String"),
    }
}

#[test]
fn test_type_mismatch_error() {
    let table_def = make_test_table_def();
    let data = make_record_data();
    let view = RawRecordView::new(&data, &table_def);
    // Field 0 is UInt32, trying to read as UInt8 should fail
    assert!(view.read_u8(0).is_err());
    // Field 2 is Float64, trying to read as UInt32 should fail
    assert!(view.read_u32(2).is_err());
    // Field 3 is Bool, trying to read as String should fail
    assert!(view.read_str(3).is_err());
}

#[test]
fn test_field_not_found_error() {
    let table_def = make_test_table_def();
    let data = make_record_data();
    let view = RawRecordView::new(&data, &table_def);
    // Field index 99 doesn't exist
    assert!(view.read_u8(99).is_err());
    assert!(view.read_u32(99).is_err());
    assert!(view.read_f64(99).is_err());
    assert!(view.read_str(99).is_err());
}

#[test]
fn test_resolve_field_index() {
    let table_def = make_test_table_def();
    let data = make_record_data();
    let view = RawRecordView::new(&data, &table_def);
    let idx = view.resolve_field_index("id").unwrap();
    assert_eq!(idx, 0);
    let idx = view.resolve_field_index("test.name").unwrap();
    assert_eq!(idx, 1);
    let idx = view.resolve_field_index("score").unwrap();
    assert_eq!(idx, 2);
}

#[test]
fn test_resolve_field_index_not_found() {
    let table_def = make_test_table_def();
    let data = make_record_data();
    let view = RawRecordView::new(&data, &table_def);
    assert!(view.resolve_field_index("nonexistent").is_err());
}

#[test]
fn test_read_i8_i16_i64() {
    // Create a table with signed integer types
    let fields: &[FieldDef] = &[
        FieldDef {
            name: "a",
            data_type: DataType::Int8,
            size: 1,
            offset: 0,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
        },
        FieldDef {
            name: "b",
            data_type: DataType::Int16,
            size: 2,
            offset: 1,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
        },
        FieldDef {
            name: "c",
            data_type: DataType::Int64,
            size: 8,
            offset: 3,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
        },
    ];
    let table_def = TableDef {
        id: 1,
        name: "signed_test",
        fields,
        primary_key: 0,
        secondary_index: None,
        secondary_index_type: remdb::types::IndexType::SortedArray,
        record_size: 11,
        max_records: 10,
    };
    let mut data = vec![0u8; 11];
    data[0] = -5i8 as u8;
    data[1..3].copy_from_slice(&(-1000i16).to_le_bytes());
    data[3..11].copy_from_slice(&(-999999999i64).to_le_bytes());

    let view = RawRecordView::new(&data, &table_def);
    assert_eq!(view.read_i8(0).unwrap(), -5);
    assert_eq!(view.read_i16(1).unwrap(), -1000);
    assert_eq!(view.read_i64(2).unwrap(), -999999999);
}

#[test]
fn test_table_field_alias_resolution() {
    let table_def = make_test_table_def();
    let data = make_record_data();
    let view = RawRecordView::new(&data, &table_def);
    // test.id should resolve to the "id" field (index 0)
    let idx = view.resolve_field_index("test.id").unwrap();
    assert_eq!(idx, 0);
    assert_eq!(view.read_u32(idx).unwrap(), 42);
}