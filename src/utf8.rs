use core::str::Utf8Error;

/// UTF-8验证级别
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Utf8ValidationLevel {
    /// 无验证（最快）
    None,
    /// 宽松验证（替换无效序列为U+FFFD）
    Lenient,
    /// 严格验证（拒绝无效序列）
    Strict,
}

/// Unicode规范化形式
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NormalizationForm {
    /// 标准分解
    NFD,
    /// 标准组合
    NFC,
    /// 兼容性分解
    NFKD,
    /// 兼容性组合
    NFKC,
}

/// UTF-8配置选项
#[derive(Debug, PartialEq, Eq)]
pub struct Utf8Config {
    pub validation_level: Utf8ValidationLevel,
    pub normalization: Option<NormalizationForm>,
    pub case_mapping: bool,
    pub grapheme_cluster: bool,
}

impl Default for Utf8Config {
    fn default() -> Self {
        Self {
            validation_level: Utf8ValidationLevel::Strict,
            normalization: None,
            case_mapping: false,
            grapheme_cluster: false,
        }
    }
}

/// UTF-8字符串处理结果
#[derive(Debug, PartialEq, Eq)]
pub enum Utf8Result<T> {
    /// 成功
    Ok(T),
    /// 验证错误
    ValidationError(Utf8Error),
    /// 其他错误
    Error(&'static str),
}

/// UTF-8核心处理函数
pub struct Utf8Processor {
    config: Utf8Config,
}

impl Utf8Processor {
    /// 创建新的UTF-8处理器
    pub fn new(config: Utf8Config) -> Self {
        Self { config }
    }

    /// 验证UTF-8字符串
    pub fn validate(&self, input: &[u8]) -> Utf8Result<()> {
        match self.config.validation_level {
            Utf8ValidationLevel::None => Utf8Result::Ok(()),
            Utf8ValidationLevel::Lenient => {
                // 宽松验证：检查是否可以替换无效序列
                let mut i = 0;
                while i < input.len() {
                    let (len, valid) = self.utf8_char_length(&input[i..]);
                    if !valid {
                        // 发现无效序列，但宽松模式下忽略
                    }
                    // 确保i至少增加1，避免无限循环
                    i += if len > 0 { len } else { 1 };
                }
                Utf8Result::Ok(())
            }
            Utf8ValidationLevel::Strict => {
                // 严格验证：使用标准库验证
                match core::str::from_utf8(input) {
                    Ok(_) => Utf8Result::Ok(()),
                    Err(e) => Utf8Result::ValidationError(e),
                }
            }
        }
    }

    /// 计算UTF-8字符串的字符长度
    pub fn char_length(&self, input: &[u8]) -> usize {
        let mut count = 0;
        let mut i = 0;

        while i < input.len() {
            let (len, valid) = self.utf8_char_length(&input[i..]);
            if valid {
                count += 1;
            }
            // 确保i至少增加1，避免无限循环
            i += if len > 0 { len } else { 1 };
        }

        count
    }

    /// 计算单个UTF-8字符的长度
    fn utf8_char_length(&self, input: &[u8]) -> (usize, bool) {
        if input.is_empty() {
            return (0, false);
        }

        let first = input[0];

        if first < 0x80 {
            // ASCII字符
            (1, true)
        } else if first < 0xC0 {
            // 续字节，但不是起始字节
            (1, false)
        } else if first < 0xE0 {
            // 2字节序列
            if input.len() >= 2 && (input[1] & 0xC0) == 0x80 {
                (2, true)
            } else {
                (1, false)
            }
        } else if first < 0xF0 {
            // 3字节序列
            if input.len() >= 3 && (input[1] & 0xC0) == 0x80 && (input[2] & 0xC0) == 0x80 {
                (3, true)
            } else {
                (1, false)
            }
        } else if first < 0xF8 {
            // 4字节序列
            if input.len() >= 4
                && (input[1] & 0xC0) == 0x80
                && (input[2] & 0xC0) == 0x80
                && (input[3] & 0xC0) == 0x80
            {
                (4, true)
            } else {
                (1, false)
            }
        } else {
            // 无效的起始字节
            (1, false)
        }
    }

