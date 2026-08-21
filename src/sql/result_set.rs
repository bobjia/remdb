//! SQL查询结果集
//! 
//! 该模块负责处理SQL查询的结果集，提供友好的结果访问接口。

use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::iter::Iterator;
use core::option::Option;
use core::result::Result as CoreResult;

use crate::{RemDbError, DataType, types::TypedValue, types::Value, MAX_STRING_LEN};

// ============================================================================
// CompactResultSet — compact owned result set with typed accessors
// ============================================================================

/// Column metadata for interpreting `CompactResultSet::raw_data`.
#[derive(Clone, Debug)]
pub struct ColumnInfo {
    /// Column name.
    pub name: String,
    /// Byte offset of this column within the record.
    pub offset: usize,
    /// Data type of this column.
    pub data_type: DataType,
    /// Field size in bytes.
    pub size: usize,
}

/// A result set storing matching records as compact raw bytes.
///
/// Provides typed accessors and backwards-compatible `TypedValue` access,
/// while avoiding the per-row `Vec<TypedValue>` allocation of the old
/// `ResultSet`.
pub struct CompactResultSet {
    /// Column metadata for interpreting `raw_data`.
    pub columns: Vec<ColumnInfo>,
    /// All matching records, row-major contiguous.
    pub raw_data: Vec<u8>,
    /// Size of one record in bytes.
    pub record_size: usize,
    /// Number of records stored.
    pub record_count: usize,
}

impl CompactResultSet {
    /// Create a new empty `CompactResultSet` from column metadata.
    pub fn new(columns: Vec<ColumnInfo>, record_size: usize) -> Self {
        CompactResultSet {
            columns,
            raw_data: Vec::new(),
            record_size,
            record_count: 0,
        }
    }

    /// Add a record by copying its raw bytes.
    pub fn add_record(&mut self, record_data: &[u8]) -> CoreResult<(), RemDbError> {
        let copy_len = core::cmp::min(record_data.len(), self.record_size);
        self.raw_data.extend_from_slice(&record_data[..copy_len]);
        self.record_count += 1;
        Ok(())
    }

    /// Get the raw data slice for a given row.
    pub(crate) fn get_row_slice(&self, row: usize) -> CoreResult<&[u8], RemDbError> {
        if row >= self.record_count {
            return Err(RemDbError::RecordNotFound);
        }
        let start = row * self.record_size;
        let end = start + self.record_size;
        self.raw_data.get(start..end).ok_or(RemDbError::RecordNotFound)
    }

    /// Get a specific column's byte range within a row.
    fn get_col_slice(&self, row: usize, col: usize) -> CoreResult<&[u8], RemDbError> {
        let col_info = self.columns.get(col).ok_or(RemDbError::FieldNotFound)?;
        let row_slice = self.get_row_slice(row)?;
        row_slice
            .get(col_info.offset..col_info.offset + col_info.size)
            .ok_or(RemDbError::FieldNotFound)
    }

    // --- Typed accessors ---

    /// Read a `UInt8` field from a specific row and column.
    pub fn get_field_u8(&self, row: usize, col: usize) -> CoreResult<u8, RemDbError> {
        let col_info = self.columns.get(col).ok_or(RemDbError::FieldNotFound)?;
        if col_info.data_type != DataType::UInt8 {
            return Err(RemDbError::TypeMismatch);
        }
        let slice = self.get_col_slice(row, col)?;
        slice.first().copied().ok_or(RemDbError::FieldNotFound)
    }

    /// Read a `UInt16` field from a specific row and column.
    pub fn get_field_u16(&self, row: usize, col: usize) -> CoreResult<u16, RemDbError> {
        let col_info = self.columns.get(col).ok_or(RemDbError::FieldNotFound)?;
        if col_info.data_type != DataType::UInt16 {
            return Err(RemDbError::TypeMismatch);
        }
        let slice = self.get_col_slice(row, col)?;
        Ok(u16::from_le_bytes(
            slice.try_into().map_err(|_| RemDbError::TypeMismatch)?,
        ))
    }

