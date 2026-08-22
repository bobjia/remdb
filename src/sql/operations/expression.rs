//! SQL Expression Evaluation
//!
//! This module contains expression evaluation logic for SQL queries.

use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;

use crate::sql::query_parser::{
    BinaryOperator, Expression, LogicalOperator, UnaryOperator,
};
use crate::sql::{QueryExecutionError};
use crate::types::{DataType, TypedValue, JsonStorage};
use crate::{MemoryTable, Value, MAX_STRING_LEN, RemDb};
use crate::sql::functions as sql_functions;
use crate::sql::operations::vector::{
    calculate_vector_l2_distance, calculate_vector_inner_product,
    calculate_vector_cosine_similarity,
};

const MAX_RECURSION_DEPTH: usize = 100;

pub fn evaluate_unary_op(
    op: UnaryOperator,
    operand: TypedValue,
) -> Result<TypedValue, QueryExecutionError> {
    match op {
        UnaryOperator::Not => {
            unsafe {
                let bool_value = match operand.value_type {
                    DataType::Bool => operand.value.bool,
                    DataType::Int8 => operand.value.i8 != 0,
                    DataType::Int16 => operand.value.i16 != 0,
                    DataType::Int32 => operand.value.i32 != 0,
                    DataType::Int64 => operand.value.i64 != 0,
                    DataType::UInt8 => operand.value.u8 != 0,
                    DataType::UInt16 => operand.value.u16 != 0,
                    DataType::UInt32 => operand.value.u32 != 0,
                    DataType::UInt64 => operand.value.u64 != 0,
                    DataType::Float32 => operand.value.float32 != 0.0,
                    DataType::Float64 => operand.value.float64 != 0.0,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };
                Ok(TypedValue {
                    value_type: DataType::Bool,
                    value: Value { bool: !bool_value },
                })
            }
        }
        UnaryOperator::Minus => {
            unsafe {
                match operand.value_type {
                    DataType::Int8 => Ok(TypedValue {
                        value_type: DataType::Int8,
                        value: Value { i8: -operand.value.i8 },
                    }),
                    DataType::Int16 => Ok(TypedValue {
                        value_type: DataType::Int16,
                        value: Value { i16: -operand.value.i16 },
                    }),
                    DataType::Int32 => Ok(TypedValue {
                        value_type: DataType::Int32,
                        value: Value { i32: -operand.value.i32 },
                    }),
                    DataType::Int64 => Ok(TypedValue {
                        value_type: DataType::Int64,
                        value: Value { i64: -operand.value.i64 },
                    }),
                    DataType::Float32 => Ok(TypedValue {
                        value_type: DataType::Float32,
                        value: Value { float32: -operand.value.float32 },
                    }),
                    DataType::Float64 => Ok(TypedValue {
                        value_type: DataType::Float64,
                        value: Value { float64: -operand.value.float64 },
                    }),
                    _ => Err(QueryExecutionError::TypeMismatch),
                }
            }
        }
        UnaryOperator::Plus => {
            Ok(operand)
        }
    }
}

pub fn evaluate_expression(
    table: &MemoryTable,
    record_values: &[TypedValue],
    expr: &Expression,
) -> Result<TypedValue, QueryExecutionError> {
    evaluate_expression_with_depth(table, record_values, expr, 0)
}

