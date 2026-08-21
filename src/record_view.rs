//! Zero-copy record view for accessing raw record bytes without allocating Value enums.
//!
//! `RawRecordView<'a>` borrows directly from table storage, providing typed accessors
//! that interpret raw bytes as typed values. This avoids the per-field `Value` allocation
//! that the old `MemoryTable::get_field()` path performs.
//!
//! # no_std compatibility
//!
//! This module uses only `alloc` and `core` — no `std` dependency.

use crate::types::{
    db_interval, db_timestamp, DataType, FieldDef, MAX_STRING_LEN, RemDbError, Result, TableDef,
    Value,
};

/// A borrowed view into a single record's raw bytes in table storage.
///
/// Provides zero-copy typed field access without allocating `Value` enums.
/// The view borrows the record data and table definition, so it has no ownership
/// of the underlying storage.
pub struct RawRecordView<'a> {
    /// Raw record bytes (borrowed from table storage).
    pub data: &'a [u8],
    /// Table definition describing field layout.
    pub table_def: &'a TableDef,
}

impl<'a> RawRecordView<'a> {
    /// Create a new record view from raw record bytes and table definition.
    pub fn new(data: &'a [u8], table_def: &'a TableDef) -> Self {
        RawRecordView { data, table_def }
    }

    /// Resolve a field name (handling `table.field` aliases) to a field index.
    pub fn resolve_field_index(&self, field_name: &str) -> Result<usize> {
        let actual_name = if let Some(dot_pos) = field_name.find('.') {
            &field_name[dot_pos + 1..]
        } else {
            field_name
        };
        self.table_def
            .fields
            .iter()
            .position(|f| f.name == actual_name)
            .ok_or(RemDbError::FieldNotFound)
    }

    /// Get the field definition for a given field index.
    fn get_field_def(&self, field_idx: usize) -> Result<&FieldDef> {
        self.table_def
            .fields
            .get(field_idx)
            .ok_or(RemDbError::FieldNotFound)
    }

    // -----------------------------------------------------------------------
    // Typed read accessors — each reads the raw bytes and interprets them as
    // the requested type.  No allocation, no `Value` enum.
    // -----------------------------------------------------------------------

    /// Read a `UInt8` field by its index.
    pub fn read_u8(&self, field_idx: usize) -> Result<u8> {
        let field = self.get_field_def(field_idx)?;
        if field.data_type != DataType::UInt8 {
            return Err(RemDbError::TypeMismatch);
        }
        self.data
            .get(field.offset)
            .copied()
            .ok_or(RemDbError::FieldNotFound)
    }

    /// Read a `UInt16` field by its index.
    pub fn read_u16(&self, field_idx: usize) -> Result<u16> {
        let field = self.get_field_def(field_idx)?;
        if field.data_type != DataType::UInt16 {
            return Err(RemDbError::TypeMismatch);
        }
        let slice = self
            .data
            .get(field.offset..field.offset + 2)
            .ok_or(RemDbError::FieldNotFound)?;
        Ok(u16::from_le_bytes(
            slice.try_into().map_err(|_| RemDbError::TypeMismatch)?,
        ))
    }

    /// Read a `UInt32` field by its index.
    pub fn read_u32(&self, field_idx: usize) -> Result<u32> {
        let field = self.get_field_def(field_idx)?;
        if field.data_type != DataType::UInt32 {
            return Err(RemDbError::TypeMismatch);
        }
        let slice = self
            .data
            .get(field.offset..field.offset + 4)
            .ok_or(RemDbError::FieldNotFound)?;
        Ok(u32::from_le_bytes(
            slice.try_into().map_err(|_| RemDbError::TypeMismatch)?,
        ))
    }

    /// Read a `UInt64` field by its index.
    pub fn read_u64(&self, field_idx: usize) -> Result<u64> {
        let field = self.get_field_def(field_idx)?;
        if field.data_type != DataType::UInt64 {
            return Err(RemDbError::TypeMismatch);
        }
        let slice = self
            .data
            .get(field.offset..field.offset + 8)
            .ok_or(RemDbError::FieldNotFound)?;
        Ok(u64::from_le_bytes(
            slice.try_into().map_err(|_| RemDbError::TypeMismatch)?,
        ))
    }

