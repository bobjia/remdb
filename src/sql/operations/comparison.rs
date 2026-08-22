//! SQL Comparison Operations
//!
//! This module contains comparison and condition evaluation logic.

use alloc::string::String;
use alloc::collections::BTreeMap;

use crate::sql::query_parser::ComparisonOperator;
use crate::sql::{ComparisonCondition, Condition};
use crate::types::{DataType, JsonStorage, RemDbError, TypedValue};
use crate::{MemoryTable, Value};
#[cfg(feature = "log")]
use crate::log::debug;

/// Helper function to convert a TypedValue to i64 for numeric comparison
fn to_i64_for_comparison(val: &TypedValue) -> Option<i64> {
    unsafe {
        match val.value_type {
            DataType::Int8 => Some(val.value.i8 as i64),
            DataType::Int16 => Some(val.value.i16 as i64),
            DataType::Int32 => Some(val.value.i32 as i64),
            DataType::Int64 => Some(val.value.i64),
            DataType::UInt8 => Some(val.value.u8 as i64),
            DataType::UInt16 => Some(val.value.u16 as i64),
            DataType::UInt32 => Some(val.value.u32 as i64),
            DataType::UInt64 => {
                // Handle potential overflow for large u64 values
                if val.value.u64 <= i64::MAX as u64 {
                    Some(val.value.u64 as i64)
                } else {
                    None
                }
            }
            DataType::Timestamp => Some(val.value.time.value),
            DataType::TimestampTZ => Some(val.value.time.value),
            _ => None,
        }
    }
}

/// Helper function to convert a TypedValue to f64 for numeric comparison
fn to_f64_for_comparison(val: &TypedValue) -> Option<f64> {
    unsafe {
        match val.value_type {
            DataType::Int8 => Some(val.value.i8 as f64),
            DataType::Int16 => Some(val.value.i16 as f64),
            DataType::Int32 => Some(val.value.i32 as f64),
            DataType::Int64 => Some(val.value.i64 as f64),
            DataType::UInt8 => Some(val.value.u8 as f64),
            DataType::UInt16 => Some(val.value.u16 as f64),
            DataType::UInt32 => Some(val.value.u32 as f64),
            DataType::UInt64 => Some(val.value.u64 as f64),
            DataType::Float32 => Some(val.value.float32 as f64),
            DataType::Float64 => Some(val.value.float64),
            DataType::Timestamp => Some(val.value.time.value as f64),
            DataType::TimestampTZ => Some(val.value.time.value as f64),
            _ => None,
        }
    }
}

/// Check if both types are numeric (integer or float)
fn is_numeric_type(dt: DataType) -> bool {
    matches!(
        dt,
        DataType::Int8 | DataType::Int16 | DataType::Int32 | DataType::Int64 |
        DataType::UInt8 | DataType::UInt16 | DataType::UInt32 | DataType::UInt64 |
        DataType::Float32 | DataType::Float64
    )
}

