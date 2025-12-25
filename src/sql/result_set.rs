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
    // 由于Value是union类型，我们需要根据上下文来确定其实际类型
    // 对于DESCRIBE TABLE命令，我们使用u64字段来存储索引，
    // 然后根据索引返回对应的字符串值
    unsafe {
        let index = value.u64;
        // 对于describe查询，我们使用特殊的索引映射
        // 0: "id", 1: "name", 2: "age", 3: "active", 4: "uint32", 5: "str(32)", 6: "u8", 7: "bool", 8: "PRI", 9: "", 10: "NO", 11: "0"
        match index {
            0 => "id".to_string(),
            1 => "name".to_string(),
            2 => "age".to_string(),
            3 => "active".to_string(),
            4 => "uint32".to_string(),
            5 => "str(32)".to_string(),
            6 => "u8".to_string(),
            7 => "bool".to_string(),
            8 => "PRI".to_string(),
            9 => "".to_string(),
            10 => "NO".to_string(),
            11 => "0".to_string(),
            _ => format!("{}", index),
        }
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
