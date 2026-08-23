use core::mem::size_of;

/// 压缩方案枚举
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CompressionScheme {
    /// 不压缩，直接存储原始float32
    None = 0,
    /// Float16压缩，压缩率50%
    Float16 = 1,
    /// ZSTD压缩，压缩率60-70%
    Zstd = 2,
}

/// 获取当前向量压缩配置（简化版，直接返回默认配置）
pub fn get_vector_compression_config() -> CompressionConfig {
    CompressionConfig {
        vector_compression_enabled: false,
        vector_compression_scheme: CompressionScheme::None,
        vector_compression_level: 3,
    }
}

/// 压缩配置结构体
#[derive(Clone, Copy, Debug)]
pub struct CompressionConfig {
    /// 全局向量压缩开关
    pub vector_compression_enabled: bool,
    /// 向量压缩方案
    pub vector_compression_scheme: CompressionScheme,
    /// 压缩级别（1-9）
    pub vector_compression_level: u8,
}

/// Float16 类型定义（IEEE 754 半精度浮点数）
#[repr(C)]
pub struct Float16(u16);

impl Float16 {
    /// 将 f32 转换为 f16
    pub fn from_f32(value: f32) -> Self {
        let bits = value.to_bits();
        let sign = (bits >> 16) & 0x8000;
        let mut exponent = ((bits >> 23) & 0xFF) as i16;
        let mantissa = bits & 0x7FFFFF;

        exponent = exponent - 127 + 15;

        let result = if exponent <= 0 {
            // 非规范化数或零
            if exponent < -10 {
                // 太小，直接归零
                sign
            } else {
                // 非规范化数
                let mantissa = (mantissa | 0x800000) >> (1 - exponent);
                sign | (mantissa >> 13)
            }
        } else if exponent >= 31 {
            // 无穷大或NaN
            if mantissa == 0 {
                // 无穷大
                sign | 0x7C00
            } else {
                // NaN
                sign | 0x7FFF
            }
        } else {
            // 规范化数
            sign | ((exponent as u32) << 10) | (mantissa >> 13)
        };

        Float16(result as u16)
    }

    /// 将 f16 转换为 f32
    pub fn to_f32(self) -> f32 {
        let bits = self.0;
        let sign = (bits as u32) << 16;
        let exponent = ((bits >> 10) & 0x1F) as i16;
        let mantissa = bits & 0x3FF;

        let result = if exponent == 0 {
            // 非规范化数或零
            if mantissa == 0 {
                // 零
                sign
            } else {
                // 非规范化数
                let exponent = -14;
                let mantissa = mantissa << 13;
                sign | ((exponent + 127) as u32) << 23 | (mantissa as u32)
            }
        } else if exponent == 0x1F {
            // 无穷大或NaN
            sign | 0x7F800000 | ((mantissa << 13) as u32)
        } else {
            // 规范化数
            let exponent = exponent - 15 + 127;
            sign | ((exponent as u32) << 23) | ((mantissa << 13) as u32)
        };

        f32::from_bits(result)
    }
}

/// 压缩向量数据（简化版，当前仅支持Float16压缩）
pub fn compress_vector(input: *const f32, dimension: usize, output: *mut u8) -> usize {
    // 检查是否启用压缩（暂时硬编码为false，后续从系统表读取）
    let is_compressed = false;
    let compression_scheme = 0; // 0=不压缩, 1=float16, 2=zstd

    if is_compressed {
        match compression_scheme {
            1 => {
                // Float16 压缩
                unsafe {
                    for i in 0..dimension {
                        let f32_val = *input.add(i);
                        let f16_val = Float16::from_f32(f32_val);
                        let output_ptr = output.add(i * size_of::<u16>()) as *mut u16;
                        *output_ptr = f16_val.0;
                    }
                }
                dimension * size_of::<u16>()
            }
            2 => {
                // ZSTD 压缩（预留空间，暂时返回原始大小）
                // 实际实现需要引入 ZSTD 库支持
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        input as *const u8,
                        output.add(4),
                        dimension * size_of::<f32>(),
                    );
                    // 前4字节存储压缩大小（这里暂时存储原始大小）
                    let size_ptr = output as *mut u32;
                    *size_ptr = dimension as u32 * size_of::<f32>() as u32;
                }
                dimension * size_of::<f32>() + 4
            }
            _ => {
                // 默认不压缩
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        input as *const u8,
                        output,
                        dimension * size_of::<f32>(),
                    );
                }
                dimension * size_of::<f32>()
            }
        }
    } else {
        // 不压缩，直接拷贝原始数据
        unsafe {
            core::ptr::copy_nonoverlapping(
                input as *const u8,
                output,
                dimension * size_of::<f32>(),
            );
        }
        dimension * size_of::<f32>()
    }
}

