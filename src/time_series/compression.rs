#![cfg_attr(not(feature = "std"), no_std)]

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
    
    while index + 16 <= compressed.len() {
        let value = u64::from_le_bytes(compressed[index..index + 8].try_into().unwrap());
        let count = u32::from_le_bytes(compressed[index + 8..index + 12].try_into().unwrap());
        
        for _ in 0..count {
            result.push(value);
        }
        
        index += 16;
    }
    
    result
}
