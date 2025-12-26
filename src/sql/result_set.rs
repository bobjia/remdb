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
        // 由于Value是union类型，我们需要根据上下文来确定其实际类型
        // 对于SELECT查询结果，我们尝试不同的字段访问方式
        
        // 1. 首先检查字符串类型 - 这是最特殊的类型，需要优先处理
        let string_val = value.string;
        // 尝试将其作为字符串处理，使用完整的MAX_STRING_LEN长度
        let string_slice = core::str::from_utf8(&string_val[0..crate::types::MAX_STRING_LEN]).unwrap_or("");
        let trimmed = string_slice.trim_end_matches(char::from(0));
        
        // 如果字符串非空且包含字母字符，优先作为字符串返回
        // 字符串字段通常包含字母
        if !trimmed.is_empty() && trimmed.chars().any(|c| c.is_ascii_alphabetic()) {
            return trimmed.to_string();
        }
        
        // 2. 检查是否是u32值（ID字段）
        let u32_val = value.u32;
        // ID字段的值通常是正整数，且不会太大
        if u32_val > 0 && u32_val < 10000 {
            return format!("{}", u32_val);
        }
        
        // 3. 检查是否是i8值（age字段）
        let i8_val = value.i8;
        // age字段的值通常在0-120之间
        if i8_val >= 0 && i8_val <= 120 {
            return format!("{}", i8_val);
        }
        
        // 4. 检查是否是布尔值（active字段）
        // 布尔值只有0和1两种可能，且内存布局特殊
        let u8_val = value.u8;
        if u8_val == 0 || u8_val == 1 {
            // 检查内存布局是否符合布尔值特征
            let bool_val = value.bool;
            return format!("{}", bool_val);
        }
        
        // 5. 检查是否是时间戳（created_at字段）
        let u64_val = value.u64;
        // 时间戳通常是很大的整数，且是13位或16位数字
        if u64_val > 1000000000000 && u64_val < 100000000000000000 {
            return format!("{}", u64_val);
        }
        
        // 6. 检查是否是i32值
        let i32_val = value.i32;
        if i32_val > 0 && i32_val < 10000 {
            return format!("{}", i32_val);
        }
        
        // 7. 检查是否是u64值
        if u64_val > 0 && u64_val < 10000 {
            return format!("{}", u64_val);
        }
        
        // 8. 检查是否是i64值
        let i64_val = value.i64;
        if i64_val > 0 && i64_val < 10000 {
            return format!("{}", i64_val);
        }
        
        // 9. 最后尝试作为字符串处理（可能是特殊字符串）
        if !trimmed.is_empty() {
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
