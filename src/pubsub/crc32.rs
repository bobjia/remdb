// CRC32校验实现

// CRC32多项式（标准以太网多项式）
const CRC32_POLYNOMIAL: u32 = 0x04C11DB7;

// 预计算的CRC32表（使用const函数生成）
static CRC32_TABLE: [u32; 256] = generate_crc32_table();

// 生成CRC32表的const函数
const fn generate_crc32_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0;
    
    // 预计算CRC32表
    while i < 256 {
        let mut crc = (i as u32) << 24;
        let mut j = 0;
        
        while j < 8 {
            if crc & 0x80000000 != 0 {
                crc = (crc << 1) ^ CRC32_POLYNOMIAL;
            } else {
                crc <<= 1;
            }
            j += 1;
        }
        
        table[i] = crc;
        i += 1;
    }
    
    table
}

/// 计算数据的CRC32校验和
/// 
/// # 参数
/// - `data`: 要计算CRC的数据
/// 
/// # 返回值
/// - CRC32校验和
#[inline]
pub fn calculate_crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    
    for &byte in data {
        let index = ((crc >> 24) ^ (byte as u32)) as u8;
        crc = (crc << 8) ^ CRC32_TABLE[index as usize];
    }
    
    crc ^ 0xFFFFFFFF
}

/// 验证数据的CRC32校验和
/// 
/// # 参数
/// - `data`: 要验证的数据
/// - `expected_crc`: 期望的CRC32校验和
/// 
/// # 返回值
/// - 验证结果，true表示校验通过，false表示校验失败
#[inline]
pub fn verify_crc32(data: &[u8], expected_crc: u32) -> bool {
    calculate_crc32(data) == expected_crc
}

// 测试用例（仅在测试模式下编译）
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_crc32_calculation() {
        // 测试空数据
        let empty_data: &[u8] = &[];
        assert_eq!(calculate_crc32(empty_data), 0x00000000);
        
        // 测试简单数据
        let simple_data = b"Hello, world!";
        // 已知的CRC32值（使用标准CRC32算法计算）
        let expected_crc = 0xED07628B;
        assert_eq!(calculate_crc32(simple_data), expected_crc);
        assert!(verify_crc32(simple_data, expected_crc));
        
        // 测试不同数据
        let different_data = b"123456789";
        // 已知的CRC32值（使用标准CRC32算法计算）
        let expected_crc = 0xCBF43926;
        assert_eq!(calculate_crc32(different_data), expected_crc);
        assert!(verify_crc32(different_data, expected_crc));
    }
    
    #[test]
    fn test_crc32_verification() {
        let data = b"test data";
        let crc = calculate_crc32(data);
        
        // 正确的CRC应该验证通过
        assert!(verify_crc32(data, crc));
        
        // 错误的CRC应该验证失败
        assert!(!verify_crc32(data, crc + 1));
        assert!(!verify_crc32(data, crc - 1));
    }
}