pub fn evaluate_expression_with_depth(
    table: &MemoryTable,
    record_values: &[TypedValue],
    expr: &Expression,
    depth: usize,
) -> Result<TypedValue, QueryExecutionError> {
    if depth > MAX_RECURSION_DEPTH {
        return Err(QueryExecutionError::InternalError);
    }
    match expr {
        Expression::Field {
            name: field_name, ..
        } => {
            if field_name == "*" {
                Ok(record_values[0].clone())
            } else {
                let actual_field_name = if field_name.contains('.') {
                    field_name.split('.').last().expect("field name must contain '.'")
                } else {
                    field_name
                };

                let field_index = table
                    .def
                    .fields
                    .iter()
                    .position(|field| field.name == *actual_field_name)
                    .ok_or(QueryExecutionError::FieldNotFound)?;

                Ok(record_values[field_index].clone())
            }
        }
        Expression::FunctionCall { name, args, .. } => {
            let mut arg_values = Vec::with_capacity(args.len());
            for arg in args {
                arg_values.push(evaluate_expression_with_depth(table, record_values, arg, depth + 1)?);
            }

            #[cfg(feature = "log")]
            crate::log::debug!("evaluate_expression: calling execute_function_call with name={}, args.len={}", name, arg_values.len());
            let result = execute_function_call(name, &arg_values);
            result
        }
        Expression::Constant {
            value: constant, ..
        } => {
            use crate::sql::Value as SqlValue;

            let (value_type, value) = match constant {
                SqlValue::Integer(i) => (DataType::Int64, Value { i64: *i }),
                SqlValue::Float(f) => (DataType::Float64, Value { float64: *f }),
                SqlValue::String(s) => {
                    let mut buf = [0; MAX_STRING_LEN];
                    let len = core::cmp::min(s.len(), MAX_STRING_LEN);
                    buf[..len].copy_from_slice(&s.as_bytes()[..len]);
                    (DataType::VarChar, Value { string: buf })
                }
                SqlValue::Boolean(b) => (DataType::Bool, Value { bool: *b }),
                SqlValue::Null => (DataType::Json, Value { json_storage: crate::types::JsonStorage::Null }),
                SqlValue::Identifier(s) => {
                    let mut buf = [0; MAX_STRING_LEN];
                    let len = core::cmp::min(s.len(), MAX_STRING_LEN);
                    buf[..len].copy_from_slice(&s.as_bytes()[..len]);
                    (DataType::VarChar, Value { string: buf })
                }
                SqlValue::Json(json_str) => {
                    let mut buf = [0u8; 256];
                    let len = core::cmp::min(json_str.len(), 256);
                    buf[..len].copy_from_slice(&json_str.as_bytes()[..len]);
                    let json_storage = if json_str.len() <= 256 {
                        crate::types::JsonStorage::Inline(buf)
                    } else {
                        crate::types::JsonStorage::Null
                    };
                    (DataType::Json, Value { json_storage })
                }
            };

            Ok(TypedValue { value_type, value })
        }
        Expression::BinaryOp {
            left, op, right, ..
        } => {
            let left_val = evaluate_expression_with_depth(table, record_values, left, depth + 1)?;
            let right_val = evaluate_expression_with_depth(table, record_values, right, depth + 1)?;

            if matches!(
                *op,
                BinaryOperator::VectorL2 | BinaryOperator::VectorIP | BinaryOperator::VectorCosine
            ) {
                if matches!(left_val.value_type, DataType::Vector) {
                    let vector_field = if let Expression::Field {
                        name: ref field_name,
                        ..
                    } = **left
                    {
                        table
                            .def
                            .fields
                            .iter()
                            .find(|field| field.name == *field_name)
                            .ok_or(QueryExecutionError::FieldNotFound)?
                    } else {
                        table
                            .def
                            .fields
                            .iter()
                            .find(|field| field.vector_metadata.is_some())
                            .ok_or(QueryExecutionError::TypeMismatch)?
                    };

                    let vector_dim = vector_field
                        .vector_metadata
                        .ok_or(QueryExecutionError::TypeMismatch)?
                        .dimension;

                    // Handle the case where the right operand is a Json with Null storage
                    // (the vector string was too large for the inline buffer)
                    if matches!(right_val.value_type, DataType::Json) {
                        if let crate::types::JsonStorage::Null = unsafe { right_val.value.json_storage } {
                            // Try to extract the original string from the Constant expression
                            if let Expression::Constant { value: crate::sql::Value::Json(json_str), .. } = right.as_ref() {
                                // Parse the vector directly from the string
                                let trimmed = json_str.trim();
                                if trimmed.starts_with('[') && trimmed.ends_with(']') {
                                    let inner = &trimmed[1..trimmed.len()-1];
                                    let elements: Vec<&str> = inner.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
                                    if elements.len() == vector_dim as usize {
                                        let vec2_values: Vec<f32> = elements.iter()
                                            .map(|s| s.parse::<f32>())
                                            .collect::<Result<Vec<_>, _>>()
                                            .map_err(|_| QueryExecutionError::TypeMismatch)?;
                                        let vec2_f64: Vec<f64> = vec2_values.iter().map(|v| *v as f64).collect();
                                        let result = match *op {
                                            BinaryOperator::VectorL2 => unsafe {
                                                calculate_vector_l2_distance(left_val.value.vector, &vec2_f64, vector_dim)
                                            },
                                            BinaryOperator::VectorIP => unsafe {
                                                calculate_vector_inner_product(left_val.value.vector, &vec2_f64, vector_dim)
                                            },
                                            BinaryOperator::VectorCosine => unsafe {
                                                calculate_vector_cosine_similarity(left_val.value.vector, &vec2_f64, vector_dim)
                                            },
                                            _ => return Err(QueryExecutionError::TypeMismatch),
                                        };
                                        return Ok(TypedValue {
                                            value_type: DataType::Float64,
                                            value: Value { float64: result },
                                        });
                                    }
                                }
                            }
                            return Err(QueryExecutionError::TypeMismatch);
                        }
                    }

                    return evaluate_vector_binary_op(left_val, *op, right_val, vector_dim);
                }
            }

            evaluate_binary_op(left_val, *op, right_val)
        }
        Expression::LogicalOp {
            left,
            op,
            right,
            ..
        } => {
            let left_val = evaluate_expression_with_depth(table, record_values, left, depth + 1)?;
            let right_val = evaluate_expression_with_depth(table, record_values, right, depth + 1)?;

            let left_bool = unsafe {
                match left_val.value_type {
                    DataType::Bool => left_val.value.bool,
                    DataType::Int8 => left_val.value.i8 != 0,
                    DataType::Int16 => left_val.value.i16 != 0,
                    DataType::Int32 => left_val.value.i32 != 0,
                    DataType::Int64 => left_val.value.i64 != 0,
                    DataType::UInt8 => left_val.value.u8 != 0,
                    DataType::UInt16 => left_val.value.u16 != 0,
                    DataType::UInt32 => left_val.value.u32 != 0,
                    DataType::UInt64 => left_val.value.u64 != 0,
                    DataType::Float32 => left_val.value.float32 != 0.0,
                    DataType::Float64 => left_val.value.float64 != 0.0,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                }
            };

            let right_bool = unsafe {
                match right_val.value_type {
                    DataType::Bool => right_val.value.bool,
                    DataType::Int8 => right_val.value.i8 != 0,
                    DataType::Int16 => right_val.value.i16 != 0,
                    DataType::Int32 => right_val.value.i32 != 0,
                    DataType::Int64 => right_val.value.i64 != 0,
                    DataType::UInt8 => right_val.value.u8 != 0,
                    DataType::UInt16 => right_val.value.u16 != 0,
                    DataType::UInt32 => right_val.value.u32 != 0,
                    DataType::UInt64 => right_val.value.u64 != 0,
                    DataType::Float32 => right_val.value.float32 != 0.0,
                    DataType::Float64 => right_val.value.float64 != 0.0,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                }
            };

            let result = match op {
                LogicalOperator::And => left_bool && right_bool,
                LogicalOperator::Or => left_bool || right_bool,
            };

            Ok(TypedValue {
                value_type: DataType::Bool,
                value: Value { bool: result },
            })
        }
        Expression::UnaryOp {
            op,
            operand,
            ..
        } => {
            let operand_val = evaluate_expression_with_depth(table, record_values, operand, depth + 1)?;
            evaluate_unary_op(*op, operand_val)
        }
    }
}