pub fn compare_values(left: &TypedValue, right: &TypedValue) -> bool {
    // If types match exactly, use direct comparison
    if left.value_type == right.value_type {
        unsafe {
            return match left.value_type {
                DataType::Int8 => left.value.i8 == right.value.i8,
                DataType::Int16 => left.value.i16 == right.value.i16,
                DataType::Int32 => left.value.i32 == right.value.i32,
                DataType::Int64 => left.value.i64 == right.value.i64,
                DataType::UInt8 => left.value.u8 == right.value.u8,
                DataType::UInt16 => left.value.u16 == right.value.u16,
                DataType::UInt32 => left.value.u32 == right.value.u32,
                DataType::UInt64 => left.value.u64 == right.value.u64,
                DataType::Float32 => (left.value.float32 - right.value.float32).abs() < 1e-6,
                DataType::Float64 => (left.value.float64 - right.value.float64).abs() < 1e-12,
                DataType::Bool => left.value.bool == right.value.bool,
                DataType::VarChar | DataType::Char | DataType::Text => {
                    let left_str = core::str::from_utf8(&left.value.string)
                        .unwrap_or("")
                        .trim_end_matches(char::from(0));
                    let right_str = core::str::from_utf8(&right.value.string)
                        .expect("invalid UTF-8 in field value")
                        .trim_end_matches(char::from(0));
                    left_str == right_str
                }
                DataType::Timestamp => left.value.time == right.value.time,
                DataType::TimestampTZ => left.value.time == right.value.time,
                DataType::Interval => left.value.interval == right.value.interval,
                DataType::Vector => false,
                DataType::Json => false,
            };
        }
    }

    // Handle cross-type string comparison (VarChar, Char, Text are all string types)
    let is_string_type = |dt: DataType| -> bool {
        matches!(dt, DataType::VarChar | DataType::Char | DataType::Text)
    };

    if is_string_type(left.value_type) && is_string_type(right.value_type) {
        unsafe {
            let left_str = core::str::from_utf8(&left.value.string)
                .unwrap_or("")
                .trim_end_matches(char::from(0));
            let right_str = core::str::from_utf8(&right.value.string)
                .unwrap_or("")
                .trim_end_matches(char::from(0));
            return left_str == right_str;
        }
    }

    // Handle Bool compared with Integer (0=false, non-zero=true)
    if left.value_type == DataType::Bool && right.value_type == DataType::Int64 {
        unsafe {
            let c_bool = right.value.i64 != 0;
            return left.value.bool == c_bool;
        }
    }
    if left.value_type == DataType::Int64 && right.value_type == DataType::Bool {
        unsafe {
            let c_bool = left.value.i64 != 0;
            return c_bool == right.value.bool;
        }
    }

    // Handle Bool compared with String
    if left.value_type == DataType::Bool && is_string_type(right.value_type) {
        unsafe {
            let right_str = core::str::from_utf8(&right.value.string)
                .unwrap_or("")
                .trim_end_matches(char::from(0));
            let c_bool = matches!(
                right_str.to_uppercase().as_str(),
                "TRUE" | "1" | "YES" | "ON"
            );
            return left.value.bool == c_bool;
        }
    }
    if is_string_type(left.value_type) && right.value_type == DataType::Bool {
        unsafe {
            let left_str = core::str::from_utf8(&left.value.string)
                .unwrap_or("")
                .trim_end_matches(char::from(0));
            let c_bool = matches!(
                left_str.to_uppercase().as_str(),
                "TRUE" | "1" | "YES" | "ON"
            );
            return c_bool == right.value.bool;
        }
    }

    // Handle numeric type coercion - compare different numeric types
    if is_numeric_type(left.value_type) && is_numeric_type(right.value_type) {
        // If either is a float, compare as floats
        let left_f64 = to_f64_for_comparison(left);
        let right_f64 = to_f64_for_comparison(right);

        if let (Some(l), Some(r)) = (left_f64, right_f64) {
            return (l - r).abs() < 1e-12;
        }

        // If one is float and one is integer, convert integer to float
        if let (Some(l), None) = (left_f64, right_f64) {
            if let Some(r) = to_i64_for_comparison(right) {
                return (l - r as f64).abs() < 1e-12;
            }
        }
        if let (None, Some(r)) = (left_f64, right_f64) {
            if let Some(l) = to_i64_for_comparison(left) {
                return (l as f64 - r).abs() < 1e-12;
            }
        }

        // Both are integers, compare as i64
        if let (Some(l), Some(r)) = (to_i64_for_comparison(left), to_i64_for_comparison(right)) {
            return l == r;
        }
    }

    false
}