    /// Read an `Int8` field by its index.
    pub fn read_i8(&self, field_idx: usize) -> Result<i8> {
        let field = self.get_field_def(field_idx)?;
        if field.data_type != DataType::Int8 {
            return Err(RemDbError::TypeMismatch);
        }
        self.data
            .get(field.offset)
            .map(|&b| b as i8)
            .ok_or(RemDbError::FieldNotFound)
    }

    /// Read an `Int16` field by its index.
    pub fn read_i16(&self, field_idx: usize) -> Result<i16> {
        let field = self.get_field_def(field_idx)?;
        if field.data_type != DataType::Int16 {
            return Err(RemDbError::TypeMismatch);
        }
        let slice = self
            .data
            .get(field.offset..field.offset + 2)
            .ok_or(RemDbError::FieldNotFound)?;
        Ok(i16::from_le_bytes(
            slice.try_into().map_err(|_| RemDbError::TypeMismatch)?,
        ))
    }

    /// Read an `Int32` field by its index.
    pub fn read_i32(&self, field_idx: usize) -> Result<i32> {
        let field = self.get_field_def(field_idx)?;
        if field.data_type != DataType::Int32 {
            return Err(RemDbError::TypeMismatch);
        }
        let slice = self
            .data
            .get(field.offset..field.offset + 4)
            .ok_or(RemDbError::FieldNotFound)?;
        Ok(i32::from_le_bytes(
            slice.try_into().map_err(|_| RemDbError::TypeMismatch)?,
        ))
    }

    /// Read an `Int64` field by its index.
    pub fn read_i64(&self, field_idx: usize) -> Result<i64> {
        let field = self.get_field_def(field_idx)?;
        if field.data_type != DataType::Int64 {
            return Err(RemDbError::TypeMismatch);
        }
        let slice = self
            .data
            .get(field.offset..field.offset + 8)
            .ok_or(RemDbError::FieldNotFound)?;
        Ok(i64::from_le_bytes(
            slice.try_into().map_err(|_| RemDbError::TypeMismatch)?,
        ))
    }

    /// Read a `Float32` field by its index.
    pub fn read_f32(&self, field_idx: usize) -> Result<f32> {
        let field = self.get_field_def(field_idx)?;
        if field.data_type != DataType::Float32 {
            return Err(RemDbError::TypeMismatch);
        }
        let slice = self
            .data
            .get(field.offset..field.offset + 4)
            .ok_or(RemDbError::FieldNotFound)?;
        Ok(f32::from_le_bytes(
            slice.try_into().map_err(|_| RemDbError::TypeMismatch)?,
        ))
    }

    /// Read a `Float64` field by its index.
    pub fn read_f64(&self, field_idx: usize) -> Result<f64> {
        let field = self.get_field_def(field_idx)?;
        if field.data_type != DataType::Float64 {
            return Err(RemDbError::TypeMismatch);
        }
        let slice = self
            .data
            .get(field.offset..field.offset + 8)
            .ok_or(RemDbError::FieldNotFound)?;
        Ok(f64::from_le_bytes(
            slice.try_into().map_err(|_| RemDbError::TypeMismatch)?,
        ))
    }

    /// Read a `Bool` field by its index.
    pub fn read_bool(&self, field_idx: usize) -> Result<bool> {
        let field = self.get_field_def(field_idx)?;
        if field.data_type != DataType::Bool {
            return Err(RemDbError::TypeMismatch);
        }
        self.data
            .get(field.offset)
            .map(|&b| b != 0)
            .ok_or(RemDbError::FieldNotFound)
    }

