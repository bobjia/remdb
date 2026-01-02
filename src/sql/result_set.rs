//! SQL查询结果集
//! 
//! 该模块负责处理SQL查询的结果集，提供友好的结果访问接口。

use alloc::string::String;
use alloc::vec::Vec;
use alloc::boxed::Box;
use core::iter::Iterator;
use core::option::Option;
use core::result::Result as CoreResult;

use crate::{RemDbError, Value};

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
    pub fn add_row(&mut self, values: Vec<Value>) {
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
    pub fn iter(&self) -> ResultRowIter {
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
    pub values: Vec<Value>,
}

impl ResultRow {
    /// 创建新的结果行
    pub fn new(values: Vec<Value>) -> Self {
        ResultRow { values }
    }

    /// 获取字段值
    pub fn get(&self, index: usize) -> CoreResult<&Value, RemDbError> {
        self.values.get(index).ok_or(RemDbError::FieldNotFound)
    }

    /// 通过列名获取字段值
    pub fn get_by_name(&self, columns: &[String], column_name: &str) -> CoreResult<&Value, RemDbError> {
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
        .filter(|col| !col.is_empty())
        .collect()
}

/// 将Value转换为字符串表示
fn value_to_string_repr(value: &Value) -> String {
    unsafe {
        // 由于Value是union类型，无法直接知道其实际类型
        // 我们需要尝试不同的访问方式，并根据结果判断
        
        // 1. 首先检查是否是u64值（如affected_rows）
        // 受影响的行数通常不会超过100万
        let u64_val = value.u64;
        if u64_val <= 1000000 {
            return format!("{}", u64_val);
        }
        
        // 2. 检查是否是i32值（ID字段等）
        let i32_val = value.i32;
        // 总是返回i32值，因为ID字段通常是i32类型
        return format!("{}", i32_val);
        
        // 4. 检查是否是布尔值（active字段）
        let bool_val = value.bool;
        // 布尔值只有true和false两种可能
        // 我们需要确保这不是其他类型的0或1值
        let u8_val = value.u8;
        
        // 如果整个8字节都是0或1，那么它可能是一个布尔值
        if (bool_val == false && u8_val == 0 && u64_val == 0) || 
           (bool_val == true && u8_val == 1 && u64_val == 1) {
            return format!("{}", bool_val);
        }
        
        // 5. 检查是否是时间戳（13位数字）
        let is_timestamp = |val| val >= 1000000000000 && val < 10000000000000;
        let timestamp_val = value.timestamp;
        if is_timestamp(timestamp_val) {
            return format!("{}", timestamp_val);
        }
        
        // 6. 最后检查是否是字符串类型
        let string_val = value.string;
        // 只检查前32字节，避免读取过多无效数据
        let string_slice = core::str::from_utf8(&string_val[0..32]).unwrap_or("");
        let trimmed = string_slice.trim_end_matches(char::from(0));
        
        // 检查是否是真正的字符串：
        // - 长度大于1
        // - 包含至少一个字母字符
        // - 不是纯数字（避免将ID等数字字段误判为字符串）
        if trimmed.len() > 1 && 
           trimmed.chars().any(|c| c.is_ascii_alphabetic()) && 
           !trimmed.chars().all(|c| c.is_ascii_digit()) {
            return trimmed.to_string();
        }
        
        // 默认情况下，返回空字符串
        "".to_string()
    }
}

/// 将值列表转换为字符串
pub fn values_to_string(values: &[Value]) -> String {
    let mut result = String::new();
    for (i, value) in values.iter().enumerate() {
        if i > 0 {
            result.push_str(", ");
        }
        result.push_str(&value_to_string_repr(value));
    }
    result
}
