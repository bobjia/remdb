//! SQL Query Utility Functions
//!
//! This module contains shared utility functions used across SQL operations.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::sql::QueryExecutionError;
use crate::types::{DataType, DEFAULT_TEXT_SIZE, DEFAULT_JSON_SIZE};

/// 解析数据类型字符串，提取基本类型、精度/维度和距离类型
/// 例如："TIMESTAMP(6)" -> ("TIMESTAMP", 6, None)
///       "VECTOR(768)" -> ("VECTOR", 768, None)
///       "VECTOR(64) WITH DISTANCE=IP" -> ("VECTOR", 64, Some(InnerProduct))
pub fn parse_data_type_with_precision(type_str: &str) -> Result<(String, u16, Option<crate::types::DistanceType>), QueryExecutionError> {
    #[cfg(feature = "log")]
    crate::log::debug!("parse_data_type_with_precision called with: {}", type_str);
    let type_str = type_str.to_uppercase();

    // 查找左括号位置
    if let Some(open_paren) = type_str.find('(') {
        // 查找对应的右括号，忽略WITH子句
        let close_paren = type_str
            .find(')')
            .ok_or(QueryExecutionError::TypeMismatch)?;

        // 提取括号内的维度值，确保只包含数字
        let param_str = type_str[open_paren + 1..close_paren].trim();
        let param = param_str
            .parse::<u16>()
            .map_err(|_| QueryExecutionError::TypeMismatch)?;

        // 提取纯基本类型，用于匹配DataType
        let base_type = type_str[..open_paren].trim();

        // 对向量类型添加维度限制
        if base_type == "VECTOR" {
            // 向量维度限制为1-4096
            if param < 1 || param > 4096 {
                return Err(QueryExecutionError::TypeMismatch);
            }
        }

        // 解析距离类型（仅适用于向量类型）
        let mut distance_type = None;
        if base_type == "VECTOR" {
            // 检查是否包含WITH DISTANCE修饰符
            if type_str.contains("WITH DISTANCE=L2") {
                distance_type = Some(crate::types::DistanceType::L2);
            } else if type_str.contains("WITH DISTANCE=INNER_PRODUCT")
                || type_str.contains("WITH DISTANCE=IP")
            {
                distance_type = Some(crate::types::DistanceType::InnerProduct);
            } else if type_str.contains("WITH DISTANCE=COSINE") {
                distance_type = Some(crate::types::DistanceType::Cosine);
            }
        }

        // 验证基本类型是否有效
        match base_type {
            "INT" | "INTEGER" | "BIGINT" | "TINYINT" | "SMALLINT" | "INT16" | "INT32" | "INT64" |
            "UINT" | "UINTEGER" | "UBIGINT" | "UTINYINT" | "USMALLINT" | "UINT16" | "UINT32" | "UINT64" |
            "FLOAT" | "DOUBLE" | "REAL" | "FLOAT32" | "FLOAT64" |
            "VARCHAR" | "CHAR" | "TEXT" |
            "BOOL" | "BOOLEAN" |
            "TIMESTAMP" | "TIMESTAMPTZ" | "JSON" |
            "VECTOR" => Ok((base_type.to_string(), param, distance_type)),
            _ => Err(QueryExecutionError::TypeMismatch),
        }
    } else {
        // 没有参数，使用默认值
        let base_type = type_str.trim();
        
        // 验证基本类型是否有效，并为不同类型设置合适的默认大小
        match base_type {
            "INT" | "INTEGER" | "BIGINT" | "TINYINT" | "SMALLINT" | "INT16" | "INT32" | "INT64" |
            "UINT" | "UINTEGER" | "UBIGINT" | "UTINYINT" | "USMALLINT" | "UINT16" | "UINT32" | "UINT64" |
            "FLOAT" | "DOUBLE" | "REAL" | "FLOAT32" | "FLOAT64" |
            "BOOL" | "BOOLEAN" => Ok((base_type.to_string(), 8, None)),
            "VARCHAR" | "CHAR" => Ok((base_type.to_string(), 64, None)),
            "TEXT" => Ok((base_type.to_string(), DEFAULT_TEXT_SIZE as u16, None)),
            "JSON" => Ok((base_type.to_string(), DEFAULT_JSON_SIZE as u16, None)),
            "TIMESTAMP" | "TIMESTAMPTZ" => Ok((base_type.to_string(), 6, None)),
            _ => Err(QueryExecutionError::TypeMismatch),
        }
    }
}

