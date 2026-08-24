//! SQL Time Functions
//!
//! This module contains time-related function implementations like TIME_BUCKET, TO_ISO8601, TO_CHAR, TO_EPOCH.

use crate::sql::QueryExecutionError;
use crate::types::DataType;
use crate::types::TypedValue;
use crate::Value;
use crate::MAX_STRING_LEN;

/// 执行TIME_BUCKET函数
pub fn execute_time_bucket(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    #[cfg(feature = "log")]
    crate::log::debug!(
        "execute_time_bucket: args.len={}, args[0].type={:?}, args[1].type={:?}",
        args.len(),
        args.first().map(|a| a.value_type),
        args.get(1).map(|a| a.value_type)
    );
    if args.len() < 2 {
        return Err(QueryExecutionError::TypeMismatch);
    }

    // 解析时间间隔参数
    let interval_micros = parse_time_interval(&args[0])?;

    // 获取时间戳参数
    let timestamp_arg = &args[1];

    // 解析可选的origin参数
    let origin_micros = if args.len() > 2 {
        parse_origin_timestamp(&args[2])?
    } else {
        0 // 默认从1970-01-01 00:00:00开始
    };

    unsafe {
        // 从不同类型中提取时间戳值
        let timestamp = match timestamp_arg.value_type {
            DataType::Timestamp => timestamp_arg.value.time.value,
            DataType::TimestampTZ => timestamp_arg.value.time.value,
            DataType::UInt64 => timestamp_arg.value.u64 as i64,
            DataType::Int64 => timestamp_arg.value.i64,
            DataType::UInt32 => timestamp_arg.value.u32 as i64,
            DataType::Int32 => timestamp_arg.value.i32 as i64,
            DataType::UInt16 => timestamp_arg.value.u16 as i64,
            DataType::Int16 => timestamp_arg.value.i16 as i64,
            DataType::UInt8 => timestamp_arg.value.u8 as i64,
            DataType::Int8 => timestamp_arg.value.i8 as i64,
            _ => return Err(QueryExecutionError::TypeMismatch),
        };

        // 将时间戳对齐到指定的时间窗口，考虑origin
        let bucketed_timestamp =
            origin_micros + ((timestamp - origin_micros) / interval_micros) * interval_micros;

        // 根据输入类型返回相同类型的结果
        match timestamp_arg.value_type {
            DataType::Timestamp => Ok(TypedValue {
                value_type: DataType::Timestamp,
                value: Value {
                    time: crate::types::db_timestamp::new(bucketed_timestamp, 0, 6, 0),
                },
            }),
            DataType::TimestampTZ => Ok(TypedValue {
                value_type: DataType::TimestampTZ,
                value: Value {
                    time: crate::types::db_timestamp::new(
                        bucketed_timestamp,
                        timestamp_arg.value.time.tz_offset,
                        timestamp_arg.value.time.precision,
                        timestamp_arg.value.time.flags,
                    ),
                },
            }),
            DataType::UInt64 => Ok(TypedValue {
                value_type: DataType::UInt64,
                value: Value {
                    u64: bucketed_timestamp as u64,
                },
            }),
            DataType::Int64 => Ok(TypedValue {
                value_type: DataType::Int64,
                value: Value {
                    i64: bucketed_timestamp,
                },
            }),
            DataType::UInt32 => Ok(TypedValue {
                value_type: DataType::UInt32,
                value: Value {
                    u32: bucketed_timestamp as u32,
                },
            }),
            DataType::Int32 => Ok(TypedValue {
                value_type: DataType::Int32,
                value: Value {
                    i32: bucketed_timestamp as i32,
                },
            }),
            DataType::UInt16 => Ok(TypedValue {
                value_type: DataType::UInt16,
                value: Value {
                    u16: bucketed_timestamp as u16,
                },
            }),
            DataType::Int16 => Ok(TypedValue {
                value_type: DataType::Int16,
                value: Value {
                    i16: bucketed_timestamp as i16,
                },
            }),
            DataType::UInt8 => Ok(TypedValue {
                value_type: DataType::UInt8,
                value: Value {
                    u8: bucketed_timestamp as u8,
                },
            }),
            DataType::Int8 => Ok(TypedValue {
                value_type: DataType::Int8,
                value: Value {
                    i8: bucketed_timestamp as i8,
                },
            }),
            _ => Err(QueryExecutionError::TypeMismatch),
        }
    }
}

