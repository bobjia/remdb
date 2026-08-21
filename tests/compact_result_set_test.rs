//! Integration tests for CompactResultSet — compact owned result set.
//!
//! Tests exercise typed accessors, row/column operations, to_string formatting,
//! and error paths.

use remdb::sql::{CompactResultSet, ColumnInfo};
use remdb::types::{DataType, Value};

fn make_columns() -> Vec<ColumnInfo> {
    vec![
        ColumnInfo {
            name: "id".to_string(),
            offset: 0,
            data_type: DataType::UInt32,
            size: 4,
        },
        ColumnInfo {
            name: "name".to_string(),
            offset: 4,
            data_type: DataType::String,
            size: 64,
        },
        ColumnInfo {
            name: "score".to_string(),
            offset: 68,
            data_type: DataType::Float64,
            size: 8,
        },
        ColumnInfo {
            name: "active".to_string(),
            offset: 76,
            data_type: DataType::Bool,
            size: 1,
        },
        ColumnInfo {
            name: "age".to_string(),
            offset: 77,
            data_type: DataType::UInt8,
            size: 1,
        },
    ]
}

fn make_record(id: u32, name: &str, score: f64, active: bool, age: u8) -> Vec<u8> {
    let mut data = vec![0u8; 78];
    data[0..4].copy_from_slice(&id.to_le_bytes());
    let name_bytes = name.as_bytes();
    let copy_len = core::cmp::min(name_bytes.len(), 64);
    data[4..4 + copy_len].copy_from_slice(&name_bytes[..copy_len]);
    data[68..76].copy_from_slice(&score.to_le_bytes());
    data[76] = if active { 1 } else { 0 };
    data[77] = age;
    data
}

#[test]
fn test_empty_result_set() {
    let columns = make_columns();
    let rs = CompactResultSet::new(columns, 78);
    assert_eq!(rs.column_count(), 5);
    assert_eq!(rs.row_count(), 0);
    assert_eq!(rs.to_string(), "Empty result set");
}

#[test]
fn test_single_row() {
    let columns = make_columns();
    let mut rs = CompactResultSet::new(columns, 78);
    let record = make_record(1, "alice", 95.5, true, 30);
    rs.add_record(&record).unwrap();
    assert_eq!(rs.row_count(), 1);
    assert_eq!(rs.get_field_u32(0, 0).unwrap(), 1);
    assert_eq!(rs.get_field_str(0, 1).unwrap(), "alice");
    assert!((rs.get_field_f64(0, 2).unwrap() - 95.5).abs() < 1e-10);
    assert!(rs.get_field_bool(0, 3).unwrap());
    assert_eq!(rs.get_field_u8(0, 4).unwrap(), 30);
}

#[test]
fn test_multiple_rows() {
    let columns = make_columns();
    let mut rs = CompactResultSet::new(columns, 78);
    let records = vec![
        make_record(1, "alice", 95.5, true, 30),
        make_record(2, "bob", 87.0, false, 25),
        make_record(3, "charlie", 92.3, true, 35),
    ];
    for r in &records {
        rs.add_record(r).unwrap();
    }
    assert_eq!(rs.row_count(), 3);
    assert_eq!(rs.get_field_u32(0, 0).unwrap(), 1);
    assert_eq!(rs.get_field_str(1, 1).unwrap(), "bob");
    assert!((rs.get_field_f64(2, 2).unwrap() - 92.3).abs() < 1e-10);
    assert!(!rs.get_field_bool(1, 3).unwrap());
    assert_eq!(rs.get_field_u8(2, 4).unwrap(), 35);
}

#[test]
fn test_get_field_typed() {
    let columns = make_columns();
    let mut rs = CompactResultSet::new(columns, 78);
    let record = make_record(42, "test", 3.14, true, 20);
    rs.add_record(&record).unwrap();

    let tv = rs.get_field_typed(0, 0).unwrap();
    assert_eq!(tv.value_type, DataType::UInt32);
    match tv.value {
        Value::U32(v) => assert_eq!(v, 42),
        _ => panic!("Expected U32"),
    }

    let tv = rs.get_field_typed(0, 1).unwrap();
    assert_eq!(tv.value_type, DataType::String);
    match tv.value {
        Value::String(v) => {
            let s = core::str::from_utf8(&v).unwrap().trim_end_matches(char::from(0));
            assert_eq!(s, "test");
        }
        _ => panic!("Expected String"),
    }
}

#[test]
fn test_get_row() {
    let columns = make_columns();
    let mut rs = CompactResultSet::new(columns, 78);
    let record = make_record(1, "alice", 95.5, true, 30);
    rs.add_record(&record).unwrap();

    let row = rs.get_row(0).unwrap();
    assert_eq!(row.len(), 5);
    assert_eq!(row[0].value_type, DataType::UInt32);
}

#[test]
fn test_get_field_raw() {
    let columns = make_columns();
    let mut rs = CompactResultSet::new(columns, 78);
    let record = make_record(1, "alice", 95.5, true, 30);
    rs.add_record(&record).unwrap();

    let raw = rs.get_field_raw(0, 0).unwrap();
    assert_eq!(raw, &1u32.to_le_bytes());
}