pub fn evaluate_vector_binary_op(
    left: TypedValue,
    op: BinaryOperator,
    right: TypedValue,
    vector_dim: u16,
) -> Result<TypedValue, QueryExecutionError> {
    if !matches!(left.value_type, DataType::Vector) {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let vec1_ptr = unsafe {
        match left.value_type {
            DataType::Vector => left.value.vector,
            _ => core::ptr::null(),
        }
    };
    if vec1_ptr.is_null() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let vec2_values: Vec<f32>;

    if matches!(right.value_type, DataType::Vector) {
        let vec2_ptr = unsafe { right.value.vector };
        if vec2_ptr.is_null() {
            return Err(QueryExecutionError::TypeMismatch);
        }

        vec2_values = unsafe {
            let vec_slice = core::slice::from_raw_parts(vec2_ptr, vector_dim as usize);
            vec_slice.to_vec()
        };
    } else if matches!(right.value_type, DataType::VarChar | DataType::Char | DataType::Text) {
        let vec_str = unsafe {
            core::str::from_utf8(&right.value.string)
                .map_err(|_| QueryExecutionError::TypeMismatch)?
                .trim_end_matches(char::from(0))
        };

        if !vec_str.starts_with('[') || !vec_str.ends_with(']') {
            return Err(QueryExecutionError::TypeMismatch);
        }

        let vec_str = vec_str.trim_start_matches('[').trim_end_matches(']');

        let elements: Vec<&str> = vec_str
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        if elements.len() != vector_dim as usize {
            return Err(QueryExecutionError::TypeMismatch);
        }

        vec2_values = elements
            .iter()
            .map(|s| s.parse::<f32>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| QueryExecutionError::TypeMismatch)?;
    } else if matches!(right.value_type, DataType::Json) {
        if let JsonStorage::Inline(json_bytes) = unsafe { right.value.json_storage } {
            let vec_str = core::str::from_utf8(&json_bytes)
                .map_err(|_| QueryExecutionError::TypeMismatch)?
                .trim_end_matches('\0');

            if !vec_str.starts_with('[') || !vec_str.ends_with(']') {
                return Err(QueryExecutionError::TypeMismatch);
            }

            let vec_str = vec_str.trim_start_matches('[').trim_end_matches(']');
            let elements: Vec<&str> = vec_str
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();

            if elements.len() != vector_dim as usize {
                return Err(QueryExecutionError::TypeMismatch);
            }

            vec2_values = elements
                .iter()
                .map(|s| s.parse::<f32>())
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| QueryExecutionError::TypeMismatch)?;
        } else {
            return Err(QueryExecutionError::TypeMismatch);
        }
    } else {
        let scalar_value = match right.value_type {
            DataType::Float32 => unsafe { right.value.float32 },
            DataType::Float64 => unsafe { right.value.float64 as f32 },
            DataType::Int8 => unsafe { right.value.i8 as f32 },
            DataType::Int16 => unsafe { right.value.i16 as f32 },
            DataType::Int32 => unsafe { right.value.i32 as f32 },
            DataType::Int64 => unsafe { right.value.i64 as f32 },
            DataType::UInt8 => unsafe { right.value.u8 as f32 },
            DataType::UInt16 => unsafe { right.value.u16 as f32 },
            DataType::UInt32 => unsafe { right.value.u32 as f32 },
            DataType::UInt64 => unsafe { right.value.u64 as f32 },
            _ => {
                return Err(QueryExecutionError::TypeMismatch);
            }
        };

        vec2_values = vec![scalar_value; vector_dim as usize];
    }

    let distance: f64 = unsafe {
        match op {
            BinaryOperator::VectorL2 => {
                let mut sum = 0.0f64;
                let vector_dim_usize = vector_dim as usize;
                for i in 0..vector_dim_usize {
                    let v1 = core::ptr::read_unaligned(vec1_ptr.add(i));
                    let v2 = vec2_values[i];
                    let diff = v1 - v2;
                    sum += (diff as f64) * (diff as f64);
                }
                sum.sqrt()
            }
            BinaryOperator::VectorIP => {
                let mut sum = 0.0f64;
                let vector_dim_usize = vector_dim as usize;
                for i in 0..vector_dim_usize {
                    let v1 = core::ptr::read_unaligned(vec1_ptr.add(i));
                    let v2 = vec2_values[i];
                    sum += (v1 as f64) * (v2 as f64);
                }
                sum
            }
            BinaryOperator::VectorCosine => {
                let mut dot = 0.0f64;
                let mut norm1 = 0.0f64;
                let mut norm2 = 0.0f64;

                let vector_dim_usize = vector_dim as usize;
                for i in 0..vector_dim_usize {
                    let v1 = core::ptr::read_unaligned(vec1_ptr.add(i)) as f64;
                    let v2 = vec2_values[i] as f64;
                    dot += v1 * v2;
                    norm1 += v1 * v1;
                    norm2 += v2 * v2;
                }

                let norm1 = norm1.sqrt();
                let norm2 = norm2.sqrt();

                if norm1 == 0.0 || norm2 == 0.0 {
                    -1.0
                } else {
                    dot / (norm1 * norm2)
                }
            }
            _ => unreachable!(),
        }
    };

    Ok(TypedValue {
        value_type: DataType::Float64,
        value: Value { float64: distance },
    })
}

