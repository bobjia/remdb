//! SQL Comparison Operations
//!
//! This module contains comparison and condition evaluation logic.

use alloc::string::String;
use alloc::collections::BTreeMap;

use crate::sql::query_parser::ComparisonOperator;
use crate::sql::{ComparisonCondition, Condition};
use crate::types::{DataType, TypedValue};
use crate::{MemoryTable, Value};
#[cfg(feature = "log")]
use crate::log::debug;

pub fn compare_values(left: &TypedValue, right: &TypedValue) -> bool {
    if left.value_type != right.value_type {
        return false;
    }

    unsafe {
        match left.value_type {
            DataType::Int8 => left.value.i8 == right.value.i8,
            DataType::Int16 => left.value.i16 == right.value.i16,
            DataType::Int32 => left.value.i32 == right.value.i32,
            DataType::Int64 => left.value.i64 == right.value.i64,
            DataType::UInt8 => left.value.u8 == right.value.u8,
            DataType::UInt16 => left.value.u16 == right.value.u16,
            DataType::UInt32 => left.value.u32 == right.value.u32,
            DataType::UInt64 => left.value.u64 == right.value.u64,
            DataType::Float32 => (left.value.float32 - right.value.float32).abs() < f32::EPSILON,
            DataType::Float64 => (left.value.float64 - right.value.float64).abs() < f64::EPSILON,
            DataType::Bool => left.value.bool == right.value.bool,
            DataType::VarChar | DataType::Char | DataType::Text => {
                let left_str = core::str::from_utf8(&left.value.string)
                    .unwrap()
                    .trim_end_matches(char::from(0));
                let right_str = core::str::from_utf8(&right.value.string)
                    .unwrap()
                    .trim_end_matches(char::from(0));
                left_str == right_str
            }
            DataType::Timestamp => left.value.time == right.value.time,
            DataType::TimestampTZ => left.value.time == right.value.time,
            DataType::Interval => left.value.interval == right.value.interval,
            DataType::Vector => false,
            DataType::Json => false,
        }
    }
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
            unsafe {
                let left_num = match field_value.value_type {
                    DataType::Int64 => field_value.value.i64 as f64,
                    DataType::Float64 => field_value.value.float64,
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
            unsafe {
                let left_num = match field_value.value_type {
                    DataType::Int64 => field_value.value.i64 as f64,
                    DataType::Float64 => field_value.value.float64,
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
            unsafe {
                let left_num = match field_value.value_type {
                    DataType::Int64 => field_value.value.i64 as f64,
                    DataType::Float64 => field_value.value.float64,
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
                    DataType::Int64 => field_value.value.i64 as f64,
                    DataType::Float64 => field_value.value.float64,
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
    use crate::sql::query_parser::Expression;
    use crate::sql::operations::expression::evaluate_expression;

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