#[test]
fn test_out_of_bounds_row() {
    let columns = make_columns();
    let rs = CompactResultSet::new(columns, 78);
    assert!(rs.get_field_u8(0, 0).is_err());
    assert!(rs.get_field_u32(99, 0).is_err());
    assert!(rs.get_row(0).is_err());
}

#[test]
fn test_out_of_bounds_col() {
    let columns = make_columns();
    let mut rs = CompactResultSet::new(columns, 78);
    let record = make_record(1, "test", 1.0, true, 20);
    rs.add_record(&record).unwrap();
    assert!(rs.get_field_u8(0, 99).is_err());
}

#[test]
fn test_type_mismatch() {
    let columns = make_columns();
    let mut rs = CompactResultSet::new(columns, 78);
    let record = make_record(1, "test", 1.0, true, 20);
    rs.add_record(&record).unwrap();
    // Column 0 is UInt32, reading as UInt8 should fail
    assert!(rs.get_field_u8(0, 0).is_err());
    // Column 1 is String, reading as UInt32 should fail
    assert!(rs.get_field_u32(0, 1).is_err());
}

#[test]
fn test_to_string_formatting() {
    let columns = vec![
        ColumnInfo {
            name: "id".to_string(),
            offset: 0,
            data_type: DataType::UInt32,
            size: 4,
        },
        ColumnInfo {
            name: "name".to_string(),
            offset: 4,
            data_type: DataType::String,
            size: 64,
        },
    ];
    let mut rs = CompactResultSet::new(columns, 68);
    let record = make_record(1, "alice", 0.0, false, 0);
    rs.add_record(&record).unwrap();

    let s = rs.to_string();
    assert!(s.contains("id"));
    assert!(s.contains("name"));
    assert!(s.contains("1"));
    assert!(s.contains("alice"));
}

#[test]
fn test_get_field_str_empty() {
    let columns = vec![
        ColumnInfo {
            name: "name".to_string(),
            offset: 0,
            data_type: DataType::String,
            size: 64,
        },
    ];
    let mut rs = CompactResultSet::new(columns, 64);
    // Empty string (all zeros)
    let record = vec![0u8; 64];
    rs.add_record(&record).unwrap();
    assert_eq!(rs.get_field_str(0, 0).unwrap(), "");
}

#[test]
fn test_all_typed_accessors() {
    let columns = vec![
        ColumnInfo { name: "a".to_string(), offset: 0, data_type: DataType::UInt8, size: 1 },
        ColumnInfo { name: "b".to_string(), offset: 1, data_type: DataType::UInt16, size: 2 },
        ColumnInfo { name: "c".to_string(), offset: 3, data_type: DataType::UInt32, size: 4 },
        ColumnInfo { name: "d".to_string(), offset: 7, data_type: DataType::UInt64, size: 8 },
        ColumnInfo { name: "e".to_string(), offset: 15, data_type: DataType::Int8, size: 1 },
        ColumnInfo { name: "f".to_string(), offset: 16, data_type: DataType::Int16, size: 2 },
        ColumnInfo { name: "g".to_string(), offset: 18, data_type: DataType::Int32, size: 4 },
        ColumnInfo { name: "h".to_string(), offset: 22, data_type: DataType::Int64, size: 8 },
        ColumnInfo { name: "i".to_string(), offset: 30, data_type: DataType::Float32, size: 4 },
        ColumnInfo { name: "j".to_string(), offset: 34, data_type: DataType::Float64, size: 8 },
        ColumnInfo { name: "k".to_string(), offset: 42, data_type: DataType::Bool, size: 1 },
    ];
    let mut rs = CompactResultSet::new(columns, 43);
    let mut record = vec![0u8; 43];
    record[0] = 200u8;
    record[1..3].copy_from_slice(&500u16.to_le_bytes());
    record[3..7].copy_from_slice(&100000u32.to_le_bytes());
    record[7..15].copy_from_slice(&u64::MAX.to_le_bytes());
    record[15] = -50i8 as u8;
    record[16..18].copy_from_slice(&(-500i16).to_le_bytes());
    record[18..22].copy_from_slice(&(-100000i32).to_le_bytes());
    record[22..30].copy_from_slice(&(-9999999999i64).to_le_bytes());
    record[30..34].copy_from_slice(&(-3.14f32).to_le_bytes());
    record[34..42].copy_from_slice(&std::f64::consts::PI.to_le_bytes());
    record[42] = 1;

    rs.add_record(&record).unwrap();

    assert_eq!(rs.get_field_u8(0, 0).unwrap(), 200);
    assert_eq!(rs.get_field_u16(0, 1).unwrap(), 500);
    assert_eq!(rs.get_field_u32(0, 2).unwrap(), 100000);
    assert_eq!(rs.get_field_u64(0, 3).unwrap(), u64::MAX);
    assert_eq!(rs.get_field_i8(0, 4).unwrap(), -50);
    assert_eq!(rs.get_field_i16(0, 5).unwrap(), -500);
    assert_eq!(rs.get_field_i32(0, 6).unwrap(), -100000);
    assert_eq!(rs.get_field_i64(0, 7).unwrap(), -9999999999);
    assert!((rs.get_field_f32(0, 8).unwrap() - (-3.14f32)).abs() < 1e-5);
    assert!((rs.get_field_f64(0, 9).unwrap() - std::f64::consts::PI).abs() < 1e-15);
    assert!(rs.get_field_bool(0, 10).unwrap());
}