//! SQL Vector Operations
//!
//! This module contains vector distance calculation functions.

use alloc::vec::Vec;

/// 计算向量L2距离（欧几里得距离）
pub fn calculate_vector_l2_distance(vec1: *const f32, vec2: &[f64], dimension: u16) -> f64 {
    let mut distance = 0.0;
    for i in 0..dimension as usize {
        unsafe {
            let diff = *vec1.add(i) as f64 - vec2[i];
            distance += diff * diff;
        }
    }
    distance.sqrt()
}

/// 计算向量内积
pub fn calculate_vector_inner_product(vec1: *const f32, vec2: &[f64], dimension: u16) -> f64 {
    let mut product = 0.0;
    for i in 0..dimension as usize {
        unsafe {
            product += *vec1.add(i) as f64 * vec2[i];
        }
    }
    product
}

/// 计算向量余弦相似度
pub fn calculate_vector_cosine_similarity(vec1: *const f32, vec2: &[f64], dimension: u16) -> f64 {
    let mut dot_product = 0.0;
    let mut norm1 = 0.0;
    let mut norm2 = 0.0;

    for i in 0..dimension as usize {
        unsafe {
            let v1 = *vec1.add(i) as f64;
            let v2 = vec2[i];
            dot_product += v1 * v2;
            norm1 += v1 * v1;
            norm2 += v2 * v2;
        }
    }

    if norm1 == 0.0 || norm2 == 0.0 {
        0.0
    } else {
        dot_product / (norm1.sqrt() * norm2.sqrt())
    }
}

/// 解析向量距离表达式，提取向量字段名和比较向量
pub fn parse_vector_distance_expression(expr: &str) -> Option<(String, &'static str, Vec<f64>)> {
    // 支持的向量操作符
    if let Some(op_pos) = expr.find("<->") {
        return parse_vector_op(expr, op_pos, "<->");
    }
    if let Some(op_pos) = expr.find("<#>") {
        return parse_vector_op(expr, op_pos, "<#>");
    }
    if let Some(op_pos) = expr.find("<=>") {
        return parse_vector_op(expr, op_pos, "<=>");
    }

    None
}

/// 解析特定向量操作符的表达式
fn parse_vector_op(
    expr: &str,
    op_pos: usize,
    op: &'static str,
) -> Option<(String, &'static str, Vec<f64>)> {
    // 提取向量字段名
    let field_name = expr[..op_pos].trim().to_string();

    // 提取比较向量部分
    let vec_part = expr[op_pos + op.len()..].trim();

    // 解析向量字符串，如 "[1.0, 2.0, 3.0]"
    if vec_part.starts_with('[') && vec_part.ends_with(']') {
        let vec_str = &vec_part[1..vec_part.len() - 1];
        let vec_values: Result<Vec<f64>, _> = vec_str
            .split(',')
            .map(|s| s.trim().parse::<f64>())
            .collect();

        if let Ok(vec) = vec_values {
            return Some((field_name, op, vec));
        }
    }

    // 如果解析失败，返回None
    None
}
