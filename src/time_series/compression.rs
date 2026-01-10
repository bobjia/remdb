

use alloc::vec::Vec;

/// 压缩类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionType {
    /// 不压缩
    None,
    /// Delta编码
    Delta,
    /// 游程编码
    RunLength,
    /// Delta+游程编码
    DeltaRunLength,
    /// Delta-Delta编码
    DeltaDelta,
}

/// Delta编码压缩
pub fn compress_delta(values: &[u64]) -> Vec<u8> {
    let mut result = Vec::with_capacity(values.len() * 8);
    let mut last = 0;
    
    for &value in values {
        let delta = value - last;
        result.extend_from_slice(&delta.to_le_bytes());
        last = value;
    }
    
    result
}

/// Delta编码解压缩
pub fn decompress_delta(compressed: &[u8], count: usize) -> Vec<u64> {
    let mut result = Vec::with_capacity(count);
    let mut last = 0;
    
    for i in 0..count {
        let delta_bytes = &compressed[i * 8..(i + 1) * 8];
        let delta = u64::from_le_bytes(delta_bytes.try_into().unwrap());
        let value = last + delta;
        result.push(value);
        last = value;
    }
    
    result
}

/// 浮点数Delta编码压缩
pub fn compress_delta_float(values: &[f64]) -> Vec<u8> {
    let mut result = Vec::with_capacity(values.len() * 8);
    let mut last = 0.0;
    
    for &value in values {
        let delta = value - last;
        result.extend_from_slice(&delta.to_le_bytes());
        last = value;
    }
    
    result
}

/// 浮点数Delta编码解压缩
pub fn decompress_delta_float(compressed: &[u8], count: usize) -> Vec<f64> {
    let mut result = Vec::with_capacity(count);
    let mut last = 0.0;
    
    for i in 0..count {
        let delta_bytes = &compressed[i * 8..(i + 1) * 8];
        let delta = f64::from_le_bytes(delta_bytes.try_into().unwrap());
        let value = last + delta;
        result.push(value);
        last = value;
    }
    
    result
}

/// Delta-Delta编码压缩（用于u64，如时间戳）
pub fn compress_delta_delta(values: &[u64]) -> Vec<u8> {
    let mut result = Vec::with_capacity(values.len() * 8);
    
    if values.is_empty() {
        return result;
    }
    
    // 存储第一个值
    result.extend_from_slice(&values[0].to_le_bytes());
    
    if values.len() == 1 {
        return result;
    }
    
    // 存储第二个值的delta
    let delta1_0 = values[1] - values[0];
    result.extend_from_slice(&delta1_0.to_le_bytes());
    
    if values.len() == 2 {
        return result;
    }
    
    // 计算并存储delta-delta值
    let mut last_delta1 = delta1_0;
    
    for i in 2..values.len() {
        let delta1 = values[i] - values[i - 1];
        let delta2 = delta1 - last_delta1;
        result.extend_from_slice(&delta2.to_le_bytes());
        last_delta1 = delta1;
    }
    
    result
}

/// Delta-Delta编码解压缩（用于u64，如时间戳）
pub fn decompress_delta_delta(compressed: &[u8], count: usize) -> Vec<u64> {
    let mut result = Vec::with_capacity(count);
    
    if count == 0 || compressed.len() < 8 {
        return result;
    }
    
    // 读取第一个值
    let first = u64::from_le_bytes(compressed[0..8].try_into().unwrap());
    result.push(first);
    
    if count == 1 {
        return result;
    }
    
    if compressed.len() < 16 {
        return result;
    }
    
    // 读取第二个值的delta
    let delta1_0 = u64::from_le_bytes(compressed[8..16].try_into().unwrap());
    let second = first + delta1_0;
    result.push(second);
    
    if count == 2 {
        return result;
    }
    
    let mut current_delta1 = delta1_0;
    let mut last_value = second;
    
    for i in 2..count {
        let offset = 16 + (i - 2) * 8;
        if offset + 8 > compressed.len() {
            break;
        }
        
        let delta2 = u64::from_le_bytes(compressed[offset..offset + 8].try_into().unwrap());
        let current_delta1_new = current_delta1 + delta2;
        let current_value = last_value + current_delta1_new;
        
        result.push(current_value);
        last_value = current_value;
        current_delta1 = current_delta1_new;
    }
    
    result
}

