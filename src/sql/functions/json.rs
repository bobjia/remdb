//! SQL JSON Functions
//!
//! This module contains JSON-related function implementations like JSON_EXTRACT, JSON_VALUE, JSON_QUERY, JSON_SET, etc.

use crate::types::{DataType, TypedValue, JsonStorage};
use crate::Value;
use crate::sql::QueryExecutionError;
use crate::MAX_STRING_LEN;

/// 从TypedValue中提取JSON字符串
fn typed_value_to_json_string(arg: &TypedValue) -> Result<String, QueryExecutionError> {
    match arg.value_type {
        DataType::Json => {
            let json_storage = unsafe { &arg.value.json_storage };
            match json_storage {
                JsonStorage::Inline(data) => {
                    let len = data.iter().position(|&b| b == 0).unwrap_or(256);
                    let result = String::from_utf8_lossy(&data[..len]).to_string();
                    Ok(result)
                }
                JsonStorage::External { pool_id, offset, length } => {
                    let pool_manager = crate::json::memory_pool::get_global_json_pool_manager()
                        .ok_or(QueryExecutionError::InternalError)?;
                    let pool = pool_manager.get_pool(*pool_id)
                        .ok_or(QueryExecutionError::InternalError)?;

                    if let Some(data_ptr) = pool.get_block_data(*offset as usize, 0) {
                        let data = unsafe { core::slice::from_raw_parts(data_ptr, *length as usize) };
                        Ok(String::from_utf8_lossy(data).to_string())
                    } else {
                        Err(QueryExecutionError::InternalError)
                    }
                }
                JsonStorage::Null => Ok("null".to_string()),
            }
        }
        _ => Err(QueryExecutionError::TypeMismatch),
    }
}

/// 从TypedValue中提取字符串
fn typed_value_to_string(arg: &TypedValue) -> Result<String, QueryExecutionError> {
    match arg.value_type {
        DataType::VarChar | DataType::Char | DataType::Text => {
            let data = unsafe { &arg.value.string };
            let len = data.iter().position(|&b| b == 0).unwrap_or(MAX_STRING_LEN);
            Ok(String::from_utf8_lossy(&data[..len]).to_string())
        }
        DataType::Int8 => {
            Ok(unsafe { arg.value.i8 }.to_string())
        }
        DataType::Int16 => {
            Ok(unsafe { arg.value.i16 }.to_string())
        }
        DataType::Int32 => {
            Ok(unsafe { arg.value.i32 }.to_string())
        }
        DataType::Int64 => {
            Ok(unsafe { arg.value.i64 }.to_string())
        }
        DataType::UInt8 => {
            Ok(unsafe { arg.value.u8 }.to_string())
        }
        DataType::UInt16 => {
            Ok(unsafe { arg.value.u16 }.to_string())
        }
        DataType::UInt32 => {
            Ok(unsafe { arg.value.u32 }.to_string())
        }
        DataType::UInt64 => {
            Ok(unsafe { arg.value.u64 }.to_string())
        }
        DataType::Float32 => {
            Ok(unsafe { arg.value.float32 }.to_string())
        }
        DataType::Float64 => {
            Ok(unsafe { arg.value.float64 }.to_string())
        }
        DataType::Bool => {
            Ok(unsafe { arg.value.bool }.to_string())
        }
        DataType::Json => {
            typed_value_to_json_string(arg)
        }
        _ => Err(QueryExecutionError::TypeMismatch),
    }
}