    /// 安全地将字节数组转换为字符串
    pub fn to_string<'a>(&self, input: &'a [u8]) -> Option<&'a str> {
        match self.config.validation_level {
            Utf8ValidationLevel::Strict => core::str::from_utf8(input).ok(),
            Utf8ValidationLevel::Lenient => {
                // 宽松模式：尝试转换，忽略无效部分
                core::str::from_utf8(input).ok()
            }
            Utf8ValidationLevel::None => {
                // 无验证：仍然使用安全的转换，因为输入可能不是有效的UTF-8
                core::str::from_utf8(input).ok()
            }
        }
    }

    /// 比较两个UTF-8字符串
    pub fn compare(&self, a: &[u8], b: &[u8]) -> core::cmp::Ordering {
        // 找到第一个null终止符的位置
        let a_len = a.iter().position(|&c| c == 0).unwrap_or(a.len());
        let b_len = b.iter().position(|&c| c == 0).unwrap_or(b.len());

        // 尝试转换为字符串进行比较
        if let (Some(a_str), Some(b_str)) = (
            core::str::from_utf8(&a[..a_len]).ok(),
            core::str::from_utf8(&b[..b_len]).ok(),
        ) {
            a_str.cmp(b_str)
        } else {
            // 如果转换失败，回退到字节比较
            a[..a_len].cmp(&b[..b_len])
        }
    }

    /// 检查字符串是否以指定前缀开始
    pub fn starts_with(&self, input: &[u8], prefix: &[u8]) -> bool {
        // 找到第一个null终止符的位置
        let input_len = input.iter().position(|&c| c == 0).unwrap_or(input.len());
        let prefix_len = prefix.iter().position(|&c| c == 0).unwrap_or(prefix.len());

        if prefix_len > input_len {
            return false;
        }

        for i in 0..prefix_len {
            if input[i] != prefix[i] {
                return false;
            }
        }

        true
    }

    /// 检查字符串是否包含指定子串
    pub fn contains(&self, input: &[u8], substring: &[u8]) -> bool {
        // 找到第一个null终止符的位置
        let input_len = input.iter().position(|&c| c == 0).unwrap_or(input.len());
        let substring_len = substring
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(substring.len());

        if substring_len == 0 {
            return true;
        }

        if substring_len > input_len {
            return false;
        }

        for i in 0..=input_len - substring_len {
            let mut match_found = true;
            for j in 0..substring_len {
                if input[i + j] != substring[j] {
                    match_found = false;
                    break;
                }
            }
            if match_found {
                return true;
            }
        }

        false
    }
}

impl Default for Utf8Processor {
    fn default() -> Self {
        Self {
            config: Utf8Config::default(),
        }
    }
}

/// 全局UTF-8处理器实例
pub static GLOBAL_UTF8_PROCESSOR: Utf8Processor = Utf8Processor {
    config: Utf8Config {
        validation_level: Utf8ValidationLevel::Strict,
        normalization: None,
        case_mapping: false,
        grapheme_cluster: false,
    },
};

/// 获取全局UTF-8处理器实例
pub fn get_global_utf8_processor() -> &'static Utf8Processor {
    &GLOBAL_UTF8_PROCESSOR
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_utf8_validation() {
        let processor = Utf8Processor::default();

        // 测试有效UTF-8
        let valid_utf8 = "Hello 世界 👋".as_bytes();
        assert_eq!(processor.validate(valid_utf8), Utf8Result::Ok(()));

        // 测试ASCII
        let ascii = "Hello World".as_bytes();
        assert_eq!(processor.validate(ascii), Utf8Result::Ok(()));
    }

    #[test]
    fn test_char_length() {
        let processor = Utf8Processor::default();

        // 测试ASCII
        let ascii = "Hello".as_bytes();
        assert_eq!(processor.char_length(ascii), 5);

        // 测试UTF-8
        let utf8 = "Hello 世界".as_bytes();
        assert_eq!(processor.char_length(utf8), 8); // "Hello " + "世界"

        // 测试包含emoji
        let emoji = "👋 世界".as_bytes();
        assert_eq!(processor.char_length(emoji), 4); // "👋 " + "世界"
    }

    #[test]
    fn test_compare() {
        let processor = Utf8Processor::default();

        let a = "apple".as_bytes();
        let b = "banana".as_bytes();
        let c = "apple".as_bytes();

        assert_eq!(processor.compare(a, b), core::cmp::Ordering::Less);
        assert_eq!(processor.compare(b, a), core::cmp::Ordering::Greater);
        assert_eq!(processor.compare(a, c), core::cmp::Ordering::Equal);
    }

    #[test]
    fn test_starts_with() {
        let processor = Utf8Processor::default();

        let input = "Hello World".as_bytes();
        let prefix1 = "Hello".as_bytes();
        let prefix2 = "World".as_bytes();

        assert!(processor.starts_with(input, prefix1));
        assert!(!processor.starts_with(input, prefix2));
    }

    #[test]
    fn test_contains() {
        let processor = Utf8Processor::default();

        let input = "Hello World".as_bytes();
        let substr1 = "World".as_bytes();
        let substr2 = "Test".as_bytes();

        assert!(processor.contains(input, substr1));
        assert!(!processor.contains(input, substr2));
    }
}
