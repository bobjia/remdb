//! SQL String Functions
//!
//! This module contains string-related function implementations like CONCAT, SUBSTRING, UPPER, LOWER, LENGTH.

use crate::sql::QueryExecutionError;
use crate::types::DataType;
use crate::types::TypedValue;
use crate::Value;
use crate::MAX_STRING_LEN;

/// 执行CONCAT函数
pub fn execute_concat(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    // 连接所有参数为字符串
    let mut result = String::new();

    for arg in args {
        unsafe {
            let arg_str = match arg.value_type {
                DataType::VarChar | DataType::Char | DataType::Text => String::from(
                    core::str::from_utf8(&arg.value.string)
                        .map_err(|_| QueryExecutionError::TypeMismatch)?
                        .trim_end_matches(char::from(0)),
                ),
                DataType::UInt8 => alloc::format!("{}", arg.value.u8),
                DataType::UInt16 => alloc::format!("{}", arg.value.u16),
                DataType::UInt32 => alloc::format!("{}", arg.value.u32),
                DataType::UInt64 => alloc::format!("{}", arg.value.u64),
                DataType::Int8 => alloc::format!("{}", arg.value.i8),
                DataType::Int16 => alloc::format!("{}", arg.value.i16),
                DataType::Int32 => alloc::format!("{}", arg.value.i32),
                DataType::Int64 => alloc::format!("{}", arg.value.i64),
                DataType::Float32 => alloc::format!("{}", arg.value.float32),
                DataType::Float64 => alloc::format!("{}", arg.value.float64),
                DataType::Bool => alloc::format!("{}", arg.value.bool),
                _ => return Err(QueryExecutionError::TypeMismatch),
            };
            result.push_str(&arg_str);
        }
    }

    // 将结果转换为TypedValue
    let mut string_value = [0; MAX_STRING_LEN];
    let len = core::cmp::min(result.len(), MAX_STRING_LEN);
    string_value[..len].copy_from_slice(&result.as_bytes()[..len]);

    Ok(TypedValue {
        value_type: DataType::VarChar,
        value: Value {
            string: string_value,
        },
    })
}