/// 执行JSON_EXTRACT函数
pub fn execute_json_extract(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.len() < 2 {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let json_str = typed_value_to_json_string(&args[0])?;
    let path = typed_value_to_string(&args[1])?;

    let doc = crate::json::document::JsonDocument::from_json(&json_str)
        .map_err(|_| QueryExecutionError::InvalidValue)?;

    match crate::json::document::json_extract(&doc, &path) {
        crate::json::document::JsonQueryResult::Scalar(s) => {
            // Try to parse as different types to enable comparisons
            if let Ok(num) = s.parse::<i64>() {
                Ok(TypedValue {
                    value_type: DataType::Int64,
                    value: Value { i64: num },
                })
            } else if let Ok(num) = s.parse::<f64>() {
                Ok(TypedValue {
                    value_type: DataType::Float64,
                    value: Value { float64: num },
                })
            } else if s == "true" || s == "false" {
                Ok(TypedValue {
                    value_type: DataType::Bool,
                    value: Value { bool: s == "true" },
                })
            } else {
                // Default to string
                let mut buf = [0; MAX_STRING_LEN];
                let len = core::cmp::min(s.len(), MAX_STRING_LEN);
                buf[..len].copy_from_slice(s.as_bytes());
                Ok(TypedValue {
                    value_type: DataType::VarChar,
                    value: Value { string: buf },
                })
            }
        }
        crate::json::document::JsonQueryResult::Object(_) |
        crate::json::document::JsonQueryResult::Array(_) => {
            let result_json = match crate::json::document::json_extract(&doc, &path) {
                crate::json::document::JsonQueryResult::Object(obj_doc) => {
                    obj_doc.to_json().unwrap_or_else(|_| "null".to_string())
                }
                crate::json::document::JsonQueryResult::Array(arr) => {
                    let json_str = arr.iter()
                        .map(|item| match item {
                            crate::json::document::JsonQueryResult::Scalar(s) => s.clone(),
                            _ => "null".to_string(),
                        })
                        .collect::<Vec<_>>()
                        .join(",");
                    format!("[{}]", json_str)
                }
                _ => "null".to_string(),
            };

            let mut buf = [0u8; 256];
            let len = core::cmp::min(result_json.len(), 256);
            buf[..len].copy_from_slice(result_json.as_bytes());
            Ok(TypedValue {
                value_type: DataType::Json,
                value: Value { json_storage: JsonStorage::Inline(buf) },
            })
        }
        crate::json::document::JsonQueryResult::None => {
            let mut buf = [0u8; 256];
            Ok(TypedValue {
                value_type: DataType::Json,
                value: Value { json_storage: JsonStorage::Inline(buf) },
            })
        }
    }
}

/// 执行JSON_VALUE函数
pub fn execute_json_value(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.len() < 2 {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let json_str = typed_value_to_json_string(&args[0])?;
    let path = typed_value_to_string(&args[1])?;

    let doc = crate::json::document::JsonDocument::from_json(&json_str)
        .map_err(|_| QueryExecutionError::InvalidValue)?;

    match crate::json::document::json_extract(&doc, &path) {
        crate::json::document::JsonQueryResult::Scalar(s) => {
            let mut buf = [0; MAX_STRING_LEN];
            let len = core::cmp::min(s.len(), MAX_STRING_LEN);
            buf[..len].copy_from_slice(s.as_bytes());
            Ok(TypedValue {
                value_type: DataType::VarChar,
                value: Value { string: buf },
            })
        }
        _ => {
            let mut buf = [0; MAX_STRING_LEN];
            Ok(TypedValue {
                value_type: DataType::VarChar,
                value: Value { string: buf },
            })
        }
    }
}

/// 执行JSON_QUERY函数
pub fn execute_json_query(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.len() < 2 {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let json_str = typed_value_to_json_string(&args[0])?;
    let path = typed_value_to_string(&args[1])?;

    let doc = crate::json::document::JsonDocument::from_json(&json_str)
        .map_err(|_| QueryExecutionError::InvalidValue)?;

    match crate::json::document::json_extract(&doc, &path) {
        crate::json::document::JsonQueryResult::Object(obj_doc) => {
            let result_json = obj_doc.to_json()
                .unwrap_or_else(|_| "null".to_string());
            let mut buf = [0u8; 256];
            let len = core::cmp::min(result_json.len(), 256);
            buf[..len].copy_from_slice(result_json.as_bytes());
            Ok(TypedValue {
                value_type: DataType::Json,
                value: Value { json_storage: JsonStorage::Inline(buf) },
            })
        }
        crate::json::document::JsonQueryResult::Array(arr) => {
            let json_str = arr.iter()
                .map(|item| match item {
                    crate::json::document::JsonQueryResult::Scalar(s) => s.clone(),
                    _ => "null".to_string(),
                })
                .collect::<Vec<_>>()
                .join(",");
            let result_json = format!("[{}]", json_str);
            let mut buf = [0u8; 256];
            let len = core::cmp::min(result_json.len(), 256);
            buf[..len].copy_from_slice(result_json.as_bytes());
            Ok(TypedValue {
                value_type: DataType::Json,
                value: Value { json_storage: JsonStorage::Inline(buf) },
            })
        }
        _ => {
            let mut buf = [0u8; 256];
            Ok(TypedValue {
                value_type: DataType::Json,
                value: Value { json_storage: JsonStorage::Inline(buf) },
            })
        }
    }
}

/// 执行JSON_HAS函数
pub fn execute_json_has(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.len() < 2 {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let json_str = typed_value_to_json_string(&args[0])?;
    let path = typed_value_to_string(&args[1])?;

    let doc = crate::json::document::JsonDocument::from_json(&json_str)
        .map_err(|_| QueryExecutionError::InvalidValue)?;

    let has = crate::json::document::json_has(&doc, &path);
    Ok(TypedValue {
        value_type: DataType::Bool,
        value: Value { bool: has },
    })
}

/// 执行JSON_TYPE函数
pub fn execute_json_type(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.len() < 2 {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let json_str = typed_value_to_json_string(&args[0])?;
    let path = typed_value_to_string(&args[1])?;

    let doc = crate::json::document::JsonDocument::from_json(&json_str)
        .map_err(|_| QueryExecutionError::InvalidValue)?;

    let type_str = crate::json::document::json_type(&doc, &path);
    let mut buf = [0; MAX_STRING_LEN];
    let len = core::cmp::min(type_str.len(), MAX_STRING_LEN);
    buf[..len].copy_from_slice(type_str.as_bytes());
    Ok(TypedValue {
        value_type: DataType::VarChar,
        value: Value { string: buf },
    })
}

/// 执行JSON_SET函数
pub fn execute_json_set(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.len() < 3 {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let json_str = typed_value_to_json_string(&args[0])?;
    let path = typed_value_to_string(&args[1])?;

    // Convert the value to proper JSON format based on its type
    let value_json_str = unsafe {
        match args[2].value_type {
            DataType::Json => {
                typed_value_to_json_string(&args[2])?
            }
            DataType::VarChar | DataType::Char | DataType::Text => {
                let data = &args[2].value.string;
                let len = data.iter().rposition(|&b| b == 0).unwrap_or(MAX_STRING_LEN);
                let s = String::from_utf8_lossy(&data[..len]).to_string();
                format!("\"{}\"", s)
            }
            DataType::Int8 => format!("{}", args[2].value.i8),
            DataType::Int16 => format!("{}", args[2].value.i16),
            DataType::Int32 => format!("{}", args[2].value.i32),
            DataType::Int64 => format!("{}", args[2].value.i64),
            DataType::UInt8 => format!("{}", args[2].value.u8),
            DataType::UInt16 => format!("{}", args[2].value.u16),
            DataType::UInt32 => format!("{}", args[2].value.u32),
            DataType::UInt64 => format!("{}", args[2].value.u64),
            DataType::Float32 => format!("{}", args[2].value.float32),
            DataType::Float64 => format!("{}", args[2].value.float64),
            DataType::Bool => {
                if args[2].value.bool { "true".to_string() } else { "false".to_string() }
            }
            _ => "null".to_string(),
        }
    };

    let mut doc = crate::json::document::JsonDocument::from_json(&json_str)
        .map_err(|_| QueryExecutionError::InvalidValue)?;

    crate::json::document::json_set(&mut doc, &path, &value_json_str)
        .map_err(|_| QueryExecutionError::InternalError)?;

    let new_json_str = doc.to_json()
        .map_err(|_| QueryExecutionError::InternalError)?;

    let mut buf = [0u8; 256];
    let len = core::cmp::min(new_json_str.len(), 256);
    buf[..len].copy_from_slice(new_json_str.as_bytes());
    Ok(TypedValue {
        value_type: DataType::Json,
        value: Value { json_storage: JsonStorage::Inline(buf) },
    })
}

/// 执行JSON_REMOVE函数
pub fn execute_json_remove(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.len() < 2 {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let json_str = typed_value_to_json_string(&args[0])?;
    let path = typed_value_to_string(&args[1])?;

    let mut doc = crate::json::document::JsonDocument::from_json(&json_str)
        .map_err(|_| QueryExecutionError::InvalidValue)?;

    crate::json::document::json_remove(&mut doc, &path)
        .map_err(|_| QueryExecutionError::InternalError)?;

    let new_json_str = doc.to_json()
        .map_err(|_| QueryExecutionError::InternalError)?;

    let mut buf = [0u8; 256];
    let len = core::cmp::min(new_json_str.len(), 256);
    buf[..len].copy_from_slice(new_json_str.as_bytes());
    Ok(TypedValue {
        value_type: DataType::Json,
        value: Value { json_storage: JsonStorage::Inline(buf) },
    })
}

/// 执行JSON_MERGE_PATCH函数
pub fn execute_json_merge_patch(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.len() < 2 {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let json_str = typed_value_to_json_string(&args[0])?;

    // Convert the patch to proper JSON format based on its type
    let patch_json_str = unsafe {
        match args[1].value_type {
            DataType::Json => {
                typed_value_to_json_string(&args[1])?
            }
            DataType::VarChar | DataType::Char | DataType::Text => {
                let data = &args[1].value.string;
                let len = data.iter().position(|&b| b == 0).unwrap_or(MAX_STRING_LEN);
                let s = String::from_utf8_lossy(&data[..len]).to_string();
                s
            }
            DataType::Int8 => format!("{}", args[1].value.i8),
            DataType::Int16 => format!("{}", args[1].value.i16),
            DataType::Int32 => format!("{}", args[1].value.i32),
            DataType::Int64 => format!("{}", args[1].value.i64),
            DataType::UInt8 => format!("{}", args[1].value.u8),
            DataType::UInt16 => format!("{}", args[1].value.u16),
            DataType::UInt32 => format!("{}", args[1].value.u32),
            DataType::UInt64 => format!("{}", args[1].value.u64),
            DataType::Float32 => format!("{}", args[1].value.float32),
            DataType::Float64 => format!("{}", args[1].value.float64),
            DataType::Bool => {
                if args[1].value.bool { "true".to_string() } else { "false".to_string() }
            }
            _ => "null".to_string(),
        }
    };

    let mut doc = crate::json::document::JsonDocument::from_json(&json_str)
        .map_err(|_| QueryExecutionError::InvalidValue)?;

    crate::json::document::json_merge_patch(&mut doc, &patch_json_str)
        .map_err(|_| QueryExecutionError::InternalError)?;

    let new_json_str = doc.to_json()
        .map_err(|_| QueryExecutionError::InternalError)?;

    let mut buf = [0u8; 256];
    let len = core::cmp::min(new_json_str.len(), 256);
    buf[..len].copy_from_slice(new_json_str.as_bytes());
    Ok(TypedValue {
        value_type: DataType::Json,
        value: Value { json_storage: JsonStorage::Inline(buf) },
    })
}

/// 执行JSON_ARRAY_APPEND函数
pub fn execute_json_array_append(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.len() < 3 {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let json_str = typed_value_to_json_string(&args[0])?;
    let path = typed_value_to_string(&args[1])?;

    // Convert the value to proper JSON format based on its type
    let value_json_str = unsafe {
        match args[2].value_type {
            DataType::Json => {
                typed_value_to_json_string(&args[2])?
            }
            DataType::VarChar | DataType::Char | DataType::Text => {
                let data = &args[2].value.string;
                let len = data.iter().position(|&b| b == 0).unwrap_or(MAX_STRING_LEN);
                let s = String::from_utf8_lossy(&data[..len]).to_string();
                format!("\"{}\"", s)
            }
            DataType::Int8 => format!("{}", args[2].value.i8),
            DataType::Int16 => format!("{}", args[2].value.i16),
            DataType::Int32 => format!("{}", args[2].value.i32),
            DataType::Int64 => format!("{}", args[2].value.i64),
            DataType::UInt8 => format!("{}", args[2].value.u8),
            DataType::UInt16 => format!("{}", args[2].value.u16),
            DataType::UInt32 => format!("{}", args[2].value.u32),
            DataType::UInt64 => format!("{}", args[2].value.u64),
            DataType::Float32 => format!("{}", args[2].value.float32),
            DataType::Float64 => format!("{}", args[2].value.float64),
            DataType::Bool => {
                if args[2].value.bool { "true".to_string() } else { "false".to_string() }
            }
            _ => "null".to_string(),
        }
    };

    let mut doc = crate::json::document::JsonDocument::from_json(&json_str)
        .map_err(|_| QueryExecutionError::InvalidValue)?;

    crate::json::document::json_set(&mut doc, &path, &value_json_str)
        .map_err(|_| QueryExecutionError::InternalError)?;

    let new_json_str = doc.to_json()
        .map_err(|_| QueryExecutionError::InternalError)?;

    let mut buf = [0u8; 256];
    let len = core::cmp::min(new_json_str.len(), 256);
    buf[..len].copy_from_slice(new_json_str.as_bytes());
    Ok(TypedValue {
        value_type: DataType::Json,
        value: Value { json_storage: JsonStorage::Inline(buf) },
    })
}

/// 执行JSON_ARRAY_LENGTH函数
pub fn execute_json_array_length(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let json_str = typed_value_to_json_string(&args[0])?;

    let doc = crate::json::document::JsonDocument::from_json(&json_str)
        .map_err(|_| QueryExecutionError::InvalidValue)?;

    match crate::json::document::json_extract(&doc, "$") {
        crate::json::document::JsonQueryResult::Array(arr) => {
            let length = arr.len() as u64;
            Ok(TypedValue {
                value_type: DataType::UInt64,
                value: Value { u64: length },
            })
        }
        _ => {
            Ok(TypedValue {
                value_type: DataType::UInt64,
                value: Value { u64: 0 },
            })
        }
    }
}

/// 执行JSON_ARRAY函数
pub fn execute_json_array(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        let mut buf = [0u8; 256];
        let json_str = "[]";
        let len = core::cmp::min(json_str.len(), 256);
        buf[..len].copy_from_slice(json_str.as_bytes());
        return Ok(TypedValue {
            value_type: DataType::Json,
            value: Value { json_storage: JsonStorage::Inline(buf) },
        });
    }

    let mut array_items = Vec::new();
    for arg in args {
        let item_str: String = unsafe {
            match arg.value_type {
                DataType::Json => {
                    typed_value_to_json_string(arg)?
                }
                DataType::VarChar | DataType::Char | DataType::Text => {
                    let data = &arg.value.string;
                    let len = data.iter().position(|&b| b == 0).unwrap_or(MAX_STRING_LEN);
                    let s = String::from_utf8_lossy(&data[..len]).to_string();
                    format!("\"{}\"", s)
                }
                DataType::Int8 => format!("{}", arg.value.i8),
                DataType::Int16 => format!("{}", arg.value.i16),
                DataType::Int32 => format!("{}", arg.value.i32),
                DataType::Int64 => format!("{}", arg.value.i64),
                DataType::UInt8 => format!("{}", arg.value.u8),
                DataType::UInt16 => format!("{}", arg.value.u16),
                DataType::UInt32 => format!("{}", arg.value.u32),
                DataType::UInt64 => format!("{}", arg.value.u64),
                DataType::Float32 => format!("{}", arg.value.float32),
                DataType::Float64 => format!("{}", arg.value.float64),
                DataType::Bool => {
                    if arg.value.bool { "true".to_string() } else { "false".to_string() }
                }
                _ => "null".to_string(),
            }
        };
        array_items.push(item_str);
    }

    let json_str = format!("[{}]", array_items.join(","));
    let mut buf = [0u8; 256];
    let len = core::cmp::min(json_str.len(), 256);
    buf[..len].copy_from_slice(json_str.as_bytes());
    Ok(TypedValue {
        value_type: DataType::Json,
        value: Value { json_storage: JsonStorage::Inline(buf) },
    })
}

/// 执行JSON_OBJECT函数
pub fn execute_json_object(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() || args.len() % 2 != 0 {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let mut object_items = Vec::new();
    for i in (0..args.len()).step_by(2) {
        let key_str = typed_value_to_string(&args[i])?;
        let value_str: String = unsafe {
            match args[i + 1].value_type {
                DataType::Json => {
                    typed_value_to_json_string(&args[i + 1])?
                }
                DataType::VarChar | DataType::Char | DataType::Text => {
                    let data = &args[i + 1].value.string;
                    let len = data.iter().position(|&b| b == 0).unwrap_or(MAX_STRING_LEN);
                    let s = String::from_utf8_lossy(&data[..len]).to_string();
                    format!("\"{}\"", s)
                }
                DataType::Int8 => format!("{}", args[i + 1].value.i8),
                DataType::Int16 => format!("{}", args[i + 1].value.i16),
                DataType::Int32 => format!("{}", args[i + 1].value.i32),
                DataType::Int64 => format!("{}", args[i + 1].value.i64),
                DataType::UInt8 => format!("{}", args[i + 1].value.u8),
                DataType::UInt16 => format!("{}", args[i + 1].value.u16),
                DataType::UInt32 => format!("{}", args[i + 1].value.u32),
                DataType::UInt64 => format!("{}", args[i + 1].value.u64),
                DataType::Float32 => format!("{}", args[i + 1].value.float32),
                DataType::Float64 => format!("{}", args[i + 1].value.float64),
                DataType::Bool => {
                    if args[i + 1].value.bool { "true".to_string() } else { "false".to_string() }
                }
                _ => "null".to_string(),
            }
        };
        object_items.push(format!("\"{}\":{}", key_str, value_str));
    }

    let json_str = format!("{{{}}}", object_items.join(","));
    let mut buf = [0u8; 256];
    let len = core::cmp::min(json_str.len(), 256);
    buf[..len].copy_from_slice(json_str.as_bytes());
    Ok(TypedValue {
        value_type: DataType::Json,
        value: Value { json_storage: JsonStorage::Inline(buf) },
    })
}