    /// Zero-copy: returns a `&str` slice into the raw table storage.
    ///
    /// The string is zero-padded; trailing null bytes are trimmed by returning
    /// a sub-slice.
    pub fn read_str(&self, field_idx: usize) -> Result<&'a str> {
        let field = self.get_field_def(field_idx)?;
        if field.data_type != DataType::String {
            return Err(RemDbError::TypeMismatch);
        }
        let slice = self
            .data
            .get(field.offset..field.offset + field.size)
            .ok_or(RemDbError::FieldNotFound)?;
        // Find the first null byte (zero-padding terminator)
        let end = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
        let trimmed_slice = &slice[..end];
        core::str::from_utf8(trimmed_slice).map_err(|_| RemDbError::TypeMismatch)
    }

    /// Read a `Timestamp` or `TimestampTZ` field by its index.
    pub fn read_timestamp(&self, field_idx: usize) -> Result<db_timestamp> {
        let field = self.get_field_def(field_idx)?;
        if field.data_type != DataType::Timestamp && field.data_type != DataType::TimestampTZ {
            return Err(RemDbError::TypeMismatch);
        }
        let value_bytes = self
            .data
            .get(field.offset..field.offset + 8)
            .ok_or(RemDbError::FieldNotFound)?;
        let value = i64::from_le_bytes(
            value_bytes
                .try_into()
                .map_err(|_| RemDbError::TypeMismatch)?,
        );
        let tz_offset = if field.data_type == DataType::TimestampTZ {
            let tz_bytes = self
                .data
                .get(field.offset + 8..field.offset + 10)
                .ok_or(RemDbError::FieldNotFound)?;
            i16::from_le_bytes(
                tz_bytes
                    .try_into()
                    .map_err(|_| RemDbError::TypeMismatch)?,
            )
        } else {
            0
        };
        Ok(db_timestamp {
            value,
            tz_offset,
            precision: 0,
            flags: 0,
        })
    }

    /// Read an `Interval` field by its index.
    pub fn read_interval(&self, field_idx: usize) -> Result<db_interval> {
        let field = self.get_field_def(field_idx)?;
        if field.data_type != DataType::Interval {
            return Err(RemDbError::TypeMismatch);
        }
        let value_bytes = self
            .data
            .get(field.offset..field.offset + 8)
            .ok_or(RemDbError::FieldNotFound)?;
        let value = i64::from_le_bytes(
            value_bytes
                .try_into()
                .map_err(|_| RemDbError::TypeMismatch)?,
        );
        Ok(db_interval {
            value,
            precision: 0,
            flags: 0,
        })
    }

    /// Raw byte access into the table storage (zero-copy).
    ///
    /// Returns a borrowed slice of the raw field bytes.
    pub fn read_raw(&self, field_idx: usize) -> Result<&'a [u8]> {
        let field = self.get_field_def(field_idx)?;
        self.data
            .get(field.offset..field.offset + field.size)
            .ok_or(RemDbError::FieldNotFound)
    }

    // -----------------------------------------------------------------------
    // Fallback: create a `Value` (copies data).
    // -----------------------------------------------------------------------

    /// Fallback: create a `TypedValue` from the field (copies data).
    ///
    /// This provides backwards compatibility with code that expects `Value` enums.
    /// Prefer the typed read accessors for new code.
    pub fn to_typed_value(&self, field_idx: usize) -> Result<Value> {
        let field = self.get_field_def(field_idx)?;
        let offset = field.offset;
        let size = field.size;
        let data = self.data;
        match field.data_type {
            DataType::UInt8 => Ok(Value::U8(
                data.get(offset).copied().ok_or(RemDbError::FieldNotFound)?,
            )),
            DataType::UInt16 => {
                let slice = data
                    .get(offset..offset + 2)
                    .ok_or(RemDbError::FieldNotFound)?;
                Ok(Value::U16(u16::from_le_bytes(
                    slice.try_into().map_err(|_| RemDbError::TypeMismatch)?,
                )))
            }
            DataType::UInt32 => {
                let slice = data
                    .get(offset..offset + 4)
                    .ok_or(RemDbError::FieldNotFound)?;
                Ok(Value::U32(u32::from_le_bytes(
                    slice.try_into().map_err(|_| RemDbError::TypeMismatch)?,
                )))
            }
            DataType::UInt64 => {
                let slice = data
                    .get(offset..offset + 8)
                    .ok_or(RemDbError::FieldNotFound)?;
                Ok(Value::U64(u64::from_le_bytes(
                    slice.try_into().map_err(|_| RemDbError::TypeMismatch)?,
                )))
            }
            DataType::Int8 => Ok(Value::I8(
                data.get(offset)
                    .map(|&b| b as i8)
                    .ok_or(RemDbError::FieldNotFound)?,
            )),
            DataType::Int16 => {
                let slice = data
                    .get(offset..offset + 2)
                    .ok_or(RemDbError::FieldNotFound)?;
                Ok(Value::I16(i16::from_le_bytes(
                    slice.try_into().map_err(|_| RemDbError::TypeMismatch)?,
                )))
            }
            DataType::Int32 => {
                let slice = data
                    .get(offset..offset + 4)
                    .ok_or(RemDbError::FieldNotFound)?;
                Ok(Value::I32(i32::from_le_bytes(
                    slice.try_into().map_err(|_| RemDbError::TypeMismatch)?,
                )))
            }
            DataType::Int64 => {
                let slice = data
                    .get(offset..offset + 8)
                    .ok_or(RemDbError::FieldNotFound)?;
                Ok(Value::I64(i64::from_le_bytes(
                    slice.try_into().map_err(|_| RemDbError::TypeMismatch)?,
                )))
            }
            DataType::Float32 => {
                let slice = data
                    .get(offset..offset + 4)
                    .ok_or(RemDbError::FieldNotFound)?;
                Ok(Value::Float32(f32::from_le_bytes(
                    slice.try_into().map_err(|_| RemDbError::TypeMismatch)?,
                )))
            }
            DataType::Float64 => {
                let slice = data
                    .get(offset..offset + 8)
                    .ok_or(RemDbError::FieldNotFound)?;
                Ok(Value::Float64(f64::from_le_bytes(
                    slice.try_into().map_err(|_| RemDbError::TypeMismatch)?,
                )))
            }
            DataType::Bool => Ok(Value::Bool(
                data.get(offset)
                    .map(|&b| b != 0)
                    .ok_or(RemDbError::FieldNotFound)?,
            )),
            DataType::String => {
                let mut buf = [0u8; MAX_STRING_LEN];
                let copy_size = core::cmp::min(size, MAX_STRING_LEN);
                let end = core::cmp::min(offset + copy_size, data.len());
                buf[..end - offset].copy_from_slice(&data[offset..end]);
                Ok(Value::String(buf))
            }
            DataType::Timestamp | DataType::TimestampTZ => {
                let slice = data
                    .get(offset..offset + 8)
                    .ok_or(RemDbError::FieldNotFound)?;
                let value = i64::from_le_bytes(
                    slice
                        .try_into()
                        .map_err(|_| RemDbError::TypeMismatch)?,
                );
                Ok(Value::Time(db_timestamp {
                    value,
                    tz_offset: 0,
                    precision: 0,
                    flags: 0,
                }))
            }
            DataType::Interval => {
                let slice = data
                    .get(offset..offset + 8)
                    .ok_or(RemDbError::FieldNotFound)?;
                let value = i64::from_le_bytes(
                    slice
                        .try_into()
                        .map_err(|_| RemDbError::TypeMismatch)?,
                );
                Ok(Value::Interval(db_interval {
                    value,
                    precision: 0,
                    flags: 0,
                }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FieldDef, TableDef, DataType, Value};

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
        ];
        TableDef {
            id: 0,
            name: "test",
            fields,
            primary_key: 0,
            secondary_index: None,
            secondary_index_type: crate::types::IndexType::SortedArray,
            record_size: 78,
            max_records: 100,
        }
    }

    fn make_record_data() -> Vec<u8> {
        let mut data = vec![0u8; 78];
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
    fn test_read_u32() {
        let table_def = make_test_table_def();
        let data = make_record_data();
        let view = RawRecordView::new(&data, &table_def);
        assert_eq!(view.read_u32(0).unwrap(), 42);
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
    fn test_read_raw() {
        let table_def = make_test_table_def();
        let data = make_record_data();
        let view = RawRecordView::new(&data, &table_def);
        let raw = view.read_raw(0).unwrap();
        assert_eq!(raw, &42u32.to_le_bytes());
    }

    #[test]
    fn test_to_typed_value() {
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
    fn test_type_mismatch_error() {
        let table_def = make_test_table_def();
        let data = make_record_data();
        let view = RawRecordView::new(&data, &table_def);
        // Field 0 is UInt32, trying to read as UInt8 should fail
        assert!(view.read_u8(0).is_err());
    }

    #[test]
    fn test_field_not_found_error() {
        let table_def = make_test_table_def();
        let data = make_record_data();
        let view = RawRecordView::new(&data, &table_def);
        // Field index 99 doesn't exist
        assert!(view.read_u8(99).is_err());
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
    }
}