    /// Read a `UInt32` field from a specific row and column.
    pub fn get_field_u32(&self, row: usize, col: usize) -> CoreResult<u32, RemDbError> {
        let col_info = self.columns.get(col).ok_or(RemDbError::FieldNotFound)?;
        if col_info.data_type != DataType::UInt32 {
            return Err(RemDbError::TypeMismatch);
        }
        let slice = self.get_col_slice(row, col)?;
        Ok(u32::from_le_bytes(
            slice.try_into().map_err(|_| RemDbError::TypeMismatch)?,
        ))
    }

    /// Read a `UInt64` field from a specific row and column.
    pub fn get_field_u64(&self, row: usize, col: usize) -> CoreResult<u64, RemDbError> {
        let col_info = self.columns.get(col).ok_or(RemDbError::FieldNotFound)?;
        if col_info.data_type != DataType::UInt64 {
            return Err(RemDbError::TypeMismatch);
        }
        let slice = self.get_col_slice(row, col)?;
        Ok(u64::from_le_bytes(
            slice.try_into().map_err(|_| RemDbError::TypeMismatch)?,
        ))
    }

    /// Read an `Int8` field from a specific row and column.
    pub fn get_field_i8(&self, row: usize, col: usize) -> CoreResult<i8, RemDbError> {
        let col_info = self.columns.get(col).ok_or(RemDbError::FieldNotFound)?;
        if col_info.data_type != DataType::Int8 {
            return Err(RemDbError::TypeMismatch);
        }
        let slice = self.get_col_slice(row, col)?;
        slice.first().map(|&b| b as i8).ok_or(RemDbError::FieldNotFound)
    }

    /// Read an `Int16` field from a specific row and column.
    pub fn get_field_i16(&self, row: usize, col: usize) -> CoreResult<i16, RemDbError> {
        let col_info = self.columns.get(col).ok_or(RemDbError::FieldNotFound)?;
        if col_info.data_type != DataType::Int16 {
            return Err(RemDbError::TypeMismatch);
        }
        let slice = self.get_col_slice(row, col)?;
        Ok(i16::from_le_bytes(
            slice.try_into().map_err(|_| RemDbError::TypeMismatch)?,
        ))
    }

    /// Read an `Int32` field from a specific row and column.
    pub fn get_field_i32(&self, row: usize, col: usize) -> CoreResult<i32, RemDbError> {
        let col_info = self.columns.get(col).ok_or(RemDbError::FieldNotFound)?;
        if col_info.data_type != DataType::Int32 {
            return Err(RemDbError::TypeMismatch);
        }
        let slice = self.get_col_slice(row, col)?;
        Ok(i32::from_le_bytes(
            slice.try_into().map_err(|_| RemDbError::TypeMismatch)?,
        ))
    }

    /// Read an `Int64` field from a specific row and column.
    pub fn get_field_i64(&self, row: usize, col: usize) -> CoreResult<i64, RemDbError> {
        let col_info = self.columns.get(col).ok_or(RemDbError::FieldNotFound)?;
        if col_info.data_type != DataType::Int64 {
            return Err(RemDbError::TypeMismatch);
        }
        let slice = self.get_col_slice(row, col)?;
        Ok(i64::from_le_bytes(
            slice.try_into().map_err(|_| RemDbError::TypeMismatch)?,
        ))
    }

    /// Read a `Float32` field from a specific row and column.
    pub fn get_field_f32(&self, row: usize, col: usize) -> CoreResult<f32, RemDbError> {
        let col_info = self.columns.get(col).ok_or(RemDbError::FieldNotFound)?;
        if col_info.data_type != DataType::Float32 {
            return Err(RemDbError::TypeMismatch);
        }
        let slice = self.get_col_slice(row, col)?;
        Ok(f32::from_le_bytes(
            slice.try_into().map_err(|_| RemDbError::TypeMismatch)?,
        ))
    }

    /// Read a `Float64` field from a specific row and column.
    pub fn get_field_f64(&self, row: usize, col: usize) -> CoreResult<f64, RemDbError> {
        let col_info = self.columns.get(col).ok_or(RemDbError::FieldNotFound)?;
        if col_info.data_type != DataType::Float64 {
            return Err(RemDbError::TypeMismatch);
        }
        let slice = self.get_col_slice(row, col)?;
        Ok(f64::from_le_bytes(
            slice.try_into().map_err(|_| RemDbError::TypeMismatch)?,
        ))
    }