pub fn evaluate_binary_op(
    left: TypedValue,
    op: BinaryOperator,
    right: TypedValue,
) -> Result<TypedValue, QueryExecutionError> {
    match op {
        BinaryOperator::Equal
        | BinaryOperator::NotEqual
        | BinaryOperator::LessThan
        | BinaryOperator::LessThanOrEqual
        | BinaryOperator::GreaterThan
        | BinaryOperator::GreaterThanOrEqual => {
            unsafe {
                if right.value_type == DataType::Int64 && right.value.i64 == 0 {
                    let is_null = match left.value_type {
                        DataType::UInt8 => left.value.u8 == 0,
                        DataType::UInt16 => left.value.u16 == 0,
                        DataType::UInt32 => left.value.u32 == 0,
                        DataType::UInt64 => left.value.u64 == 0,
                        DataType::Int8 => left.value.i8 == 0,
                        DataType::Int16 => left.value.i16 == 0,
                        DataType::Int32 => left.value.i32 == 0,
                        DataType::Int64 => left.value.i64 == 0,
                        DataType::Float32 => left.value.float32 == 0.0,
                        DataType::Float64 => left.value.float64 == 0.0,
                        DataType::Bool => !left.value.bool,
                        DataType::VarChar | DataType::Char | DataType::Text => {
                            let mut is_empty = true;
                            for &byte in &left.value.string {
                                if byte != 0 {
                                    is_empty = false;
                                    break;
                                }
                            }
                            is_empty
                        }
                        DataType::Timestamp => left.value.time.value == 0,
                        DataType::TimestampTZ => left.value.time.value == 0,
                        DataType::Interval => left.value.interval.value == 0,
                        DataType::Vector => left.value.vector.is_null(),
                        DataType::Json => {
                            matches!(left.value.json_storage, JsonStorage::Null)
                        },
                    };
                    
                    let result = match op {
                        BinaryOperator::Equal => is_null,
                        BinaryOperator::NotEqual => !is_null,
                        _ => return Err(QueryExecutionError::TypeMismatch),
                    };
                    
                    return Ok(TypedValue {
                        value_type: DataType::Bool,
                        value: Value { bool: result },
                    });
                }
                
                if matches!(left.value_type, DataType::VarChar | DataType::Char | DataType::Text) && matches!(right.value_type, DataType::VarChar | DataType::Char | DataType::Text) {
                    let left_str = core::str::from_utf8(&left.value.string)
                        .map_err(|_| QueryExecutionError::TypeMismatch)?
                        .trim_end_matches(char::from(0));
                    let right_str = core::str::from_utf8(&right.value.string)
                        .map_err(|_| QueryExecutionError::TypeMismatch)?
                        .trim_end_matches(char::from(0));
                    
                    let result = match op {
                        BinaryOperator::Equal => left_str == right_str,
                        BinaryOperator::NotEqual => left_str != right_str,
                        BinaryOperator::LessThan => left_str < right_str,
                        BinaryOperator::LessThanOrEqual => left_str <= right_str,
                        BinaryOperator::GreaterThan => left_str > right_str,
                        BinaryOperator::GreaterThanOrEqual => left_str >= right_str,
                        _ => return Err(QueryExecutionError::TypeMismatch),
                    };
                    
                    return Ok(TypedValue {
                        value_type: DataType::Bool,
                        value: Value { bool: result },
                    });
                }
                
                let left_val = match left.value_type {
                    DataType::UInt8 => left.value.u8 as f64,
                    DataType::UInt16 => left.value.u16 as f64,
                    DataType::UInt32 => left.value.u32 as f64,
                    DataType::UInt64 => left.value.u64 as f64,
                    DataType::Int8 => left.value.i8 as f64,
                    DataType::Int16 => left.value.i16 as f64,
                    DataType::Int32 => left.value.i32 as f64,
                    DataType::Int64 => left.value.i64 as f64,
                    DataType::Float32 => left.value.float32 as f64,
                    DataType::Float64 => left.value.float64,
                    DataType::Bool => left.value.bool as u8 as f64,
                    DataType::Timestamp => left.value.time.value as f64,
                    DataType::TimestampTZ => left.value.time.value as f64,
                    DataType::Json => {
                        let json_str = unsafe {
                            match &left.value.json_storage {
                                JsonStorage::Inline(data) => {
                                    let len = data.iter().rposition(|&b| b == 0).unwrap_or(256);
                                    String::from_utf8_lossy(&data[..len]).to_string()
                                }
                                JsonStorage::External { pool_id, offset, length } => {
                                    let pool_manager = crate::json::memory_pool::get_global_json_pool_manager()
                                        .ok_or(QueryExecutionError::InternalError)?;
                                    let pool = pool_manager.get_pool(*pool_id)
                                        .ok_or(QueryExecutionError::InternalError)?;
                                    if let Some(data_ptr) = pool.get_block_data(*offset as usize, 0) {
                                        let data = unsafe { core::slice::from_raw_parts(data_ptr, *length as usize) };
                                        let len = data.iter().position(|&b| b == 0).unwrap_or(data.len());
                                        String::from_utf8_lossy(&data[..len]).to_string()
                                    } else {
                                        return Err(QueryExecutionError::InternalError);
                                    }
                                }
                                JsonStorage::Null => "null".to_string(),
                            }
                        };
                        json_str.parse::<f64>().unwrap_or(0.0)
                    }
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };

                let right_val = match right.value_type {
                    DataType::UInt8 => right.value.u8 as f64,
                    DataType::UInt16 => right.value.u16 as f64,
                    DataType::UInt32 => right.value.u32 as f64,
                    DataType::UInt64 => right.value.u64 as f64,
                    DataType::Int8 => right.value.i8 as f64,
                    DataType::Int16 => right.value.i16 as f64,
                    DataType::Int32 => right.value.i32 as f64,
                    DataType::Int64 => right.value.i64 as f64,
                    DataType::Float32 => right.value.float32 as f64,
                    DataType::Float64 => right.value.float64,
                    DataType::Bool => right.value.bool as u8 as f64,
                    DataType::Timestamp => right.value.time.value as f64,
                    DataType::TimestampTZ => right.value.time.value as f64,
                    DataType::Json => {
                        let json_str = unsafe {
                            match &right.value.json_storage {
                                JsonStorage::Inline(data) => {
                                    let len = data.iter().rposition(|&b| b == 0).unwrap_or(256);
                                    String::from_utf8_lossy(&data[..len]).to_string()
                                }
                                JsonStorage::External { pool_id, offset, length } => {
                                    let pool_manager = crate::json::memory_pool::get_global_json_pool_manager()
                                        .ok_or(QueryExecutionError::InternalError)?;
                                    let pool = pool_manager.get_pool(*pool_id)
                                        .ok_or(QueryExecutionError::InternalError)?;
                                    if let Some(data_ptr) = pool.get_block_data(*offset as usize, 0) {
                                        let data = unsafe { core::slice::from_raw_parts(data_ptr, *length as usize) };
                                        let len = data.iter().position(|&b| b == 0).unwrap_or(data.len());
                                        String::from_utf8_lossy(&data[..len]).to_string()
                                    } else {
                                        return Err(QueryExecutionError::InternalError);
                                    }
                                }
                                JsonStorage::Null => "null".to_string(),
                            }
                        };
                        json_str.parse::<f64>().unwrap_or(0.0)
                    }
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };

                let result = match op {
                    BinaryOperator::Equal => left_val == right_val,
                    BinaryOperator::NotEqual => left_val != right_val,
                    BinaryOperator::LessThan => left_val < right_val,
                    BinaryOperator::LessThanOrEqual => left_val <= right_val,
                    BinaryOperator::GreaterThan => left_val > right_val,
                    BinaryOperator::GreaterThanOrEqual => left_val >= right_val,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };

                return Ok(TypedValue {
                    value_type: DataType::Bool,
                    value: Value { bool: result },
                });
            }
        }
        _ => {}
    }

    if op == BinaryOperator::Subtract {
        unsafe {
            match (left.value_type, right.value_type) {
                (DataType::Timestamp, DataType::Timestamp)
                | (DataType::TimestampTZ, DataType::TimestampTZ)
                | (DataType::Timestamp, DataType::TimestampTZ)
                | (DataType::TimestampTZ, DataType::Timestamp) => {
                    let t1 = left.value.time.value;
                    let t2 = right.value.time.value;
                    let diff = t1 - t2;

                    return Ok(TypedValue {
                        value_type: DataType::Interval,
                        value: Value {
                            interval: crate::types::db_interval::new(diff, 6, 0),
                        },
                    });
                }
                _ => {}
            }
        }
    }

    if matches!(
        left.value_type,
        DataType::UInt8
            | DataType::UInt16
            | DataType::UInt32
            | DataType::UInt64
            | DataType::Int8
            | DataType::Int16
            | DataType::Int32
            | DataType::Int64
            | DataType::Float32
            | DataType::Float64
    ) && matches!(
        right.value_type,
        DataType::UInt8
            | DataType::UInt16
            | DataType::UInt32
            | DataType::UInt64
            | DataType::Int8
            | DataType::Int16
            | DataType::Int32
            | DataType::Int64
            | DataType::Float32
            | DataType::Float64
    ) {
        unsafe {
            let left_val = match left.value_type {
                DataType::UInt8 => left.value.u8 as f64,
                DataType::UInt16 => left.value.u16 as f64,
                DataType::UInt32 => left.value.u32 as f64,
                DataType::UInt64 => left.value.u64 as f64,
                DataType::Int8 => left.value.i8 as f64,
                DataType::Int16 => left.value.i16 as f64,
                DataType::Int32 => left.value.i32 as f64,
                DataType::Int64 => left.value.i64 as f64,
                DataType::Float32 => left.value.float32 as f64,
                DataType::Float64 => left.value.float64,
                _ => unreachable!(),
            };

            let right_val = match right.value_type {
                DataType::UInt8 => right.value.u8 as f64,
                DataType::UInt16 => right.value.u16 as f64,
                DataType::UInt32 => right.value.u32 as f64,
                DataType::UInt64 => right.value.u64 as f64,
                DataType::Int8 => right.value.i8 as f64,
                DataType::Int16 => right.value.i16 as f64,
                DataType::Int32 => right.value.i32 as f64,
                DataType::Int64 => right.value.i64 as f64,
                DataType::Float32 => right.value.float32 as f64,
                DataType::Float64 => right.value.float64,
                _ => unreachable!(),
            };

            let result = match op {
                BinaryOperator::Add => left_val + right_val,
                BinaryOperator::Subtract => left_val - right_val,
                BinaryOperator::Multiply => left_val * right_val,
                BinaryOperator::Divide => {
                    if right_val == 0.0 {
                        return Err(QueryExecutionError::InternalError);
                    }
                    left_val / right_val
                },
                _ => return Err(QueryExecutionError::TypeMismatch),
            };

            return Ok(TypedValue {
                value_type: DataType::Float64,
                value: Value { float64: result },
            });
        }
    }

    let interval_micros = match right.value_type {
        DataType::Int64 => unsafe { right.value.i64 },
        DataType::VarChar | DataType::Char | DataType::Text => unsafe {
            let interval_str = core::str::from_utf8(&right.value.string)
                .map_err(|_| QueryExecutionError::TypeMismatch)?
                .trim_end_matches(char::from(0));
            sql_functions::parse_interval_string(interval_str)?
        },
        _ => return Err(QueryExecutionError::TypeMismatch),
    };

    match op {
        BinaryOperator::Add => {
            unsafe {
                match left.value_type {
                    DataType::Timestamp => {
                        let timestamp = left.value.time.value;
                        let new_timestamp = timestamp + interval_micros;

                        Ok(TypedValue {
                            value_type: DataType::Timestamp,
                            value: Value {
                                time: crate::types::db_timestamp::new(new_timestamp, 0, 6, 0),
                            },
                        })
                    }
                    DataType::TimestampTZ => {
                        let timestamp = left.value.time.value;
                        let tz_offset = left.value.time.tz_offset;
                        let new_timestamp = timestamp + interval_micros;

                        Ok(TypedValue {
                            value_type: DataType::TimestampTZ,
                            value: Value {
                                time: crate::types::db_timestamp::new(
                                    new_timestamp,
                                    tz_offset,
                                    6,
                                    0,
                                ),
                            },
                        })
                    }
                    _ => Err(QueryExecutionError::TypeMismatch),
                }
            }
        }
        BinaryOperator::Subtract => {
            unsafe {
                match left.value_type {
                    DataType::Timestamp => {
                        let timestamp = left.value.time.value;
                        let new_timestamp = timestamp - interval_micros;

                        Ok(TypedValue {
                            value_type: DataType::Timestamp,
                            value: Value {
                                time: crate::types::db_timestamp::new(new_timestamp, 0, 6, 0),
                            },
                        })
                    }
                    DataType::TimestampTZ => {
                        let timestamp = left.value.time.value;
                        let tz_offset = left.value.time.tz_offset;
                        let new_timestamp = timestamp - interval_micros;

                        Ok(TypedValue {
                            value_type: DataType::TimestampTZ,
                            value: Value {
                                time: crate::types::db_timestamp::new(
                                    new_timestamp,
                                    tz_offset,
                                    6,
                                    0,
                                ),
                            },
                        })
                    }
                    _ => Err(QueryExecutionError::TypeMismatch),
                }
            }
        }
        BinaryOperator::Equal
        | BinaryOperator::NotEqual
        | BinaryOperator::LessThan
        | BinaryOperator::LessThanOrEqual
        | BinaryOperator::GreaterThan
        | BinaryOperator::GreaterThanOrEqual => {
            unsafe {
                let t1 = match left.value_type {
                    DataType::Timestamp => left.value.time.value,
                    DataType::TimestampTZ => left.value.time.value,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };

                let t2 = match right.value_type {
                    DataType::Timestamp => right.value.time.value,
                    DataType::TimestampTZ => right.value.time.value,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };

                let result = match op {
                    BinaryOperator::Equal => t1 == t2,
                    BinaryOperator::NotEqual => t1 != t2,
                    BinaryOperator::LessThan => t1 < t2,
                    BinaryOperator::LessThanOrEqual => t1 <= t2,
                    BinaryOperator::GreaterThan => t1 > t2,
                    BinaryOperator::GreaterThanOrEqual => t1 >= t2,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };

                Ok(TypedValue {
                    value_type: DataType::Bool,
                    value: Value { bool: result },
                })
            }
        }
        _ => Err(QueryExecutionError::TypeMismatch),
    }
}