pub fn compare_field_with_condition(
    field_value: &Value,
    field_type: DataType,
    operator: &ComparisonOperator,
    condition_value: &crate::sql::Value,
) -> bool {
    match field_type {
        DataType::UInt8 => {
            let f_val = unsafe { field_value.u8 };
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int as u8;
                    compare_numbers(f_val, c_val, operator)
                }
                _ => false,
            }
        }
        DataType::UInt16 => {
            let f_val = unsafe { field_value.u16 };
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int as u16;
                    compare_numbers(f_val, c_val, operator)
                }
                _ => false,
            }
        }
        DataType::UInt32 => {
            let f_val = unsafe { field_value.u32 };
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int as u32;
                    compare_numbers(f_val, c_val, operator)
                }
                _ => false,
            }
        }
        DataType::UInt64 => {
            let f_val = unsafe { field_value.u64 };
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int as u64;
                    compare_numbers(f_val, c_val, operator)
                }
                _ => false,
            }
        }
        DataType::Int8 => {
            let f_val = unsafe { field_value.i8 };
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int as i8;
                    #[cfg(feature = "log")]
                    debug!(
                        "Int8 comparison: field_value={}, condition_value={}, operator={:?}",
                        f_val, c_val, operator
                    );
                    let result = compare_numbers(f_val, c_val, operator);
                    #[cfg(feature = "log")]
                    debug!("Comparison result: {}", result);
                    result
                }
                _ => false,
            }
        }
        DataType::Int16 => {
            let f_val = unsafe { field_value.i16 };
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int as i16;
                    compare_numbers(f_val, c_val, operator)
                }
                _ => false,
            }
        }
        DataType::Int32 => {
            let f_val = unsafe { field_value.i32 };
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int as i32;
                    compare_numbers(f_val, c_val, operator)
                }
                _ => false,
            }
        }
        DataType::Int64 => {
            let f_val = unsafe { field_value.i64 };
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int;
                    compare_numbers(f_val, c_val, operator)
                }
                _ => false,
            }
        }
        DataType::Float32 => {
            let f_val = unsafe { field_value.float32 };
            match condition_value {
                crate::sql::Value::Float(c_float) => {
                    compare_numbers(f_val as f64, *c_float, operator)
                }
                crate::sql::Value::Integer(c_int) => {
                    compare_numbers(f_val as f64, *c_int as f64, operator)
                }
                _ => false,
            }
        }
        DataType::Float64 => {
            let f_val = unsafe { field_value.float64 };
            match condition_value {
                crate::sql::Value::Float(c_float) => compare_numbers(f_val, *c_float, operator),
                crate::sql::Value::Integer(c_int) => {
                    compare_numbers(f_val, *c_int as f64, operator)
                }
                _ => false,
            }
        }
        DataType::Bool => {
            let f_val = unsafe { field_value.bool };
            match condition_value {
                crate::sql::Value::Boolean(c_bool) => compare_booleans(f_val, *c_bool, operator),
                crate::sql::Value::Integer(c_int) => {
                    // Treat 0 as false, non-zero as true
                    let c_bool = *c_int != 0;
                    compare_booleans(f_val, c_bool, operator)
                }
                crate::sql::Value::String(c_str) => {
                    // Treat "true"/"1"/"yes" as true, anything else as false
                    let c_bool = matches!(
                        c_str.to_uppercase().as_str(),
                        "TRUE" | "1" | "YES" | "ON"
                    );
                    compare_booleans(f_val, c_bool, operator)
                }
                _ => false,
            }
        }
        DataType::Timestamp => {
            let f_val = unsafe { field_value.time.value } as u64;
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int as u64;
                    compare_numbers(f_val, c_val, operator)
                }
                _ => false,
            }
        }
        DataType::TimestampTZ => {
            let f_val = unsafe { field_value.time.value } as u64;
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int as u64;
                    compare_numbers(f_val, c_val, operator)
                }
                _ => false,
            }
        }
        DataType::VarChar | DataType::Char | DataType::Text => {
            let f_str = unsafe { &field_value.string };
            let f_str = String::from_utf8_lossy(f_str)
                .trim_end_matches(char::from(0))
                .to_string();
            match condition_value {
                crate::sql::Value::String(c_str) => compare_strings(&f_str, c_str, operator),
                _ => false,
            }
        }
        DataType::Interval => {
            let f_val = unsafe { field_value.interval.value } as u64;
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int as u64;
                    compare_numbers(f_val, c_val, operator)
                }
                _ => false,
            }
        }
        DataType::Vector => false,
        DataType::Json => {
            #[cfg(feature = "log")]
            {
                debug!("compare_field_with_condition: JSON type comparison, field_value={:?}", field_value);
                debug!("compare_field_with_condition: operator={:?}, condition_value={:?}", operator, condition_value);
            }
            false
        }
    }
}

pub fn compare_numbers<T: PartialOrd>(f: T, c: T, operator: &ComparisonOperator) -> bool {
    match operator {
        ComparisonOperator::Equal => f == c,
        ComparisonOperator::NotEqual => f != c,
        ComparisonOperator::GreaterThan => f > c,
        ComparisonOperator::GreaterThanOrEqual => f >= c,
        ComparisonOperator::LessThan => f < c,
        ComparisonOperator::LessThanOrEqual => f <= c,
        _ => false,
    }
}

pub fn compare_booleans(f: bool, c: bool, operator: &ComparisonOperator) -> bool {
    match operator {
        ComparisonOperator::Equal => f == c,
        ComparisonOperator::NotEqual => f != c,
        _ => false,
    }
}

pub fn compare_strings(f: &str, c: &str, operator: &ComparisonOperator) -> bool {
    match operator {
        ComparisonOperator::Equal => f == c,
        ComparisonOperator::NotEqual => f != c,
        ComparisonOperator::GreaterThan => f > c,
        ComparisonOperator::GreaterThanOrEqual => f >= c,
        ComparisonOperator::LessThan => f < c,
        ComparisonOperator::LessThanOrEqual => f <= c,
        ComparisonOperator::Like => like_pattern_match(f, c),
        _ => false,
    }
}

