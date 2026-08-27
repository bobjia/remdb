//! SQL时序表查询操作
//!
//! 该模块包含时序表的SELECT查询执行、降采样和插值逻辑。

use crate::sql::query_parser::Expression;
use crate::sql::utils::estimate_memory_usage;
use crate::sql::{check_memory_limit, QueryExecutionError, ResultSet, SqlQuery};
use crate::types::{DataType, TypedValue};
use crate::{RemDb, TimeSeriesTable, Value};
use alloc::string::ToString;
use alloc::vec::Vec;
/// 评估时序表表达式
fn evaluate_timeseries_expression(
    expr: &crate::sql::query_parser::Expression,
    record: &crate::time_series::TimeSeriesRecord,
    ts_table: &crate::time_series::TimeSeriesTable,
) -> Result<TypedValue, QueryExecutionError> {
    match expr {
        crate::sql::query_parser::Expression::Field { name, .. } => {
            // 查找字段索引
            for (i, field) in ts_table.def.base.fields.iter().enumerate() {
                if field.name == *name {
                    if i == ts_table.def.time_field {
                        // 时间字段
                        return Ok(TypedValue {
                            value_type: DataType::UInt64,
                            value: Value {
                                u64: record.timestamp,
                            },
                        });
                    } else if i == ts_table.def.value_field {
                        // 值字段
                        return Ok(TypedValue {
                            value_type: DataType::Float64,
                            value: Value {
                                float64: record.value,
                            },
                        });
                    } else {
                        // 标签字段（简化处理，暂时返回0）
                        return Ok(TypedValue {
                            value_type: DataType::UInt64,
                            value: Value { u64: 0 },
                        });
                    }
                }
            }
            Err(QueryExecutionError::FieldNotFound)
        }
        crate::sql::query_parser::Expression::FunctionCall { name, args, .. } => {
            let func_name = name.to_uppercase();
            // 简化实现，仅支持基本聚合函数
            match func_name.as_str() {
                "AVG" | "SUM" | "MIN" | "MAX" | "COUNT" => {
                    // 对于单条记录，这些函数返回记录值
                    evaluate_timeseries_expression(&args[0], record, ts_table)
                }
                _ => Err(QueryExecutionError::UnsupportedFunction(name.clone())),
            }
        }
        _ => {
            // 其他表达式类型暂不支持
            Err(QueryExecutionError::UnsupportedFunction(
                "Complex expression in timeseries query".to_string(),
            ))
        }
    }
}

/// 解析SAMPLE BY时间间隔字符串，如"1h"、"5m"、"30s"
fn parse_sample_interval(interval_str: &str) -> Result<u64, QueryExecutionError> {
    let mut total_seconds = 0;
    let mut current_number = 0;

    for ch in interval_str.chars() {
        if ch.is_ascii_digit() {
            current_number = current_number * 10 + (ch as u64 - '0' as u64);
        } else {
            match ch.to_ascii_lowercase() {
                'h' => total_seconds += current_number * 3600,
                'm' => total_seconds += current_number * 60,
                's' => total_seconds += current_number,
                _ => return Err(QueryExecutionError::InvalidValue),
            }
            current_number = 0;
        }
    }

    // 处理末尾没有单位的情况（默认为秒）
    if current_number > 0 {
        total_seconds += current_number;
    }

    if total_seconds == 0 {
        return Err(QueryExecutionError::InvalidValue);
    }

    Ok(total_seconds)
}