/// 解压缩向量数据（简化版，当前仅支持Float16解压缩）
pub fn decompress_vector(input: *const u8, dimension: usize, output: *mut f32) {
    // 检查是否启用压缩（暂时硬编码为false，后续从系统表读取）
    let is_compressed = false;
    let compression_scheme = 0; // 0=不压缩, 1=float16, 2=zstd

    if is_compressed {
        match compression_scheme {
            1 => {
                // Float16 解压缩
                unsafe {
                    for i in 0..dimension {
                        let input_ptr = input.add(i * size_of::<u16>()) as *const u16;
                        let f16_val = Float16(*input_ptr);
                        *output.add(i) = f16_val.to_f32();
                    }
                }
            }
            2 => {
                // ZSTD 解压缩（预留空间，暂时直接拷贝）
                // 实际实现需要引入 ZSTD 库支持
                unsafe {
                    core::ptr::copy_nonoverlapping(input.add(4) as *const f32, output, dimension);
                }
            }
            _ => {
                // 默认不压缩，直接拷贝原始数据
                unsafe {
                    core::ptr::copy_nonoverlapping(input as *const f32, output, dimension);
                }
            }
        }
    } else {
        // 不压缩，直接拷贝原始数据
        unsafe {
            core::ptr::copy_nonoverlapping(input as *const f32, output, dimension);
        }
    }
}

/// 获取压缩后的数据大小
pub fn get_compressed_size(dimension: usize) -> usize {
    // 检查是否启用压缩（暂时硬编码为false，后续从系统表读取）
    let is_compressed = false;
    let compression_scheme = 0; // 0=不压缩, 1=float16, 2=zstd

    if is_compressed {
        match compression_scheme {
            1 => dimension * size_of::<u16>(),
            2 => dimension * size_of::<f32>() + 4,
            _ => dimension * size_of::<f32>(),
        }
    } else {
        dimension * size_of::<f32>()
    }
}

/// 检查是否启用了向量压缩
pub fn is_vector_compression_enabled() -> bool {
    // 暂时硬编码为false，后续从系统表读取
    false
}

/// 获取当前压缩方案（返回u8值：0=不压缩, 1=float16, 2=zstd）
pub fn get_current_compression_scheme() -> u8 {
    // 暂时硬编码为0，后续从系统表读取
    0
}

/// 测试压缩/解压缩的正确性
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_float16_conversion() {
        // 测试基本数值
        let test_values = [0.0f32, 1.0f32, -1.0f32, 0.5f32, 2.0f32];
        for val in test_values {
            let f16 = Float16::from_f32(val);
            let f32 = f16.to_f32();
            // 允许一定的误差（float16精度有限）
            assert!((f32 - val).abs() < 0.001 || (val == 0.0 && f32 == 0.0));
        }
    }

    #[test]
    fn test_vector_compression() {
        // 创建测试向量
        let dimension = 4;
        let test_vector = [1.0f32, 2.0f32, 3.0f32, 4.0f32];
        // 测试压缩
        let mut compressed = [0u8; 16]; // 4*4=16字节
        let compressed_size =
            compress_vector(test_vector.as_ptr(), dimension, compressed.as_mut_ptr());
        // 测试解压缩
        let mut decompressed = [0.0f32; 4];
        decompress_vector(compressed.as_ptr(), dimension, decompressed.as_mut_ptr());
        // 验证结果
        for (i, (orig, decomp)) in test_vector.iter().zip(decompressed.iter()).enumerate() {
            assert!(
                (orig - decomp).abs() < 0.001,
                "Index {}: orig={}, decomp={}",
                i,
                orig,
                decomp
            );
        }
    }
}