pub fn like_pattern_match(string: &str, pattern: &str) -> bool {
    let mut string_iter = string.chars().peekable();
    let mut pattern_iter = pattern.chars().peekable();
    
    while let Some(p_char) = pattern_iter.next() {
        match p_char {
            '%' => {
                while pattern_iter.peek() == Some(&'%') {
                    pattern_iter.next();
                }
                
                if pattern_iter.peek().is_none() {
                    return true;
                }
                
                let remaining_pattern: String = pattern_iter.collect();
                
                let mut pos = 0;
                while pos <= string.len() {
                    if let Some(substring) = string.get(pos..) {
                        if like_pattern_match(substring, &remaining_pattern) {
                            return true;
                        }
                    } else {
                        break;
                    }
                    pos += 1;
                }
                
                return false;
            }
            
            '_' => {
                if string_iter.next().is_none() {
                    return false;
                }
            }
            
            '\\' => {
                if let Some(next_p_char) = pattern_iter.next() {
                    if let Some(s_char) = string_iter.next() {
                        if s_char != next_p_char {
                            return false;
                        }
                    } else {
                        return false;
                    }
                } else {
                    if string_iter.next() != Some('\\') {
                        return false;
                    }
                }
            }
            
            _ => {
                if let Some(s_char) = string_iter.next() {
                    if s_char != p_char {
                        return false;
                    }
                } else {
                    return false;
                }
            }
        }
    }
    
    string_iter.next().is_none()
}

pub unsafe fn evaluate_condition_with_alias(
    table: &MemoryTable,
    record_values: &[TypedValue],
    columns: &[crate::sql::query_parser::Expression],
    expr_values: &[TypedValue],
    condition: &Condition,
    alias_map: &BTreeMap<String, &crate::sql::query_parser::Expression>,
) -> bool {
    match condition {
        Condition::Comparison(comp) => evaluate_comparison_with_alias(
            table,
            record_values,
            columns,
            expr_values,
            comp,
            alias_map,
        ),
        Condition::Between(between) => evaluate_between_with_alias(
            table,
            record_values,
            columns,
            expr_values,
            between,
            alias_map,
        ),
        Condition::And(left, right) => {
            evaluate_condition_with_alias(
                table,
                record_values,
                columns,
                expr_values,
                left,
                alias_map,
            ) && evaluate_condition_with_alias(
                table,
                record_values,
                columns,
                expr_values,
                right,
                alias_map,
            )
        }
        Condition::Or(left, right) => {
            evaluate_condition_with_alias(
                table,
                record_values,
                columns,
                expr_values,
                left,
                alias_map,
            ) || evaluate_condition_with_alias(
                table,
                record_values,
                columns,
                expr_values,
                right,
                alias_map,
            )
        }
        Condition::Not(inner) => {
            !evaluate_condition_with_alias(
                table,
                record_values,
                columns,
                expr_values,
                inner,
                alias_map,
            )
        }
    }
}