/// 浮点数Delta-Delta编码压缩
pub fn compress_delta_delta_float(values: &[f64]) -> Vec<u8> {
    let mut result = Vec::with_capacity(values.len() * 8);
    
    if values.is_empty() {
        return result;
    }
    
    // 存储第一个值
    result.extend_from_slice(&values[0].to_le_bytes());
    
    if values.len() == 1 {
        return result;
    }
    
    // 存储第二个值的delta
    let delta1_0 = values[1] - values[0];
    result.extend_from_slice(&delta1_0.to_le_bytes());
    
    if values.len() == 2 {
        return result;
    }
    
    // 计算并存储delta-delta值
    let mut last_delta1 = delta1_0;
    
    for i in 2..values.len() {
        let delta1 = values[i] - values[i - 1];
        let delta2 = delta1 - last_delta1;
        result.extend_from_slice(&delta2.to_le_bytes());
        last_delta1 = delta1;
    }
    
    result
}

/// 浮点数Delta-Delta编码解压缩
pub fn decompress_delta_delta_float(compressed: &[u8], count: usize) -> Vec<f64> {
    let mut result = Vec::with_capacity(count);
    
    if count == 0 || compressed.len() < 8 {
        return result;
    }
    
    // 读取第一个值
    let first = f64::from_le_bytes(compressed[0..8].try_into().unwrap());
    result.push(first);
    
    if count == 1 {
        return result;
    }
    
    if compressed.len() < 16 {
        return result;
    }
    
    // 读取第二个值的delta
    let delta1_0 = f64::from_le_bytes(compressed[8..16].try_into().unwrap());
    let second = first + delta1_0;
    result.push(second);
    
    if count == 2 {
        return result;
    }
    
    let mut current_delta1 = delta1_0;
    let mut last_value = second;
    
    for i in 2..count {
        let offset = 16 + (i - 2) * 8;
        if offset + 8 > compressed.len() {
            break;
        }
        
        let delta2 = f64::from_le_bytes(compressed[offset..offset + 8].try_into().unwrap());
        let current_delta1_new = current_delta1 + delta2;
        let current_value = last_value + current_delta1_new;
        
        result.push(current_value);
        last_value = current_value;
        current_delta1 = current_delta1_new;
    }
    
    result
}

/// 游程编码压缩
pub fn compress_run_length(values: &[u64]) -> Vec<u8> {
    let mut result = Vec::new();
    
    if values.is_empty() {
        return result;
    }
    
    let mut current = values[0];
    let mut count = 1;
    
    for &value in &values[1..] {
        if value == current && count < u16::MAX as u32 {
            count += 1;
        } else {
            // 写入当前值和计数
            result.extend_from_slice(&current.to_le_bytes());
            result.extend_from_slice(&count.to_le_bytes());
            
            current = value;
            count = 1;
        }
    }
    
    // 写入最后一组值和计数
    result.extend_from_slice(&current.to_le_bytes());
    result.extend_from_slice(&count.to_le_bytes());
    
    result
}

/// 游程编码解压缩
pub fn decompress_run_length(compressed: &[u8]) -> Vec<u64> {
    let mut result = Vec::new();
    let mut index = 0;
    
    while index + 12 <= compressed.len() {
        let value = u64::from_le_bytes(compressed[index..index + 8].try_into().unwrap());
        let count = u32::from_le_bytes(compressed[index + 8..index + 12].try_into().unwrap());
        
        for _ in 0..count {
            result.push(value);
        }
        
        index += 12;
    }
    
    result
}