    /// Read a `Bool` field from a specific row and column.
    pub fn get_field_bool(&self, row: usize, col: usize) -> CoreResult<bool, RemDbError> {
        let col_info = self.columns.get(col).ok_or(RemDbError::FieldNotFound)?;
        if col_info.data_type != DataType::Bool {
            return Err(RemDbError::TypeMismatch);
        }
        let slice = self.get_col_slice(row, col)?;
        slice.first().map(|&b| b != 0).ok_or(RemDbError::FieldNotFound)
    }

    /// Read a `String` field from a specific row and column (zero-copy borrow).
    pub fn get_field_str(&self, row: usize, col: usize) -> CoreResult<&str, RemDbError> {
        let col_info = self.columns.get(col).ok_or(RemDbError::FieldNotFound)?;
        if col_info.data_type != DataType::String {
            return Err(RemDbError::TypeMismatch);
        }
        let slice = self.get_col_slice(row, col)?;
        let end = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
        let trimmed = &slice[..end];
        core::str::from_utf8(trimmed).map_err(|_| RemDbError::TypeMismatch)
    }

    /// Raw byte access (borrows from `CompactResultSet`'s own storage).
    pub fn get_field_raw(&self, row: usize, col: usize) -> CoreResult<&[u8], RemDbError> {
        self.get_col_slice(row, col)
    }

    /// Backwards-compatible: create `TypedValue` on demand.
    pub fn get_field_typed(&self, row: usize, col: usize) -> CoreResult<TypedValue, RemDbError> {
        let col_info = self.columns.get(col).ok_or(RemDbError::FieldNotFound)?;
        let value = self.get_field_value(row, col)?;
        Ok(TypedValue {
            value_type: col_info.data_type,
            value,
        })
    }

    /// Helper: read a field as `Value` enum from the raw data.
    fn get_field_value(&self, row: usize, col: usize) -> CoreResult<Value, RemDbError> {
        let col_info = self.columns.get(col).ok_or(RemDbError::FieldNotFound)?;
        let slice = self.get_col_slice(row, col)?;
        match col_info.data_type {
            DataType::UInt8 => Ok(Value::U8(
                slice.first().copied().ok_or(RemDbError::FieldNotFound)?,
            )),
            DataType::UInt16 => Ok(Value::U16(u16::from_le_bytes(
                slice.try_into().map_err(|_| RemDbError::TypeMismatch)?,
            ))),
            DataType::UInt32 => Ok(Value::U32(u32::from_le_bytes(
                slice.try_into().map_err(|_| RemDbError::TypeMismatch)?,
            ))),
            DataType::UInt64 => Ok(Value::U64(u64::from_le_bytes(
                slice.try_into().map_err(|_| RemDbError::TypeMismatch)?,
            ))),
            DataType::Int8 => Ok(Value::I8(
                slice.first().map(|&b| b as i8).ok_or(RemDbError::FieldNotFound)?,
            )),
            DataType::Int16 => Ok(Value::I16(i16::from_le_bytes(
                slice.try_into().map_err(|_| RemDbError::TypeMismatch)?,
            ))),
            DataType::Int32 => Ok(Value::I32(i32::from_le_bytes(
                slice.try_into().map_err(|_| RemDbError::TypeMismatch)?,
            ))),
            DataType::Int64 => Ok(Value::I64(i64::from_le_bytes(
                slice.try_into().map_err(|_| RemDbError::TypeMismatch)?,
            ))),
            DataType::Float32 => Ok(Value::Float32(f32::from_le_bytes(
                slice.try_into().map_err(|_| RemDbError::TypeMismatch)?,
            ))),
            DataType::Float64 => Ok(Value::Float64(f64::from_le_bytes(
                slice.try_into().map_err(|_| RemDbError::TypeMismatch)?,
            ))),
            DataType::Bool => Ok(Value::Bool(
                slice.first().map(|&b| b != 0).ok_or(RemDbError::FieldNotFound)?,
            )),
            DataType::String => {
                let mut buf = [0u8; MAX_STRING_LEN];
                let copy_size = core::cmp::min(slice.len(), MAX_STRING_LEN);
                buf[..copy_size].copy_from_slice(&slice[..copy_size]);
                Ok(Value::String(buf))
            }
            DataType::Timestamp | DataType::TimestampTZ => {
                let value_bytes: &[u8; 8] = slice
                    .get(..8)
                    .and_then(|s| s.try_into().ok())
                    .ok_or(RemDbError::TypeMismatch)?;
                let value = i64::from_le_bytes(*value_bytes);
                Ok(Value::Time(crate::types::db_timestamp {
                    value,
                    tz_offset: 0,
                    precision: 0,
                    flags: 0,
                }))
            }
            DataType::Interval => {
                let value_bytes: &[u8; 8] = slice
                    .get(..8)
                    .and_then(|s| s.try_into().ok())
                    .ok_or(RemDbError::TypeMismatch)?;
                let value = i64::from_le_bytes(*value_bytes);
                Ok(Value::Interval(crate::types::db_interval {
                    value,
                    precision: 0,
                    flags: 0,
                }))
            }
        }
    }