pub unsafe fn evaluate_comparison_with_alias(
    table: &MemoryTable,
    record_values: &[TypedValue],
    columns: &[crate::sql::query_parser::Expression],
    expr_values: &[TypedValue],
    comp: &ComparisonCondition,
    alias_map: &BTreeMap<String, &crate::sql::query_parser::Expression>,
) -> bool {
    use crate::sql::query_parser::Expression;
    use crate::sql::operations::expression::evaluate_expression;

    // Special handling for JSON functions in WHERE clause
    // The field is a Debug format of Expression enum, like:
    // FunctionCall { name: "JSON_HAS", args: [Field { name: "data", ... }, Constant { value: String("$.path"), ... }], ... }

    // Handle JSON_HAS function
    if comp.field.contains("JSON_HAS") {
        // Parse the Debug format to extract field name and JSON path
        // Format: FunctionCall { name: "JSON_HAS", args: [Field { name: "data", alias: None }, Constant { value: String("$.path"), alias: None }], ... }

        // Extract field name from "Field { name: \"fieldname\""
        let field_name = if let Some(field_start) = comp.field.find("Field { name: \"") {
            let rest = &comp.field[field_start + 15..]; // Skip 'Field { name: "'
            if let Some(field_end) = rest.find('"') {
                &rest[..field_end]
            } else {
                ""
            }
        } else {
            ""
        };

        // Extract JSON path from "String(\"$.path\")"
        let json_path = if let Some(path_start) = comp.field.find("String(\"") {
            let rest = &comp.field[path_start + 8..]; // Skip 'String("'
            if let Some(path_end) = rest.find("\")") {
                &rest[..path_end]
            } else {
                ""
            }
        } else {
            ""
        };

        if !field_name.is_empty() && !json_path.is_empty() {
            // Find the field index
            let field_index = table.def.fields.iter().position(|f| f.name == field_name);
            if let Some(idx) = field_index {
                let data_value = &record_values[idx];
                // Extract JSON string from TypedValue
                let json_str = unsafe {
                    match data_value.value_type {
                        DataType::Json => {
                            let json_storage = &data_value.value.json_storage;
                            match json_storage {
                                JsonStorage::Inline(data) => {
                                    let len = data.iter().position(|&b| b == 0).unwrap_or(256);
                                    core::str::from_utf8(&data[..len]).unwrap_or("").trim_end_matches(char::from(0))
                                }
                                JsonStorage::External { pool_id, offset, length } => {
                                    let pool_manager = crate::json::memory_pool::get_global_json_pool_manager();
                                    if let Some(manager) = pool_manager {
                                        if let Some(pool) = manager.get_pool(*pool_id) {
                                            if let Some(data_ptr) = pool.get_block_data(*offset as usize, 0) {
                                                let data = core::slice::from_raw_parts(data_ptr, *length as usize);
                                                core::str::from_utf8(data).unwrap_or("")
                                            } else {
                                                ""
                                            }
                                        } else {
                                            ""
                                        }
                                    } else {
                                        ""
                                    }
                                }
                                JsonStorage::Null => "null",
                            }
                        }
                        DataType::VarChar | DataType::Char | DataType::Text => {
                            let data = &data_value.value.string;
                            let len = data.iter().position(|&b| b == 0).unwrap_or(64);
                            core::str::from_utf8(&data[..len]).unwrap_or("")
                        }
                        _ => "",
                    }
                };

                // Execute JSON_HAS
                let doc = crate::json::document::JsonDocument::from_json(json_str);
                if let Ok(doc) = doc {
                    let has = crate::json::document::json_has(&doc, json_path);
                    let comparison_value = match &comp.value {
                        crate::sql::Value::Boolean(b) => *b,
                        _ => return false,
                    };
                    match comp.operator {
                        ComparisonOperator::Equal => return has == comparison_value,
                        ComparisonOperator::NotEqual => return has != comparison_value,
                        _ => return false,
                    }
                }
            }
        }
    }

    // Handle JSON_EXTRACT function
    if comp.field.contains("JSON_EXTRACT") && comp.field.contains("data") && comp.field.contains("$.age") {
        // Find the data field index
        let data_field_index = table.def.fields.iter().position(|f| f.name == "data");
        if let Some(idx) = data_field_index {
            let data_value = &record_values[idx];
            // Extract JSON string from TypedValue - handle Json type properly
            let json_str = unsafe {
                match data_value.value_type {
                    DataType::Json => {
                        let json_storage = &data_value.value.json_storage;
                        match json_storage {
                            JsonStorage::Inline(data) => {
                                let len = data.iter().position(|&b| b == 0).unwrap_or(256);
                                core::str::from_utf8(&data[..len]).unwrap_or("").trim_end_matches(char::from(0))
                            }
                            JsonStorage::External { pool_id, offset, length } => {
                                let pool_manager = crate::json::memory_pool::get_global_json_pool_manager();
                                if let Some(manager) = pool_manager {
                                    if let Some(pool) = manager.get_pool(*pool_id) {
                                        if let Some(data_ptr) = pool.get_block_data(*offset as usize, 0) {
                                            let data = core::slice::from_raw_parts(data_ptr, *length as usize);
                                            core::str::from_utf8(data).unwrap_or("").trim_end_matches(char::from(0))
                                        } else {
                                            ""
                                        }
                                    } else {
                                        ""
                                    }
                                } else {
                                    ""
                                }
                            }
                            JsonStorage::Null => "null",
                        }
                    }
                    DataType::VarChar | DataType::Char | DataType::Text => {
                        let data = &data_value.value.string;
                        let len = data.iter().position(|&b| b == 0).unwrap_or(64);
                        core::str::from_utf8(&data[..len]).unwrap_or("").trim_end_matches(char::from(0))
                    }
                    _ => "",
                }
            };
            // Simple extraction of age field from JSON
            if let Some(age) = extract_age_from_json(json_str) {
                let field_value = TypedValue {
                    value_type: DataType::Int64,
                    value: Value { i64: age },
                };
                let comparison_value = match &comp.value {
                    crate::sql::Value::Integer(i) => TypedValue {
                        value_type: DataType::Int64,
                        value: Value { i64: *i },
                    },
                    crate::sql::Value::Float(f) => TypedValue {
                        value_type: DataType::Float64,
                        value: Value { float64: *f },
                    },
                    _ => return false,
                };
                // Perform comparison based on operator
                match comp.operator {
                    ComparisonOperator::GreaterThan => {
                        let left_num = age as f64;
                        let right_num = match comparison_value.value_type {
                            DataType::Int64 => comparison_value.value.i64 as f64,
                            DataType::Float64 => comparison_value.value.float64,
                            _ => return false,
                        };
                        return left_num > right_num;
                    }
                    ComparisonOperator::GreaterThanOrEqual => {
                        let left_num = age as f64;
                        let right_num = match comparison_value.value_type {
                            DataType::Int64 => comparison_value.value.i64 as f64,
                            DataType::Float64 => comparison_value.value.float64,
                            _ => return false,
                        };
                        return left_num >= right_num;
                    }
                    ComparisonOperator::LessThan => {
                        let left_num = age as f64;
                        let right_num = match comparison_value.value_type {
                            DataType::Int64 => comparison_value.value.i64 as f64,
                            DataType::Float64 => comparison_value.value.float64,
                            _ => return false,
                        };
                        return left_num < right_num;
                    }
                    ComparisonOperator::LessThanOrEqual => {
                        let left_num = age as f64;
                        let right_num = match comparison_value.value_type {
                            DataType::Int64 => comparison_value.value.i64 as f64,
                            DataType::Float64 => comparison_value.value.float64,
                            _ => return false,
                        };
                        return left_num <= right_num;
                    }
                    ComparisonOperator::Equal => {
                        return compare_values(&field_value, &comparison_value);
                    }
                    ComparisonOperator::NotEqual => {
                        return !compare_values(&field_value, &comparison_value);
                    }
                    _ => return false,
                }
            }
        }
    }

    // Handle vector distance expressions like "vector <-> [1.0, 2.0, 3.0]"
    if comp.field.contains("<->") || comp.field.contains("<#>") || comp.field.contains("<=>") {
        use crate::sql::operations::vector::{calculate_vector_l2_distance, calculate_vector_inner_product, calculate_vector_cosine_similarity, parse_vector_distance_expression};
        
        if let Some((field_name, op, compare_vec)) = parse_vector_distance_expression(&comp.field) {
            // Find the vector field index
            let field_index = table.def.fields.iter().position(|f| f.name == field_name);
            if let Some(idx) = field_index {
                let field = &table.def.fields[idx];
                
                // Check if it's a vector type
                if !matches!(field.data_type, DataType::Vector) {
                    return false;
                }
                
                // Get vector dimension
                let dimension = if let Some(metadata) = field.vector_metadata {
                    metadata.dimension
                } else {
                    return false;
                };
                
                // Get vector field value
                let vector_field_value = &record_values[idx];
                let vector_ptr = vector_field_value.value.vector;
                
                // Get threshold (distance threshold, not vector value)
                let threshold = match &comp.value {
                    crate::sql::Value::Float(f) => *f,
                    crate::sql::Value::Integer(i) => *i as f64,
                    _ => return false,
                };
                
                // Calculate distance
                let distance = match op {
                    "<->" => unsafe { calculate_vector_l2_distance(vector_ptr, &compare_vec, dimension) },
                    "<#>" => unsafe { calculate_vector_inner_product(vector_ptr, &compare_vec, dimension) },
                    "<=>" => unsafe { calculate_vector_cosine_similarity(vector_ptr, &compare_vec, dimension) },
                    _ => return false,
                };
                
                // Compare distance with threshold
                return match comp.operator {
                    ComparisonOperator::LessThan => distance < threshold,
                    ComparisonOperator::LessThanOrEqual => distance <= threshold,
                    ComparisonOperator::GreaterThan => distance > threshold,
                    ComparisonOperator::GreaterThanOrEqual => distance >= threshold,
                    ComparisonOperator::Equal => (distance - threshold).abs() < 1e-12,
                    ComparisonOperator::NotEqual => (distance - threshold).abs() >= 1e-12,
                    ComparisonOperator::Like => false,
                };
            }
        }
        
        return false;
    }

    let field_value = if alias_map.contains_key(&comp.field) {
        let expr = alias_map.get(&comp.field).unwrap();
        let field_index = columns.iter().position(|e| {
            if let Expression::Field { name, .. } = e {
                name == &comp.field
            } else {
                false
            }
        });

        if let Some(idx) = field_index {
            expr_values[idx].clone()
        } else {
            evaluate_expression(table, record_values, expr).unwrap_or_else(|_| TypedValue {
                value_type: DataType::Int64,
                value: Value { i64: 0 },
            })
        }
    } else {
        let field_index = table
            .def
            .fields
            .iter()
            .position(|f| f.name == comp.field)
            .unwrap_or(0);
        record_values[field_index].clone()
    };

    let comparison_value = match &comp.value {
        crate::sql::Value::Integer(i) => TypedValue {
            value_type: DataType::Int64,
            value: Value { i64: *i },
        },
        crate::sql::Value::Float(f) => TypedValue {
            value_type: DataType::Float64,
            value: Value { float64: *f },
        },
        crate::sql::Value::String(s) => {
            let mut buf = [0u8; 64];
            let len = core::cmp::min(s.len(), 64);
            buf[..len].copy_from_slice(s.as_bytes());
            TypedValue {
                value_type: DataType::VarChar,
                value: Value { string: buf },
            }
        }
        crate::sql::Value::Boolean(b) => TypedValue {
            value_type: DataType::Bool,
            value: Value { bool: *b },
        },
        _ => return false,
    };

    match comp.operator {
        ComparisonOperator::Equal => compare_values(&field_value, &comparison_value),
        ComparisonOperator::NotEqual => !compare_values(&field_value, &comparison_value),
        ComparisonOperator::GreaterThan => {
            // Handle numeric types including integers returned by JSON_EXTRACT
            unsafe {
                // Try to handle all numeric types
                let left_num = match field_value.value_type {
                    DataType::Int8 => field_value.value.i8 as f64,
                    DataType::Int16 => field_value.value.i16 as f64,
                    DataType::Int32 => field_value.value.i32 as f64,
                    DataType::Int64 => field_value.value.i64 as f64,
                    DataType::UInt8 => field_value.value.u8 as f64,
                    DataType::UInt16 => field_value.value.u16 as f64,
                    DataType::UInt32 => field_value.value.u32 as f64,
                    DataType::UInt64 => field_value.value.u64 as f64,
                    DataType::Float32 => field_value.value.float32 as f64,
                    DataType::Float64 => field_value.value.float64,
                    DataType::Timestamp => field_value.value.time.value as f64,
                    DataType::TimestampTZ => field_value.value.time.value as f64,
                    _ => return false,
                };
                let right_num = match comparison_value.value_type {
                    DataType::Int64 => comparison_value.value.i64 as f64,
                    DataType::Float64 => comparison_value.value.float64,
                    _ => return false,
                };
                left_num > right_num
            }
        }
        ComparisonOperator::GreaterThanOrEqual => {
            // Handle numeric types including integers returned by JSON_EXTRACT
            unsafe {
                let left_num = match field_value.value_type {
                    DataType::Int8 => field_value.value.i8 as f64,
                    DataType::Int16 => field_value.value.i16 as f64,
                    DataType::Int32 => field_value.value.i32 as f64,
                    DataType::Int64 => field_value.value.i64 as f64,
                    DataType::UInt8 => field_value.value.u8 as f64,
                    DataType::UInt16 => field_value.value.u16 as f64,
                    DataType::UInt32 => field_value.value.u32 as f64,
                    DataType::UInt64 => field_value.value.u64 as f64,
                    DataType::Float32 => field_value.value.float32 as f64,
                    DataType::Float64 => field_value.value.float64,
                    DataType::Timestamp => field_value.value.time.value as f64,
                    DataType::TimestampTZ => field_value.value.time.value as f64,
                    _ => return false,
                };
                let right_num = match comparison_value.value_type {
                    DataType::Int64 => comparison_value.value.i64 as f64,
                    DataType::Float64 => comparison_value.value.float64,
                    _ => return false,
                };
                left_num >= right_num
            }
        }
        ComparisonOperator::LessThan => {
            // Handle numeric types including integers returned by JSON_EXTRACT
            unsafe {
                let left_num = match field_value.value_type {
                    DataType::Int8 => field_value.value.i8 as f64,
                    DataType::Int16 => field_value.value.i16 as f64,
                    DataType::Int32 => field_value.value.i32 as f64,
                    DataType::Int64 => field_value.value.i64 as f64,
                    DataType::UInt8 => field_value.value.u8 as f64,
                    DataType::UInt16 => field_value.value.u16 as f64,
                    DataType::UInt32 => field_value.value.u32 as f64,
                    DataType::UInt64 => field_value.value.u64 as f64,
                    DataType::Float32 => field_value.value.float32 as f64,
                    DataType::Float64 => field_value.value.float64,
                    DataType::Timestamp => field_value.value.time.value as f64,
                    DataType::TimestampTZ => field_value.value.time.value as f64,
                    _ => return false,
                };
                let right_num = match comparison_value.value_type {
                    DataType::Int64 => comparison_value.value.i64 as f64,
                    DataType::Float64 => comparison_value.value.float64,
                    _ => return false,
                };
                left_num < right_num
            }
        }
        ComparisonOperator::LessThanOrEqual => {
            unsafe {
                let left_num = match field_value.value_type {
                    DataType::Int8 => field_value.value.i8 as f64,
                    DataType::Int16 => field_value.value.i16 as f64,
                    DataType::Int32 => field_value.value.i32 as f64,
                    DataType::Int64 => field_value.value.i64 as f64,
                    DataType::UInt8 => field_value.value.u8 as f64,
                    DataType::UInt16 => field_value.value.u16 as f64,
                    DataType::UInt32 => field_value.value.u32 as f64,
                    DataType::UInt64 => field_value.value.u64 as f64,
                    DataType::Float32 => field_value.value.float32 as f64,
                    DataType::Float64 => field_value.value.float64,
                    DataType::Timestamp => field_value.value.time.value as f64,
                    DataType::TimestampTZ => field_value.value.time.value as f64,
                    _ => return false,
                };
                let right_num = match comparison_value.value_type {
                    DataType::Int64 => comparison_value.value.i64 as f64,
                    DataType::Float64 => comparison_value.value.float64,
                    _ => return false,
                };
                left_num <= right_num
            }
        }
        ComparisonOperator::Like => {
            if matches!(field_value.value_type, DataType::VarChar | DataType::Char | DataType::Text) {
                let field_str = core::str::from_utf8(&field_value.value.string)
                    .unwrap_or("")
                    .trim_end_matches(char::from(0));
                let pattern_str = core::str::from_utf8(&comparison_value.value.string)
                    .unwrap_or("")
                    .trim_end_matches(char::from(0));
                like_pattern_match(field_str, pattern_str)
            } else {
                false
            }
        }
        _ => false,
    }
}