pub fn execute_function_call(
    name: &str,
    args: &[TypedValue],
) -> Result<TypedValue, QueryExecutionError> {
    #[cfg(feature = "log")]
    crate::log::debug!("execute_function_call: name={}, args.len={}", name, args.len());
    match name.to_uppercase().as_str() {
        "COUNT" => sql_functions::execute_count(args),
        "SUM" => sql_functions::execute_sum(args),
        "AVG" => sql_functions::execute_avg(args),
        "MIN" => sql_functions::execute_min(args),
        "MAX" => sql_functions::execute_max(args),
        "STDDEV" => sql_functions::execute_stddev(args),
        "VAR" => sql_functions::execute_var(args),
        "STDDEV_SAMP" => sql_functions::execute_stddev_samp(args),
        "VAR_SAMP" => sql_functions::execute_var_samp(args),
        "MOVING_AVERAGE" => sql_functions::execute_moving_average(args),
        "MOVING_SUM" => sql_functions::execute_moving_sum(args),
        "TIME_BUCKET" => sql_functions::execute_time_bucket(args),
        "TO_ISO8601" => sql_functions::execute_to_iso8601(args),
        "TO_CHAR" => sql_functions::execute_to_char(args),
        "TO_EPOCH" => sql_functions::execute_to_epoch(args),
        "CONCAT" => sql_functions::execute_concat(args),
        "SUBSTRING" => sql_functions::execute_substring(args),
        "UPPER" => sql_functions::execute_upper(args),
        "LOWER" => sql_functions::execute_lower(args),
        "LENGTH" => sql_functions::execute_length(args),
        "CHAR_LENGTH" => sql_functions::execute_char_length(args),
        "ABS" => sql_functions::execute_abs(args),
        "SQRT" => sql_functions::execute_sqrt(args),
        "POWER" => sql_functions::execute_power(args),
        "SIN" => sql_functions::execute_sin(args),
        "COS" => sql_functions::execute_cos(args),
        "LOG" => sql_functions::execute_log(args),
        "EXP" => sql_functions::execute_exp(args),
        "ROUND" => sql_functions::execute_round(args),
        "CEIL" => sql_functions::execute_ceil(args),
        "FLOOR" => sql_functions::execute_floor(args),
        "MOD" => sql_functions::execute_mod(args),
        "JSON_EXTRACT" => sql_functions::execute_json_extract(args),
        "JSON_VALUE" => sql_functions::execute_json_value(args),
        "JSON_QUERY" => sql_functions::execute_json_query(args),
        "JSON_HAS" => sql_functions::execute_json_has(args),
        "JSON_TYPE" => sql_functions::execute_json_type(args),
        "JSON_SET" => sql_functions::execute_json_set(args),
        "JSON_REMOVE" => sql_functions::execute_json_remove(args),
        "JSON_MERGE_PATCH" => sql_functions::execute_json_merge_patch(args),
        "JSON_ARRAY_APPEND" => sql_functions::execute_json_array_append(args),
        "JSON_ARRAY_LENGTH" => sql_functions::execute_json_array_length(args),
        "JSON_KEYS" => sql_functions::execute_json_keys(args),
        "JSON_ARRAY" => sql_functions::execute_json_array(args),
        "JSON_OBJECT" => sql_functions::execute_json_object(args),
        _ => {
            crate::model::model_udf::execute_model_udf(name, args)
                .or_else(|_| {
                    Err(QueryExecutionError::UnsupportedFunction(name.to_string()))
                })
        }
    }
}