    /// Backwards-compatible: return a `Vec<TypedValue>` for a row.
    pub fn get_row(&self, row: usize) -> CoreResult<Vec<TypedValue>, RemDbError> {
        let mut result = Vec::with_capacity(self.columns.len());
        for col in 0..self.columns.len() {
            result.push(self.get_field_typed(row, col)?);
        }
        Ok(result)
    }

    /// Number of columns in the result set.
    pub fn column_count(&self) -> usize {
        self.columns.len()
    }

    /// Number of rows in the result set.
    pub fn row_count(&self) -> usize {
        self.record_count
    }

    /// Column metadata.
    pub fn columns(&self) -> &[ColumnInfo] {
        &self.columns
    }

    /// Format the result set as a string (same format as the old `ResultSet`).
    pub fn to_string(&self) -> String {
        if self.record_count == 0 {
            return "Empty result set".to_string();
        }
        let mut result = String::new();
        for (i, col) in self.columns.iter().enumerate() {
            if i > 0 {
                result.push_str(", ");
            }
            result.push_str(&col.name);
        }
        result.push('\n');
        for (i, _) in self.columns.iter().enumerate() {
            if i > 0 {
                result.push_str("--+");
            }
            result.push_str("----");
        }
        result.push('\n');
        for row in 0..self.record_count {
            for col in 0..self.columns.len() {
                if col > 0 {
                    result.push_str(", ");
                }
                if let Ok(tv) = self.get_field_typed(row, col) {
                    result.push_str(&value_to_string_repr(&tv));
                }
            }
            result.push('\n');
        }
        result
    }
}

// ============================================================================
// Legacy ResultSet (deprecated, kept for backwards compatibility)
// ============================================================================

/// 查询结果集
pub struct ResultSet {
    /// 结果集中的列名
    pub columns: Vec<String>,
    /// 结果集中的行数据
    pub rows: Vec<ResultRow>,
    /// 当前行索引
    current_row: usize,
}

impl ResultSet {
    /// 创建新的结果集
    pub fn new(columns: Vec<String>) -> Self {
        ResultSet {
            columns,
            rows: Vec::new(),
            current_row: 0,
        }
    }

    /// 添加一行数据
    pub fn add_row(&mut self, values: Vec<TypedValue>) {
        self.rows.push(ResultRow::new(values));
    }

    /// 获取结果集的列数
    pub fn column_count(&self) -> usize {
        self.columns.len()
    }

    /// 获取结果集的行数
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// 获取列名列表
    pub fn columns(&self) -> &Vec<String> {
        &self.columns
    }

    /// 获取指定行
    pub fn get_row(&self, index: usize) -> Option<&ResultRow> {
        self.rows.get(index)
    }

    /// 获取指定行（可变）
    pub fn get_row_mut(&mut self, index: usize) -> Option<&mut ResultRow> {
        self.rows.get_mut(index)
    }

    /// 重置结果集迭代器
    pub fn reset_iterator(&mut self) {
        self.current_row = 0;
    }

    /// 获取下一行
    pub fn next_row(&mut self) -> Option<&ResultRow> {
        if self.current_row < self.rows.len() {
            let row = &self.rows[self.current_row];
            self.current_row += 1;
            Some(row)
        } else {
            None
        }
    }