/// 插值缺失的时间窗口
fn interpolate_missing_window(
    window_start: u64,
    prev_data: &Option<(u64, f64, u8, [u64; 8])>,
    next_data: Option<(u64, &&Vec<&crate::time_series::TimeSeriesRecord>)>,
    fill_clause: &crate::sql::query_parser::FillClause,
) -> Option<crate::time_series::TimeSeriesRecord> {
    match fill_clause {
        crate::sql::query_parser::FillClause::Prev => {
            if let Some((_prev_ts, prev_val, prev_tag_count, prev_tags)) = prev_data {
                Some(crate::time_series::TimeSeriesRecord {
                    timestamp: window_start,
                    value: *prev_val,
                    tag_count: *prev_tag_count,
                    tags: *prev_tags,
                })
            } else {
                None
            }
        }
        crate::sql::query_parser::FillClause::Next => {
            if let Some((_next_ts, next_records)) = next_data {
                if !next_records.is_empty() {
                    let first_record = next_records[0];
                    Some(crate::time_series::TimeSeriesRecord {
                        timestamp: window_start,
                        value: first_record.value,
                        tag_count: first_record.tag_count,
                        tags: first_record.tags,
                    })
                } else {
                    None
                }
            } else {
                None
            }
        }
        crate::sql::query_parser::FillClause::Linear => {
            match (prev_data, next_data) {
                (
                    Some((prev_ts, prev_val, prev_tag_count, prev_tags)),
                    Some((next_ts, next_records)),
                ) => {
                    if !next_records.is_empty() {
                        let first_next_record = next_records[0];
                        let time_ratio =
                            (window_start - prev_ts) as f64 / (next_ts - prev_ts) as f64;
                        let interpolated_value =
                            prev_val + (first_next_record.value - prev_val) * time_ratio;

                        Some(crate::time_series::TimeSeriesRecord {
                            timestamp: window_start,
                            value: interpolated_value,
                            tag_count: *prev_tag_count, // 使用前一个窗口的标签
                            tags: *prev_tags,
                        })
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }
        crate::sql::query_parser::FillClause::FixedValue(value) => {
            // 对于固定值，需要从现有记录中获取标签信息
            if let Some((_prev_ts, _prev_val, prev_tag_count, prev_tags)) = prev_data {
                Some(crate::time_series::TimeSeriesRecord {
                    timestamp: window_start,
                    value: *value,
                    tag_count: *prev_tag_count,
                    tags: *prev_tags,
                })
            } else if let Some((_next_ts, next_records)) = next_data {
                if !next_records.is_empty() {
                    let first_record = next_records[0];
                    Some(crate::time_series::TimeSeriesRecord {
                        timestamp: window_start,
                        value: *value,
                        tag_count: first_record.tag_count,
                        tags: first_record.tags,
                    })
                } else {
                    None
                }
            } else {
                None
            }
        }
    }
}

/// 对时序记录进行降采样
fn downsample_records(
    records: &[crate::time_series::TimeSeriesRecord],
    sample_interval: &str,
    fill_clause: Option<&crate::sql::query_parser::FillClause>,
) -> Result<Vec<crate::time_series::TimeSeriesRecord>, QueryExecutionError> {
    if records.is_empty() {
        return Ok(Vec::new());
    }

    // 解析时间间隔
    let interval_seconds = parse_sample_interval(sample_interval)?;
    let interval_nanos = interval_seconds * 1_000_000_000u64;

    // 找到最小和最大时间戳
    let min_timestamp = records
        .iter()
        .map(|r| r.timestamp)
        .min()
        .expect("records must not be empty");
    let max_timestamp = records
        .iter()
        .map(|r| r.timestamp)
        .max()
        .expect("records must not be empty");

    // 按时间窗口分组
    let mut windows: std::collections::BTreeMap<u64, Vec<&crate::time_series::TimeSeriesRecord>> =
        std::collections::BTreeMap::new();

    for record in records {
        let window_start = (record.timestamp / interval_nanos) * interval_nanos;
        windows.entry(window_start).or_default().push(record);
    }

    // 确定窗口范围
    let first_window = (min_timestamp / interval_nanos) * interval_nanos;
    let last_window = (max_timestamp / interval_nanos) * interval_nanos;

    // 为每个窗口生成降采样记录（包括空窗口）
    let mut result = Vec::new();
    let mut prev_window_data: Option<(u64, f64, u8, [u64; 8])> = None;
    let mut next_window_iter = windows.iter().peekable();

    let mut current_window = first_window;
    while current_window <= last_window {
        if let Some((&window_start, _window_records)) = next_window_iter.peek() {
            if window_start == current_window {
                // 当前窗口有数据
                let window_records = next_window_iter
                    .next()
                    .expect("window iterator must have elements")
                    .1;

                // 计算窗口内记录的平均值（优化版本，减少迭代次数）
                let (sum, count) = window_records
                    .iter()
                    .fold((0.0, 0), |(sum, count), record| {
                        (sum + record.value, count + 1)
                    });
                let avg_value: f64 = sum / count as f64;

                // 使用第一个记录的标签
                let first_record = window_records[0];

                result.push(crate::time_series::TimeSeriesRecord {
                    timestamp: current_window,
                    value: avg_value,
                    tag_count: first_record.tag_count,
                    tags: first_record.tags,
                });

                // 保存为前一个窗口数据（用于PREV插值）
                prev_window_data = Some((
                    current_window,
                    avg_value,
                    first_record.tag_count,
                    first_record.tags,
                ));
            } else {
                // 当前窗口无数据，需要插值
                if let Some(fill_clause) = fill_clause {
                    if let Some(record) = interpolate_missing_window(
                        current_window,
                        &prev_window_data,
                        next_window_iter.peek().map(|(&ts, recs)| (ts, recs)),
                        fill_clause,
                    ) {
                        result.push(record);
                    }
                }
                // 如果没有指定FILL子句，则跳过空窗口
            }
        } else {
            // 后续所有窗口都无数据
            if let Some(fill_clause) = fill_clause {
                if let Some(record) =
                    interpolate_missing_window(current_window, &prev_window_data, None, fill_clause)
                {
                    result.push(record);
                }
            }
        }

        current_window += interval_nanos;
    }

    Ok(result)
}

/// 执行时序表SELECT查询
pub fn execute_select_timeseries_query(
    db: &mut RemDb,
    query: &SqlQuery,
) -> Result<ResultSet, QueryExecutionError> {
    // 1. 查找要查询的时序表
    let ts_table = find_timeseries_table_by_name(db, &query.table_name)?;

    // 2. 确定要返回的列表达式
    let columns = if query.select_all {
        // 返回所有列（作为Field表达式）
        ts_table
            .def
            .base
            .fields
            .iter()
            .map(|field| Expression::Field {
                name: field.name.to_string(),
                alias: None,
            })
            .collect()
    } else {
        // 对于时序表，我们暂时只支持简单字段选择
        // TODO: 实现完整的表达式支持
        query.columns.clone()
    };

    // 3. 生成结果集的列名
    let result_columns = columns
        .iter()
        .map(|expr| match expr {
            Expression::Field { name, alias } => alias.clone().unwrap_or_else(|| name.clone()),
            Expression::FunctionCall { alias, name, .. } => {
                alias.clone().unwrap_or_else(|| name.clone())
            }
            Expression::Constant { alias, .. } => {
                alias.clone().unwrap_or_else(|| "constant".to_string())
            }
            Expression::BinaryOp { alias, .. } => {
                alias.clone().unwrap_or_else(|| "binary_op".to_string())
            }
            Expression::LogicalOp { alias, .. } => {
                alias.clone().unwrap_or_else(|| "logical_op".to_string())
            }
            Expression::UnaryOp { alias, .. } => {
                alias.clone().unwrap_or_else(|| "unary_op".to_string())
            }
        })
        .collect();

    // 4. 创建别名映射
    let mut alias_map = alloc::collections::BTreeMap::new();
    for expr in &columns {
        match expr {
            Expression::Field { name: _, alias } => {
                if let Some(alias) = alias {
                    alias_map.insert(alias.clone(), expr);
                }
            }
            Expression::FunctionCall { alias, .. } => {
                if let Some(alias) = alias {
                    alias_map.insert(alias.clone(), expr);
                }
            }
            Expression::Constant { alias, .. } => {
                if let Some(alias) = alias {
                    alias_map.insert(alias.clone(), expr);
                }
            }
            Expression::BinaryOp { alias, .. } => {
                if let Some(alias) = alias {
                    alias_map.insert(alias.clone(), expr);
                }
            }
            Expression::LogicalOp { alias, .. } => {
                if let Some(alias) = alias {
                    alias_map.insert(alias.clone(), expr);
                }
            }
            Expression::UnaryOp { alias, .. } => {
                if let Some(alias) = alias {
                    alias_map.insert(alias.clone(), expr);
                }
            }
        }
    }

    // 5. 创建结果集
    let mut result_set = ResultSet::new(result_columns);

    // 5. 提取时间范围条件
    let (start_time, end_time) = if let Some(where_clause) = &query.where_clause {
        extract_time_range_from_condition(&where_clause.condition, ts_table)?
    } else {
        // 如果没有WHERE条件，查询所有数据
        (0, u64::MAX)
    };

    // 6. 执行时间范围查询
    let raw_records = ts_table
        .query_time_range(start_time, end_time)
        .map_err(|_| QueryExecutionError::InternalError)?;

    // 6.1 内存使用检查
    let estimated_memory = estimate_memory_usage(&raw_records);
    // 从系统表获取内存限制
    let (max_memory_mb, _) = crate::get_query_resource_config();
    check_memory_limit(estimated_memory, Some(max_memory_mb))?;

    // 7. 应用SAMPLE BY降采样（如果指定）
    let sampled_records = if let Some(sample_interval) = &query.sample_by {
        downsample_records(&raw_records, sample_interval, query.fill_clause.as_ref())?
    } else {
        raw_records
    };

    // 8. 转换为TypedValue并添加到结果集
    for record in sampled_records {
        let mut row_data = Vec::with_capacity(columns.len());
        for expr in &columns {
            let value = evaluate_timeseries_expression(expr, &record, ts_table)?;
            row_data.push(value);
        }
        result_set.add_row(row_data);
    }

    // 注意：execute_select_timeseries_query函数中没有stats和start_time变量，暂时注释掉统计信息
    /*
    // 计算执行时间
    let end_time = Instant::now();
    _stats.execution_time = end_time.duration_since(start_time).as_micros() as u64;

    // 输出查询执行统计信息
    #[cfg(feature = "log")]
    {
        info!("Query execution stats:");
        info!("  Used index: {}", _stats.used_index);
        info!("  Scanned records: {}", _stats.scanned_records);
        info!("  Matched records: {}", _stats.matched_records);
        info!("  Execution time: {}μs", _stats.execution_time);
    }
    */

    Ok(result_set)
}

/// 从WHERE条件中提取时间范围
fn extract_time_range_from_condition(
    condition: &crate::sql::query_parser::Condition,
    ts_table: &crate::time_series::TimeSeriesTable,
) -> Result<(u64, u64), QueryExecutionError> {
    use crate::sql::query_parser::ComparisonOperator;

    // 获取时间字段名称
    let time_field_name = ts_table.def.base.fields[ts_table.def.time_field]
        .name
        .clone();

    // 递归解析条件
    fn extract_from_condition(
        condition: &crate::sql::query_parser::Condition,
        time_field_name: &str,
    ) -> Result<(Option<u64>, Option<u64>), QueryExecutionError> {
        match condition {
            crate::sql::query_parser::Condition::Comparison(comp) => {
                if comp.field == time_field_name {
                    match comp.operator {
                        ComparisonOperator::GreaterThan
                        | ComparisonOperator::GreaterThanOrEqual => {
                            if let crate::sql::query_parser::Value::Integer(value) = comp.value {
                                Ok((Some(value as u64), None))
                            } else {
                                Err(QueryExecutionError::InvalidCondition)
                            }
                        }
                        ComparisonOperator::LessThan | ComparisonOperator::LessThanOrEqual => {
                            if let crate::sql::query_parser::Value::Integer(value) = comp.value {
                                Ok((None, Some(value as u64)))
                            } else {
                                Err(QueryExecutionError::InvalidCondition)
                            }
                        }
                        ComparisonOperator::Equal => {
                            if let crate::sql::query_parser::Value::Integer(value) = comp.value {
                                Ok((Some(value as u64), Some(value as u64)))
                            } else {
                                Err(QueryExecutionError::InvalidCondition)
                            }
                        }
                        _ => Ok((None, None)),
                    }
                } else {
                    Ok((None, None))
                }
            }
            crate::sql::query_parser::Condition::Between(between) => {
                if between.field == time_field_name {
                    if let (
                        crate::sql::query_parser::Value::Integer(min),
                        crate::sql::query_parser::Value::Integer(max),
                    ) = (&between.min_value, &between.max_value)
                    {
                        Ok((Some(*min as u64), Some(*max as u64)))
                    } else {
                        Err(QueryExecutionError::InvalidCondition)
                    }
                } else {
                    Ok((None, None))
                }
            }
            crate::sql::query_parser::Condition::And(left, right) => {
                let (left_min, left_max) = extract_from_condition(left, time_field_name)?;
                let (right_min, right_max) = extract_from_condition(right, time_field_name)?;

                let min = left_min.or(right_min);
                let max = left_max.or(right_max);
                Ok((min, max))
            }
            crate::sql::query_parser::Condition::Or(_, _) => {
                // OR条件不能简单合并，暂时不支持
                Err(QueryExecutionError::UnsupportedFunction(
                    "OR conditions in time range extraction".to_string(),
                ))
            }
            crate::sql::query_parser::Condition::Not(_) => {
                // NOT条件不支持
                Err(QueryExecutionError::UnsupportedFunction(
                    "NOT conditions in time range extraction".to_string(),
                ))
            }
        }
    }

    let (min_opt, max_opt) = extract_from_condition(condition, &time_field_name)?;

    let start_time = min_opt.unwrap_or(0);
    let end_time = max_opt.unwrap_or(u64::MAX);

    Ok((start_time, end_time))
}

/// 查找时序表
fn find_timeseries_table_by_name<'a>(
    db: &'a RemDb,
    table_name: &str,
) -> Result<&'a TimeSeriesTable, QueryExecutionError> {
    for table in db.time_series_tables.iter().flatten() {
        if table.def.base.name == table_name {
            return Ok(table);
        }
    }

    Err(QueryExecutionError::TableNotFound)
}