pub fn evaluate_expression_without_table(
    db: &mut RemDb,
    expr: &Expression,
) -> Result<TypedValue, QueryExecutionError> {
    evaluate_expression_without_table_with_depth(db, expr, 0)
}

pub fn evaluate_expression_without_table_with_depth(
    db: &mut RemDb,
    expr: &Expression,
    depth: usize,
) -> Result<TypedValue, QueryExecutionError> {
    if depth > MAX_RECURSION_DEPTH {
        return Err(QueryExecutionError::InternalError);
    }
    match expr {
        Expression::Field {
            name: _field_name, ..
        } => {
            Err(QueryExecutionError::FieldNotFound)
        }
        Expression::FunctionCall { name, args, .. } => {
            let mut evaluated_args = Vec::with_capacity(args.len());
            for arg in args {
                evaluated_args.push(evaluate_expression_without_table_with_depth(db, arg, depth + 1)?);
            }

            execute_function_call(name, &evaluated_args)
        }
        Expression::Constant { value, .. } => {
            match value {
                crate::sql::query_parser::Value::Integer(i) => {
                    Ok(TypedValue {
                        value_type: DataType::Int64,
                        value: Value { i64: *i },
                    })
                }
                crate::sql::query_parser::Value::Float(f) => {
                    Ok(TypedValue {
                        value_type: DataType::Float64,
                        value: Value { float64: *f },
                    })
                }
                crate::sql::query_parser::Value::String(s) => {
                    let mut buf = [0; MAX_STRING_LEN];
                    let len = core::cmp::min(s.len(), MAX_STRING_LEN);
                    buf[..len].copy_from_slice(s.as_bytes());
                    Ok(TypedValue {
                        value_type: DataType::VarChar,
                        value: Value { string: buf },
                    })
                }
                crate::sql::query_parser::Value::Boolean(b) => {
                    Ok(TypedValue {
                        value_type: DataType::Bool,
                        value: Value { bool: *b },
                    })
                }
                crate::sql::query_parser::Value::Null => {
                    Ok(TypedValue {
                        value_type: DataType::Json,
                        value: Value { json_storage: JsonStorage::Null },
                    })
                }
                crate::sql::query_parser::Value::Identifier(_) => {
                    Err(QueryExecutionError::InvalidValue)
                }
                crate::sql::query_parser::Value::Json(s) => {
                    let mut buf = [0u8; 256];
                    let len = core::cmp::min(s.len(), 256);
                    buf[..len].copy_from_slice(s.as_bytes());
                    Ok(TypedValue {
                        value_type: DataType::Json,
                        value: Value { json_storage: JsonStorage::Inline(buf) },
                    })
                }
            }
        }
        Expression::BinaryOp { op, left, right, .. } => {
            let left_val = evaluate_expression_without_table_with_depth(db, left, depth + 1)?;
            let right_val = evaluate_expression_without_table_with_depth(db, right, depth + 1)?;

            evaluate_binary_op(left_val, *op, right_val)
        }
        Expression::LogicalOp { op, left, right, .. } => {
            let left_val = evaluate_expression_without_table_with_depth(db, left, depth + 1)?;
            let right_val = evaluate_expression_without_table_with_depth(db, right, depth + 1)?;

            unsafe {
                let left_bool = match left_val.value_type {
                    DataType::Bool => left_val.value.bool,
                    DataType::Int8 => left_val.value.i8 != 0,
                    DataType::Int16 => left_val.value.i16 != 0,
                    DataType::Int32 => left_val.value.i32 != 0,
                    DataType::Int64 => left_val.value.i64 != 0,
                    DataType::UInt8 => left_val.value.u8 != 0,
                    DataType::UInt16 => left_val.value.u16 != 0,
                    DataType::UInt32 => left_val.value.u32 != 0,
                    DataType::UInt64 => left_val.value.u64 != 0,
                    DataType::Float32 => left_val.value.float32 != 0.0,
                    DataType::Float64 => left_val.value.float64 != 0.0,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };

                let right_bool = match right_val.value_type {
                    DataType::Bool => right_val.value.bool,
                    DataType::Int8 => right_val.value.i8 != 0,
                    DataType::Int16 => right_val.value.i16 != 0,
                    DataType::Int32 => right_val.value.i32 != 0,
                    DataType::Int64 => right_val.value.i64 != 0,
                    DataType::UInt8 => right_val.value.u8 != 0,
                    DataType::UInt16 => right_val.value.u16 != 0,
                    DataType::UInt32 => right_val.value.u32 != 0,
                    DataType::UInt64 => right_val.value.u64 != 0,
                    DataType::Float32 => right_val.value.float32 != 0.0,
                    DataType::Float64 => right_val.value.float64 != 0.0,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };

                let result = match op {
                    LogicalOperator::And => left_bool && right_bool,
                    LogicalOperator::Or => left_bool || right_bool,
                };

                Ok(TypedValue {
                    value_type: DataType::Bool,
                    value: Value { bool: result },
                })
            }
        }
        Expression::UnaryOp { op, operand, .. } => {
            let operand_val = evaluate_expression_without_table_with_depth(db, operand, depth + 1)?;

            evaluate_unary_op(*op, operand_val)
        }
    }
}