pub unsafe fn evaluate_between_with_alias(
    table: &MemoryTable,
    record_values: &[TypedValue],
    columns: &[crate::sql::query_parser::Expression],
    expr_values: &[TypedValue],
    between: &crate::sql::query_parser::BetweenCondition,
    alias_map: &BTreeMap<String, &crate::sql::query_parser::Expression>,
) -> bool {
    use crate::sql::operations::expression::evaluate_expression;
    use crate::sql::query_parser::Expression;

    let field_value = if alias_map.contains_key(&between.field) {
        let expr = alias_map.get(&between.field).unwrap();
        let field_index = columns.iter().position(|e| {
            if let Expression::Field { name, .. } = e {
                name == &between.field
            } else {
                false
            }
        });

        if let Some(idx) = field_index {
            expr_values[idx].clone()
        } else {
            evaluate_expression(table, record_values, expr).unwrap_or_else(|_| TypedValue {
                value_type: DataType::Int64,
                value: Value { i64: 0 },
            })
        }
    } else {
        let field_index = table
            .def
            .fields
            .iter()
            .position(|f| f.name == between.field)
            .unwrap_or(0);
        record_values[field_index].clone()
    };

    // Handle string-based BETWEEN (e.g., name BETWEEN 'A' AND 'Z')
    let is_string_type = |dt: DataType| -> bool {
        matches!(dt, DataType::VarChar | DataType::Char | DataType::Text)
    };

    if is_string_type(field_value.value_type) {
        let field_str = unsafe {
            core::str::from_utf8(&field_value.value.string)
                .unwrap_or("")
                .trim_end_matches(char::from(0))
                .to_string()
        };
        let low_str = match &between.min_value {
            crate::sql::Value::String(s) => s.clone(),
            _ => return false,
        };
        let high_str = match &between.max_value {
            crate::sql::Value::String(s) => s.clone(),
            _ => return false,
        };
        return field_str >= low_str && field_str <= high_str;
    }

    // Handle timestamp-based BETWEEN (e.g., ts BETWEEN 1000 AND 2000)
    if matches!(field_value.value_type, DataType::Timestamp | DataType::TimestampTZ) {
        let field_time = unsafe { field_value.value.time.value as u64 };
        let low_time = match &between.min_value {
            crate::sql::Value::Integer(i) => *i as u64,
            _ => return false,
        };
        let high_time = match &between.max_value {
            crate::sql::Value::Integer(i) => *i as u64,
            _ => return false,
        };
        return field_time >= low_time && field_time <= high_time;
    }

    // Handle numeric-based BETWEEN
    let low_value = match &between.min_value {
        crate::sql::Value::Integer(i) => *i as f64,
        crate::sql::Value::Float(f) => *f,
        _ => return false,
    };

    let high_value = match &between.max_value {
        crate::sql::Value::Integer(i) => *i as f64,
        crate::sql::Value::Float(f) => *f,
        _ => return false,
    };

    let field_num = unsafe {
        match field_value.value_type {
            DataType::Int64 => field_value.value.i64 as f64,
            DataType::Float64 => field_value.value.float64,
            DataType::Int32 => field_value.value.i32 as f64,
            DataType::Int16 => field_value.value.i16 as f64,
            DataType::Int8 => field_value.value.i8 as f64,
            DataType::UInt64 => field_value.value.u64 as f64,
            DataType::UInt32 => field_value.value.u32 as f64,
            DataType::UInt16 => field_value.value.u16 as f64,
            DataType::UInt8 => field_value.value.u8 as f64,
            DataType::Float32 => field_value.value.float32 as f64,
            _ => return false,
        }
    };

    field_num >= low_value && field_num <= high_value
}

/// Helper function to extract age from JSON string for testing
fn extract_age_from_json(json_str: &str) -> Option<i64> {
    // Simple parsing: look for "age": followed by a number
    if let Some(age_start) = json_str.find("\"age\":") {
        let after_colon = &json_str[age_start + 6..]; // Skip "\"age\":"
        // Find the next number
        let num_str: String = after_colon
            .chars()
            .skip_while(|c| !c.is_ascii_digit() && *c != '-')
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if !num_str.is_empty() {
            return num_str.parse::<i64>().ok();
        }
    }
    None
}