/// 检查内存限制
pub fn check_memory_limit(
    estimated_usage: usize,
    max_memory_mb: Option<u32>,
) -> Result<(), QueryExecutionError> {
    if let Some(max_mb) = max_memory_mb {
        let max_bytes = (max_mb as usize) * 1024 * 1024;
        if estimated_usage > max_bytes {
            return Err(QueryExecutionError::ResourceLimitExceeded(
                format!("Query exceeds memory limit: {}MB estimated, {}MB allowed", 
                       estimated_usage / (1024 * 1024), max_mb)));
        }
    }

    Ok(())
}

/// 处理AT TIME ZONE操作符
/// 将timestamp转换为指定时区的timestamp
pub fn process_at_time_zone(
    timestamp: &crate::types::db_timestamp,
    timezone_spec: &str,
) -> Result<crate::types::db_timestamp, QueryExecutionError> {
    // 解析时区规范
    let tz_offset = if timezone_spec.starts_with('+') || timezone_spec.starts_with('-') {
        // 处理时区偏移格式，如 '+08:00' 或 '-05:30'
        let parts: Vec<&str> = timezone_spec.split(':').collect();
        if parts.len() == 2 {
            let hours = parts[0]
                .parse::<i32>()
                .map_err(|_| QueryExecutionError::TypeMismatch)?;
            let minutes = parts[1]
                .parse::<i32>()
                .map_err(|_| QueryExecutionError::TypeMismatch)?;
            ((hours * 3600) + (minutes * 60)) as i16
        } else {
            return Err(QueryExecutionError::TypeMismatch);
        }
    } else {
        // 处理时区名称格式，如 'UTC', 'Asia/Shanghai'
        crate::types::get_timezone_offset(timezone_spec).ok_or(QueryExecutionError::TypeMismatch)?
    };

    // 转换时间戳到指定时区
    Ok(crate::types::convert_timezone(timestamp, tz_offset))
}

/// 处理TIMEZONE()函数
/// 获取指定时区的偏移量
pub fn process_timezone_function(timezone_spec: &str) -> Result<i16, QueryExecutionError> {
    // 解析时区规范
    if timezone_spec.starts_with('+') || timezone_spec.starts_with('-') {
        // 处理时区偏移格式，如 '+08:00' 或 '-05:30'
        let parts: Vec<&str> = timezone_spec.split(':').collect();
        if parts.len() == 2 {
            let hours = parts[0]
                .parse::<i32>()
                .map_err(|_| QueryExecutionError::TypeMismatch)?;
            let minutes = parts[1]
                .parse::<i32>()
                .map_err(|_| QueryExecutionError::TypeMismatch)?;
            Ok(((hours * 3600) + (minutes * 60)) as i16)
        } else {
            Err(QueryExecutionError::TypeMismatch)
        }
    } else {
        // 处理时区名称格式，如 'UTC', 'Asia/Shanghai'
        crate::types::get_timezone_offset(timezone_spec).ok_or(QueryExecutionError::TypeMismatch)
    }
}

/// 处理TO_CHAR()函数
/// 将时间戳转换为指定格式的字符串
pub fn process_to_char(
    timestamp: &crate::types::db_timestamp,
    format: &str,
) -> Result<String, QueryExecutionError> {
    Ok(crate::types::time_format::to_char(timestamp, format))
}

/// 处理TO_ISO8601()函数
/// 将时间戳转换为ISO 8601格式的字符串
pub fn process_to_iso8601(
    timestamp: &crate::types::db_timestamp,
) -> Result<String, QueryExecutionError> {
    Ok(crate::types::time_format::to_iso8601(timestamp))
}

/// 处理TO_EPOCH()函数
/// 将时间戳转换为epoch秒数
pub fn process_to_epoch(timestamp: &crate::types::db_timestamp) -> Result<f64, QueryExecutionError> {
    Ok(crate::types::time_format::to_epoch(timestamp))
}