/// 执行SUBSTRING函数
pub fn execute_substring(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.len() < 2 {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let str_arg = &args[0];
    let start_arg = &args[1];
    let length_arg = if args.len() > 2 { Some(&args[2]) } else { None };

    unsafe {
        // 获取字符串
        let str_value = match str_arg.value_type {
            DataType::VarChar | DataType::Char | DataType::Text => {
                core::str::from_utf8(&str_arg.value.string)
                    .map_err(|_| QueryExecutionError::TypeMismatch)?
                    .trim_end_matches(char::from(0))
                    .to_string()
            }
            _ => return Err(QueryExecutionError::TypeMismatch),
        };

        // 获取起始位置 (1-based)
        let start = match start_arg.value_type {
            DataType::UInt8 => start_arg.value.u8 as usize,
            DataType::UInt16 => start_arg.value.u16 as usize,
            DataType::UInt32 => start_arg.value.u32 as usize,
            DataType::UInt64 => start_arg.value.u64 as usize,
            DataType::Int8 => start_arg.value.i8 as usize,
            DataType::Int16 => start_arg.value.i16 as usize,
            DataType::Int32 => start_arg.value.i32 as usize,
            DataType::Int64 => start_arg.value.i64 as usize,
            _ => return Err(QueryExecutionError::TypeMismatch),
        };

        // 获取长度
        let length = if let Some(len_arg) = length_arg {
            match len_arg.value_type {
                DataType::UInt8 => Some(len_arg.value.u8 as usize),
                DataType::UInt16 => Some(len_arg.value.u16 as usize),
                DataType::UInt32 => Some(len_arg.value.u32 as usize),
                DataType::UInt64 => Some(len_arg.value.u64 as usize),
                DataType::Int8 => Some(len_arg.value.i8 as usize),
                DataType::Int16 => Some(len_arg.value.i16 as usize),
                DataType::Int32 => Some(len_arg.value.i32 as usize),
                DataType::Int64 => Some(len_arg.value.i64 as usize),
                _ => return Err(QueryExecutionError::TypeMismatch),
            }
        } else {
            None
        };

        // 执行子字符串提取
        let str_bytes = str_value.as_bytes();

        // 转换为字节位置（1-based -> 0-based）
        let byte_start = if start > 0 { start - 1 } else { 0 };
        if byte_start >= str_bytes.len() {
            return Ok(TypedValue {
                value_type: DataType::VarChar,
                value: Value {
                    string: [0; MAX_STRING_LEN],
                },
            });
        }

        let substring = if let Some(len) = length {
            let byte_end = core::cmp::min(byte_start + len, str_bytes.len());
            String::from_utf8_lossy(&str_bytes[byte_start..byte_end]).to_string()
        } else {
            String::from_utf8_lossy(&str_bytes[byte_start..]).to_string()
        };

        // 将结果转换为TypedValue
        let mut string_value = [0; MAX_STRING_LEN];
        let slen = core::cmp::min(substring.len(), MAX_STRING_LEN);
        string_value[..slen].copy_from_slice(&substring.as_bytes()[..slen]);

        Ok(TypedValue {
            value_type: DataType::VarChar,
            value: Value {
                string: string_value,
            },
        })
    }
}

/// 执行UPPER函数
pub fn execute_upper(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let arg = &args[0];

    unsafe {
        let str_value = match arg.value_type {
            DataType::VarChar | DataType::Char | DataType::Text => {
                core::str::from_utf8(&arg.value.string)
                    .map_err(|_| QueryExecutionError::TypeMismatch)?
                    .trim_end_matches(char::from(0))
                    .to_string()
            }
            _ => return Err(QueryExecutionError::TypeMismatch),
        };

        let result = str_value.to_uppercase();

        // 将结果转换为TypedValue
        let mut string_value = [0; MAX_STRING_LEN];
        let len = core::cmp::min(result.len(), MAX_STRING_LEN);
        string_value[..len].copy_from_slice(&result.as_bytes()[..len]);

        Ok(TypedValue {
            value_type: DataType::VarChar,
            value: Value {
                string: string_value,
            },
        })
    }
}

/// 执行LOWER函数
pub fn execute_lower(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let arg = &args[0];

    unsafe {
        let str_value = match arg.value_type {
            DataType::VarChar | DataType::Char | DataType::Text => {
                core::str::from_utf8(&arg.value.string)
                    .map_err(|_| QueryExecutionError::TypeMismatch)?
                    .trim_end_matches(char::from(0))
                    .to_string()
            }
            _ => return Err(QueryExecutionError::TypeMismatch),
        };

        let result = str_value.to_lowercase();

        // 将结果转换为TypedValue
        let mut string_value = [0; MAX_STRING_LEN];
        let len = core::cmp::min(result.len(), MAX_STRING_LEN);
        string_value[..len].copy_from_slice(&result.as_bytes()[..len]);

        Ok(TypedValue {
            value_type: DataType::VarChar,
            value: Value {
                string: string_value,
            },
        })
    }
}

/// 执行LENGTH函数（字节长度）
pub fn execute_length(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let arg = &args[0];

    unsafe {
        let str_value = match arg.value_type {
            DataType::VarChar | DataType::Char | DataType::Text => {
                core::str::from_utf8(&arg.value.string)
                    .map_err(|_| QueryExecutionError::TypeMismatch)?
                    .trim_end_matches(char::from(0))
                    .to_string()
            }
            _ => return Err(QueryExecutionError::TypeMismatch),
        };

        let len = str_value.len();

        Ok(TypedValue {
            value_type: DataType::Int64,
            value: Value { i64: len as i64 },
        })
    }
}

/// 执行CHAR_LENGTH函数（字符长度，UTF-8感知）
pub fn execute_char_length(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let arg = &args[0];

    unsafe {
        let str_value = match arg.value_type {
            DataType::VarChar | DataType::Char | DataType::Text => {
                core::str::from_utf8(&arg.value.string)
                    .map_err(|_| QueryExecutionError::TypeMismatch)?
                    .trim_end_matches(char::from(0))
                    .to_string()
            }
            _ => return Err(QueryExecutionError::TypeMismatch),
        };

        let len = str_value.chars().count();

        Ok(TypedValue {
            value_type: DataType::Int64,
            value: Value { i64: len as i64 },
        })
    }
}