/// 执行TO_ISO8601函数
pub fn execute_to_iso8601(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let timestamp_arg = &args[0];

    unsafe {
        match timestamp_arg.value_type {
            DataType::Timestamp | DataType::TimestampTZ => {
                let timestamp = &timestamp_arg.value.time;
                let result = process_to_iso8601(timestamp)?;

                // 将字符串转换为TypedValue
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
            _ => Err(QueryExecutionError::TypeMismatch),
        }
    }
}

/// 执行TO_CHAR函数
pub fn execute_to_char(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.len() < 2 {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let timestamp_arg = &args[0];
    let format_arg = &args[1];

    unsafe {
        match (timestamp_arg.value_type, format_arg.value_type) {
            (
                DataType::Timestamp | DataType::TimestampTZ,
                DataType::VarChar | DataType::Char | DataType::Text,
            ) => {
                let timestamp = &timestamp_arg.value.time;
                // 提取字符串格式
                let format_str = core::str::from_utf8(&format_arg.value.string)
                    .map_err(|_| QueryExecutionError::TypeMismatch)?
                    .trim_end_matches(char::from(0));

                let result = process_to_char(timestamp, format_str)?;

                // 将字符串转换为TypedValue
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
            _ => Err(QueryExecutionError::TypeMismatch),
        }
    }
}

/// 执行TO_EPOCH函数
pub fn execute_to_epoch(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let timestamp_arg = &args[0];

    unsafe {
        match timestamp_arg.value_type {
            DataType::Timestamp | DataType::TimestampTZ => {
                let timestamp = &timestamp_arg.value.time;
                let result = process_to_epoch(timestamp)?;

                Ok(TypedValue {
                    value_type: DataType::Float64,
                    value: Value { float64: result },
                })
            }
            _ => Err(QueryExecutionError::TypeMismatch),
        }
    }
}

/// 解析时间字符串为微秒时间戳
pub fn parse_time_string(time_str: &str) -> Result<i64, QueryExecutionError> {
    let time_str = time_str.trim();
    let mut parts = time_str.split_whitespace();

    let date_part = parts.next().ok_or(QueryExecutionError::TypeMismatch)?;
    let date_components: Vec<&str> = date_part.split('-').collect();
    if date_components.len() != 3 {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let year = date_components[0]
        .parse::<i64>()
        .map_err(|_| QueryExecutionError::TypeMismatch)?;
    let month = date_components[1]
        .parse::<i64>()
        .map_err(|_| QueryExecutionError::TypeMismatch)?;
    let day = date_components[2]
        .parse::<i64>()
        .map_err(|_| QueryExecutionError::TypeMismatch)?;

    let mut hour = 0;
    let mut minute = 0;
    let mut second = 0;

    if let Some(time_part) = parts.next() {
        let (time_only, _tz_offset_seconds) = split_timezone_from_time(time_part);
        let time_components: Vec<&str> = time_only.split(':').collect();
        if time_components.len() != 3 {
            return Err(QueryExecutionError::TypeMismatch);
        }

        hour = time_components[0]
            .parse::<i64>()
            .map_err(|_| QueryExecutionError::TypeMismatch)?;
        minute = time_components[1]
            .parse::<i64>()
            .map_err(|_| QueryExecutionError::TypeMismatch)?;
        second = time_components[2]
            .parse::<i64>()
            .map_err(|_| QueryExecutionError::TypeMismatch)?;
    }

    let mut seconds = 0;

    for _y in 1970..year {
        seconds += 365 * 24 * 60 * 60;
    }

    let days_in_month = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for m in 0..(month - 1) {
        seconds += days_in_month[m as usize] * 24 * 60 * 60;
    }

    seconds += (day - 1) * 24 * 60 * 60;
    seconds += hour * 60 * 60;
    seconds += minute * 60;
    seconds += second;

    Ok(seconds * 1000000)
}

fn split_timezone_from_time(time_part: &str) -> (&str, i32) {
    if let Some(pos) = time_part.find(['+', '-']) {
        if pos > 0 {
            let before = &time_part[..pos];
            let after = &time_part[pos..];
            if after.len() > 1 && after.chars().nth(1).is_some_and(|c| c.is_ascii_digit()) {
                let tz_seconds = parse_timezone_offset(after).unwrap_or(0);
                return (before, tz_seconds);
            }
        }
    }
    (time_part, 0)
}

fn parse_timezone_offset(tz_str: &str) -> Option<i32> {
    let sign = if tz_str.starts_with('+') {
        1
    } else if tz_str.starts_with('-') {
        -1
    } else {
        return None;
    };
    let offset_str = &tz_str[1..];

    let parts: Vec<&str> = offset_str.split(':').collect();
    if parts.len() == 2 {
        let hours = parts[0].parse::<i32>().ok()?;
        let minutes = parts[1].parse::<i32>().ok()?;
        Some(sign * (hours * 3600 + minutes * 60))
    } else if offset_str.len() == 2 {
        let hours = offset_str.parse::<i32>().ok()?;
        Some(sign * hours * 3600)
    } else if offset_str.len() == 4 {
        let hours = offset_str[0..2].parse::<i32>().ok()?;
        let minutes = offset_str[2..4].parse::<i32>().ok()?;
        Some(sign * (hours * 3600 + minutes * 60))
    } else {
        None
    }
}

/// 解析origin时间戳参数
pub fn parse_origin_timestamp(origin_arg: &TypedValue) -> Result<i64, QueryExecutionError> {
    unsafe {
        match origin_arg.value_type {
            // 数值形式的时间戳（微秒）
            DataType::UInt8 => Ok(origin_arg.value.u8 as i64),
            DataType::UInt16 => Ok(origin_arg.value.u16 as i64),
            DataType::UInt32 => Ok(origin_arg.value.u32 as i64),
            DataType::UInt64 => Ok(origin_arg.value.u64 as i64),
            DataType::Int8 => Ok(origin_arg.value.i8 as i64),
            DataType::Int16 => Ok(origin_arg.value.i16 as i64),
            DataType::Int32 => Ok(origin_arg.value.i32 as i64),
            DataType::Int64 => Ok(origin_arg.value.i64),
            // 字符串形式的时间戳（如'2020-01-01'）
            DataType::VarChar | DataType::Char | DataType::Text => {
                let origin_str = core::str::from_utf8(&origin_arg.value.string)
                    .map_err(|_| QueryExecutionError::TypeMismatch)?
                    .trim_end_matches(char::from(0));

                // 尝试解析为时间字符串
                parse_time_string(origin_str)
            }
            // 时间类型
            DataType::Timestamp => Ok(origin_arg.value.time.value),
            DataType::TimestampTZ => Ok(origin_arg.value.time.value),
            _ => Err(QueryExecutionError::TypeMismatch),
        }
    }
}

/// 解析时间间隔参数
pub fn parse_time_interval(interval_arg: &TypedValue) -> Result<i64, QueryExecutionError> {
    unsafe {
        match interval_arg.value_type {
            // 数值形式的时间间隔（微秒）
            DataType::UInt8 => Ok(interval_arg.value.u8 as i64),
            DataType::UInt16 => Ok(interval_arg.value.u16 as i64),
            DataType::UInt32 => Ok(interval_arg.value.u32 as i64),
            DataType::UInt64 => Ok(interval_arg.value.u64 as i64),
            DataType::Int8 => Ok(interval_arg.value.i8 as i64),
            DataType::Int16 => Ok(interval_arg.value.i16 as i64),
            DataType::Int32 => Ok(interval_arg.value.i32 as i64),
            DataType::Int64 => Ok(interval_arg.value.i64),
            DataType::Float32 => Ok(interval_arg.value.float32 as i64),
            DataType::Float64 => Ok(interval_arg.value.float64 as i64),
            // 字符串形式的时间间隔，如'5 minutes'、'1 hour'等
            DataType::VarChar | DataType::Char | DataType::Text => {
                let interval_str = core::str::from_utf8(&interval_arg.value.string)
                    .map_err(|_| QueryExecutionError::TypeMismatch)?
                    .trim_end_matches(char::from(0));

                parse_interval_string(interval_str)
            }
            _ => Err(QueryExecutionError::TypeMismatch),
        }
    }
}

/// 解析时间间隔字符串
pub fn parse_interval_string(interval_str: &str) -> Result<i64, QueryExecutionError> {
    let interval_str = interval_str.trim().to_lowercase();
    let mut parts = interval_str.split_whitespace();

    let value_str = parts.next().ok_or(QueryExecutionError::TypeMismatch)?;

    // Try to parse as "value unit" (space-separated, e.g. "15 minutes")
    if let Ok(num) = value_str.parse::<f64>() {
        let unit = parts.next().unwrap_or("");

        let micros_per_unit: i64 = match unit {
            "microsecond" | "microseconds" | "us" => 1,
            "millisecond" | "milliseconds" | "ms" => 1000,
            "second" | "seconds" | "s" => 1_000_000,
            "minute" | "minutes" | "m" => 60_000_000,
            "hour" | "hours" | "h" => 3_600_000_000,
            "day" | "days" | "d" => 86_400_000_000,
            "week" | "weeks" | "w" => 604_800_000_000,
            "month" | "months" => 2_592_000_000_000, // Approximate 30 days
            "year" | "years" => 31_556_952_000_000,  // Approximate 365.25 days
            // 如果没有单位，尝试将整个值解析为毫秒
            "" => {
                let ms = value_str
                    .parse::<i64>()
                    .map_err(|_| QueryExecutionError::TypeMismatch)?;
                return Ok(ms * 1000);
            }
            _ => return Err(QueryExecutionError::TypeMismatch),
        };

        return Ok((num * micros_per_unit as f64) as i64);
    }

    // Try to parse combined format like "15m", "1h", "30s", "500ms", etc.
    let num_str = value_str
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect::<String>();
    let unit_str = value_str
        .chars()
        .skip_while(|c| c.is_ascii_digit() || *c == '.')
        .collect::<String>();

    if num_str.is_empty() || unit_str.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let num = num_str
        .parse::<f64>()
        .map_err(|_| QueryExecutionError::TypeMismatch)?;
    let micros_per_unit: i64 = match unit_str.as_str() {
        "us" => 1,
        "ms" => 1000,
        "s" => 1_000_000,
        "m" => 60_000_000,
        "h" => 3_600_000_000,
        "d" => 86_400_000_000,
        "w" => 604_800_000_000,
        _ => return Err(QueryExecutionError::TypeMismatch),
    };

    Ok((num * micros_per_unit as f64) as i64)
}

// Helper functions for TO_ISO8601, TO_CHAR, TO_EPOCH
pub fn process_to_iso8601(
    timestamp: &crate::types::db_timestamp,
) -> Result<String, QueryExecutionError> {
    let microseconds = timestamp.value;
    let tz_offset = timestamp.tz_offset;

    // Convert microseconds to seconds for time calculation
    let total_seconds = microseconds / 1_000_000;

    // Apply timezone offset
    let adjusted_seconds = total_seconds + tz_offset as i64;

    // Calculate date and time components
    let days = adjusted_seconds / 86_400;
    let remaining_seconds = (adjusted_seconds % 86_400) + 86_400; // Ensure positive
    let adjusted_days = days + (remaining_seconds / 86_400) - 1;
    let final_seconds = remaining_seconds % 86_400;

    // Unix epoch: 1970-01-01
    // Approximate calculation (simplified)
    let year = 1970 + (adjusted_days / 365);

    // Simplified month/day calculation (not handling leap years properly)
    let day_of_year = (adjusted_days % 365) as i32;
    let month = (day_of_year / 30) + 1;
    let day = (day_of_year % 30) + 1;

    let hour = (final_seconds / 3600) as u8;
    let minute = ((final_seconds % 3600) / 60) as u8;
    let second = (final_seconds % 60) as u8;

    let micros = (microseconds % 1_000_000) as u32;

    // Format timezone offset
    let tz_hours = (tz_offset.abs() / 3600) as u8;
    let tz_minutes = ((tz_offset.abs() % 3600) / 60) as u8;
    let tz_sign = if tz_offset >= 0 { '+' } else { '-' };

    // Format the result
    if micros > 0 {
        Ok(alloc::format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:06}{}{:02}:{:02}",
            year,
            month,
            day,
            hour,
            minute,
            second,
            micros,
            tz_sign,
            tz_hours,
            tz_minutes
        ))
    } else {
        Ok(alloc::format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{}{:02}:{:02}",
            year,
            month,
            day,
            hour,
            minute,
            second,
            tz_sign,
            tz_hours,
            tz_minutes
        ))
    }
}

pub fn process_to_char(
    timestamp: &crate::types::db_timestamp,
    format_str: &str,
) -> Result<String, QueryExecutionError> {
    let microseconds = timestamp.value;

    let total_seconds = microseconds / 1_000_000;
    let days = total_seconds / 86_400;
    let remaining_seconds = total_seconds % 86_400;

    let year = 1970 + (days / 365);
    let day_of_year = (days % 365) as i32;
    let month = (day_of_year / 30) + 1;
    let day = (day_of_year % 30) + 1;

    let hour = (remaining_seconds / 3600) as u8;
    let minute = ((remaining_seconds % 3600) / 60) as u8;
    let second = (remaining_seconds % 60) as u8;

    let mut result = format_str.to_string();

    // Simple format replacements
    result = result.replace("YYYY", &alloc::format!("{:04}", year));
    result = result.replace("YY", &alloc::format!("{:02}", year % 100));
    result = result.replace("MM", &alloc::format!("{:02}", month));
    result = result.replace("DD", &alloc::format!("{:02}", day));
    result = result.replace("HH24", &alloc::format!("{:02}", hour));
    result = result.replace("MI", &alloc::format!("{:02}", minute));
    result = result.replace("SS", &alloc::format!("{:02}", second));

    Ok(result)
}

pub fn process_to_epoch(
    timestamp: &crate::types::db_timestamp,
) -> Result<f64, QueryExecutionError> {
    let microseconds = timestamp.value;
    let seconds = microseconds as f64 / 1_000_000.0;
    Ok(seconds)
}