    /// 获取结果集迭代器
    pub fn iter(&self) -> ResultRowIter<'_> {
        ResultRowIter {
            result_set: self,
            current: 0,
        }
    }

    /// 将结果集转换为字符串表示
    pub fn to_string(&self) -> String {
        if self.rows.is_empty() {
            return "Empty result set".to_string();
        }

        let mut result = String::new();

        // 添加列名
        for (i, column) in self.columns.iter().enumerate() {
            if i > 0 {
                result.push_str(", ");
            }
            result.push_str(column);
        }
        result.push_str("\n");

        // 添加分隔线
        for (i, _) in self.columns.iter().enumerate() {
            if i > 0 {
                result.push_str("--+");
            }
            result.push_str("----");
        }
        result.push_str("\n");

        // 添加行数据
        for row in &self.rows {
            for (i, value) in row.values.iter().enumerate() {
                if i > 0 {
                    result.push_str(", ");
                }
                result.push_str(&value_to_string_repr(value));
            }
            result.push_str("\n");
        }

        result
    }
}

/// 结果行
pub struct ResultRow {
    /// 行中的值
    pub values: Vec<TypedValue>,
}

impl ResultRow {
    /// 创建新的结果行
    pub fn new(values: Vec<TypedValue>) -> Self {
        ResultRow { values }
    }

    /// 获取字段值
    pub fn get(&self, index: usize) -> CoreResult<&TypedValue, RemDbError> {
        self.values.get(index).ok_or(RemDbError::FieldNotFound)
    }

    /// 通过列名获取字段值
    pub fn get_by_name(&self, columns: &[String], column_name: &str) -> CoreResult<&TypedValue, RemDbError> {
        if let Some(index) = columns.iter().position(|col| col == column_name) {
            self.get(index)
        } else {
            Err(RemDbError::FieldNotFound)
        }
    }

    /// 获取值的数量
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// 检查是否为空行
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// 结果行迭代器
pub struct ResultRowIter<'a> {
    /// 所属的结果集
    result_set: &'a ResultSet,
    /// 当前迭代位置
    current: usize,
}

impl<'a> Iterator for ResultRowIter<'a> {
    type Item = &'a ResultRow;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current < self.result_set.rows.len() {
            let row = &self.result_set.rows[self.current];
            self.current += 1;
            Some(row)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.result_set.rows.len() - self.current;
        (remaining, Some(remaining))
    }
}

impl<'a> ExactSizeIterator for ResultRowIter<'a> {
    fn len(&self) -> usize {
        self.result_set.rows.len() - self.current
    }
}

/// 将字符串转换为结果集列名
pub fn string_to_columns(s: &str) -> Vec<String> {
    s.split(",")
        .map(|col| col.trim().to_string())
        .filter(|col: &String| !col.is_empty())
        .collect()
}

/// 将TypedValue转换为字符串表示
fn value_to_string_repr(value: &TypedValue) -> String {
    unsafe {
        match value.value_type {
            DataType::UInt8 => alloc::format!("{}", value.value.as_u8()),
            DataType::UInt16 => alloc::format!("{}", value.value.as_u16()),
            DataType::UInt32 => alloc::format!("{}", value.value.as_u32()),
            DataType::UInt64 => alloc::format!("{}", value.value.as_u64()),
            DataType::Int8 => alloc::format!("{}", value.value.as_i8()),
            DataType::Int16 => alloc::format!("{}", value.value.as_i16()),
            DataType::Int32 => alloc::format!("{}", value.value.as_i32()),
            DataType::Int64 => alloc::format!("{}", value.value.as_i64()),
            DataType::Float32 => alloc::format!("{}", value.value.as_float32()),
            DataType::Float64 => alloc::format!("{}", value.value.as_float64()),
            DataType::Bool => alloc::format!("{}", value.value.as_bool()),
            DataType::Timestamp => alloc::format!("{}", value.value.as_time().value),
            DataType::TimestampTZ => alloc::format!("{}", value.value.as_time().value),
            DataType::String => {
                let string_slice = core::str::from_utf8(value.value.as_string()).unwrap_or("");
                string_slice.trim_end_matches(char::from(0)).to_string()
            },
            DataType::Interval => {
                alloc::format!("{}", value.value.as_interval().value)
            },
        }
    }
}

/// 将值列表转换为字符串
pub fn values_to_string(values: &[TypedValue]) -> String {
    let mut result = String::new();
    for (i, value) in values.iter().enumerate() {
        if i > 0 {
            result.push_str(", ");
        }
        result.push_str(&value_to_string_repr(value));
    }
    result
}
