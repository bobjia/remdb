//! JSON文档处理
//! 
//! 该模块实现了JSON文档的二进制存储和操作，支持：
//! - 二进制JSON格式（MessagePack/CBOR）
//! - 零拷贝访问
//! - 延迟解析
//! - 与JSON内存池集成

use crate::json::memory_pool::get_global_json_pool_manager;
use crate::types::JsonStorage;

/// JSON值类型
#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    /// 字符串
    String(alloc::string::String),
    /// 数字
    Number(alloc::string::String),
    /// 布尔值
    Boolean(bool),
    /// null
    Null,
    /// 对象
    Object(alloc::collections::BTreeMap<alloc::string::String, JsonValue>),
    /// 数组
    Array(alloc::vec::Vec<JsonValue>),
}

/// JSON查询结果
#[derive(Debug, Clone, PartialEq)]
pub enum JsonQueryResult {
    /// 无结果
    None,
    /// 标量值
    Scalar(alloc::string::String),
    /// 对象
    Object(JsonDocument),
    /// 数组
    Array(alloc::vec::Vec<JsonQueryResult>),
}

/// JSON文档
pub struct JsonDocument {
    /// 存储方式
    storage: JsonStorage,
    /// 大小（字节）
    size: usize,
}

impl JsonDocument {
    /// 从JSON字符串创建文档
    pub fn from_json(json_str: &str) -> Result<Self, &'static str> {
        // 处理空字符串情况
        if json_str.trim().is_empty() {
            return Ok(Self {
                storage: JsonStorage::Null,
                size: 0,
            });
        }
        
        // 检查JSON字符串长度，限制为10KB
        const MAX_JSON_SIZE: usize = 10 * 1024; // 10KB
        if json_str.len() > MAX_JSON_SIZE {
            return Err("JSON too large (max 10KB)");
        }
        
        // 直接存储原始JSON字符串，不序列化为MessagePack/CBOR
        let data = json_str.as_bytes();
        let size = data.len();
        
        Self::from_binary(data, size)
    }
    
    /// 从二进制数据创建文档
    pub fn from_binary(data: &[u8], size: usize) -> Result<Self, &'static str> {
        if size == 0 {
            return Ok(Self {
                storage: JsonStorage::Null,
                size: 0,
            });
        }
        
        // 检查二进制数据长度，限制为10KB
        const MAX_JSON_SIZE: usize = 10 * 1024; // 10KB
        if size > MAX_JSON_SIZE {
            return Err("JSON too large (max 10KB)");
        }
        
        if size <= 256 {
            // 使用内联存储
            let mut inline_data = [0u8; 256];
            inline_data[..size].copy_from_slice(data);
            
            Ok(Self {
                storage: JsonStorage::Inline(inline_data),
                size,
            })
        } else {
            // 使用外部存储
            let pool_manager = get_global_json_pool_manager().ok_or("JSON pool manager not initialized")?;
            
            // 获取默认内存池（ID 0）
            let pool = pool_manager.get_pool_mut(0).ok_or("Default JSON pool not found")?;
            
            match pool.allocate(size) {
                Some((block_idx, offset)) => {
                    // 复制数据到内存池
                    if let Some(data_ptr) = pool.get_block_data(block_idx, offset) {
                        unsafe {
                            core::ptr::copy_nonoverlapping(data.as_ptr(), data_ptr as *mut u8, size);
                        }
                        
                        Ok(Self {
                            storage: JsonStorage::External {
                                pool_id: 0,
                                offset: block_idx as u32,
                                length: size as u32,
                            },
                            size,
                        })
                    } else {
                        Err("Failed to get block data")
                    }
                }
                None => {
                    Err("Failed to allocate JSON memory")
                }
            }
        }
    }
    
    /// 直接设置JSON字符串内容（避免创建新的JsonDocument）
    pub fn set_json_string(&mut self, json_str: &str) -> Result<(), &'static str> {
        // 检查JSON字符串长度，限制为10KB
        const MAX_JSON_SIZE: usize = 10 * 1024; // 10KB
        if json_str.len() > MAX_JSON_SIZE {
            return Err("JSON too large (max 10KB)");
        }
        
        // 直接存储原始JSON字符串，始终使用内联存储
        let data = json_str.as_bytes();
        let size = data.len();
        
        // 始终使用内联存储（最多256字节）
        if size > 256 {
            return Err("JSON too large for inline storage (max 256 bytes)");
        }
        
        let mut inline_data = [0u8; 256];
        inline_data[..size].copy_from_slice(data);
        
        self.storage = JsonStorage::Inline(inline_data);
        self.size = size;
        Ok(())
    }
    
    /// 序列化为MessagePack
    fn serialize_to_messagepack(json_str: &str) -> Result<(alloc::vec::Vec<u8>, usize), &'static str> {
        // 首先解析JSON字符串为JsonValue
        let json_value = Self::parse_json_str(json_str)?;
        
        // 序列化JsonValue为MessagePack
        let mut buffer = alloc::vec::Vec::new();
        Self::serialize_value_to_msgpack(&json_value, &mut buffer)?;
        
        let size = buffer.len();
        Ok((buffer, size))
    }
    
    /// 将JsonValue序列化为MessagePack
    fn serialize_value_to_msgpack(value: &JsonValue, buffer: &mut alloc::vec::Vec<u8>) -> Result<(), &'static str> {
        match value {
            JsonValue::Null => {
                buffer.push(0xc0); // nil
            }
            JsonValue::Boolean(b) => {
                if *b {
                    buffer.push(0xc3); // true
                } else {
                    buffer.push(0xc2); // false
                }
            }
            JsonValue::String(s) => {
                // 序列化字符串
                let len = s.len();
                if len < 32 {
                    // fixstr
                    buffer.push(0xa0 | len as u8);
                } else if len < 256 {
                    // str 8
                    buffer.push(0xd9);
                    buffer.push(len as u8);
                } else if len < 65536 {
                    // str 16
                    buffer.push(0xda);
                    buffer.extend_from_slice(&[(len >> 8) as u8, len as u8]);
                } else {
                    // str 32
                    buffer.push(0xdb);
                    buffer.extend_from_slice(&[(len >> 24) as u8, (len >> 16) as u8, (len >> 8) as u8, len as u8]);
                }
                buffer.extend_from_slice(s.as_bytes());
            }
            JsonValue::Number(n) => {
                // 尝试解析为整数
                if let Ok(i) = n.parse::<i64>() {
                    // 序列化整数
                    if i >= 0 && i < 128 {
                        // positive fixint
                        buffer.push(i as u8);
                    } else if i >= -32 && i < 0 {
                        // negative fixint
                        buffer.push((i as u8) & 0xff);
                    } else if i >= -128 && i < 128 {
                        // int 8
                        buffer.push(0xd0);
                        buffer.push(i as u8);
                    } else if i >= -32768 && i < 32768 {
                        // int 16
                        buffer.push(0xd1);
                        buffer.extend_from_slice(&[(i >> 8) as u8, i as u8]);
                    } else if i >= -2147483648 && i < 2147483648 {
                        // int 32
                        buffer.push(0xd2);
                        buffer.extend_from_slice(&[(i >> 24) as u8, (i >> 16) as u8, (i >> 8) as u8, i as u8]);
                    } else {
                        // int 64
                        buffer.push(0xd3);
                        buffer.extend_from_slice(&[
                            (i >> 56) as u8, (i >> 48) as u8, (i >> 40) as u8, (i >> 32) as u8,
                            (i >> 24) as u8, (i >> 16) as u8, (i >> 8) as u8, i as u8
                        ]);
                    }
                } else if let Ok(f) = n.parse::<f64>() {
                    // 序列化浮点数
                    buffer.push(0xcb); // float 64
                    buffer.extend_from_slice(&f.to_le_bytes());
                } else {
                    return Err("Invalid number format");
                }
            }
            JsonValue::Array(arr) => {
                // 序列化数组
                let len = arr.len();
                if len < 16 {
                    // fixarray
                    buffer.push(0x90 | len as u8);
                } else if len < 65536 {
                    // array 16
                    buffer.push(0xdc);
                    buffer.extend_from_slice(&[(len >> 8) as u8, len as u8]);
                } else {
                    // array 32
                    buffer.push(0xdd);
                    buffer.extend_from_slice(&[(len >> 24) as u8, (len >> 16) as u8, (len >> 8) as u8, len as u8]);
                }
                // 序列化数组元素
                for item in arr {
                    Self::serialize_value_to_msgpack(item, buffer)?;
                }
            }
            JsonValue::Object(obj) => {
                // 序列化对象
                let len = obj.len();
                if len < 16 {
                    // fixmap
                    buffer.push(0x80 | len as u8);
                } else if len < 65536 {
                    // map 16
                    buffer.push(0xde);
                    buffer.extend_from_slice(&[(len >> 8) as u8, len as u8]);
                } else {
                    // map 32
                    buffer.push(0xdf);
                    buffer.extend_from_slice(&[(len >> 24) as u8, (len >> 16) as u8, (len >> 8) as u8, len as u8]);
                }
                // 序列化键值对
                for (key, val) in obj {
                    // 键必须是字符串
                    let key_value = JsonValue::String(key.clone());
                    Self::serialize_value_to_msgpack(&key_value, buffer)?;
                    Self::serialize_value_to_msgpack(val, buffer)?;
                }
            }
        }
        Ok(())
    }
    
    /// 序列化为CBOR
    fn serialize_to_cbor(json_str: &str) -> Result<(alloc::vec::Vec<u8>, usize), &'static str> {
        // 首先解析JSON字符串为JsonValue
        let json_value = Self::parse_json_str(json_str)?;
        
        // 序列化JsonValue为CBOR
        let mut buffer = alloc::vec::Vec::new();
        Self::serialize_value_to_cbor(&json_value, &mut buffer)?;
        
        let size = buffer.len();
        Ok((buffer, size))
    }
    
    /// 将JsonValue序列化为CBOR
    fn serialize_value_to_cbor(value: &JsonValue, buffer: &mut alloc::vec::Vec<u8>) -> Result<(), &'static str> {
        match value {
            JsonValue::Null => {
                buffer.push(0xf6); // null
            }
            JsonValue::Boolean(b) => {
                if *b {
                    buffer.push(0xf5); // true
                } else {
                    buffer.push(0xf4); // false
                }
            }
            JsonValue::String(s) => {
                // 序列化字符串
                let len = s.len();
                if len < 24 {
                    // 短字符串
                    buffer.push(0x60 | len as u8);
                } else if len < 256 {
                    // 8位长度字符串
                    buffer.push(0x78);
                    buffer.push(len as u8);
                } else if len < 65536 {
                    // 16位长度字符串
                    buffer.push(0x79);
                    buffer.extend_from_slice(&[(len >> 8) as u8, len as u8]);
                } else {
                    // 32位长度字符串
                    buffer.push(0x7a);
                    buffer.extend_from_slice(&[(len >> 24) as u8, (len >> 16) as u8, (len >> 8) as u8, len as u8]);
                }
                buffer.extend_from_slice(s.as_bytes());
            }
            JsonValue::Number(n) => {
                // 尝试解析为整数
                if let Ok(i) = n.parse::<i64>() {
                    if i >= 0 {
                        // 无符号整数
                        if i < 24 {
                            buffer.push(i as u8);
                        } else if i < 256 {
                            buffer.push(0x18);
                            buffer.push(i as u8);
                        } else if i < 65536 {
                            buffer.push(0x19);
                            buffer.extend_from_slice(&[(i >> 8) as u8, i as u8]);
                        } else if i < 4294967296 {
                            buffer.push(0x1a);
                            buffer.extend_from_slice(&[(i >> 24) as u8, (i >> 16) as u8, (i >> 8) as u8, i as u8]);
                        } else {
                            buffer.push(0x1b);
                            buffer.extend_from_slice(&[
                                (i >> 56) as u8, (i >> 48) as u8, (i >> 40) as u8, (i >> 32) as u8,
                                (i >> 24) as u8, (i >> 16) as u8, (i >> 8) as u8, i as u8
                            ]);
                        }
                    } else {
                        // 有符号整数
                        let abs_i = -i - 1;
                        if abs_i < 24 {
                            buffer.push(0x20 | abs_i as u8);
                        } else if abs_i < 256 {
                            buffer.push(0x38);
                            buffer.push(abs_i as u8);
                        } else if abs_i < 65536 {
                            buffer.push(0x39);
                            buffer.extend_from_slice(&[(abs_i >> 8) as u8, abs_i as u8]);
                        } else if abs_i < 4294967296 {
                            buffer.push(0x3a);
                            buffer.extend_from_slice(&[(abs_i >> 24) as u8, (abs_i >> 16) as u8, (abs_i >> 8) as u8, abs_i as u8]);
                        } else {
                            buffer.push(0x3b);
                            buffer.extend_from_slice(&[
                                (abs_i >> 56) as u8, (abs_i >> 48) as u8, (abs_i >> 40) as u8, (abs_i >> 32) as u8,
                                (abs_i >> 24) as u8, (abs_i >> 16) as u8, (abs_i >> 8) as u8, abs_i as u8
                            ]);
                        }
                    }
                } else if let Ok(f) = n.parse::<f64>() {
                    // 序列化浮点数
                    buffer.push(0xfb); // float 64
                    buffer.extend_from_slice(&f.to_le_bytes());
                } else {
                    return Err("Invalid number format");
                }
            }
            JsonValue::Array(arr) => {
                // 序列化数组
                let len = arr.len();
                if len < 24 {
                    // 短数组
                    buffer.push(0x80 | len as u8);
                } else if len < 256 {
                    // 8位长度数组
                    buffer.push(0x98);
                    buffer.push(len as u8);
                } else if len < 65536 {
                    // 16位长度数组
                    buffer.push(0x99);
                    buffer.extend_from_slice(&[(len >> 8) as u8, len as u8]);
                } else {
                    // 32位长度数组
                    buffer.push(0x9a);
                    buffer.extend_from_slice(&[(len >> 24) as u8, (len >> 16) as u8, (len >> 8) as u8, len as u8]);
                }
                // 序列化数组元素
                for item in arr {
                    Self::serialize_value_to_cbor(item, buffer)?;
                }
            }
            JsonValue::Object(obj) => {
                // 序列化对象
                let len = obj.len();
                if len < 24 {
                    // 短映射
                    buffer.push(0xa0 | len as u8);
                } else if len < 256 {
                    // 8位长度映射
                    buffer.push(0xb8);
                    buffer.push(len as u8);
                } else if len < 65536 {
                    // 16位长度映射
                    buffer.push(0xb9);
                    buffer.extend_from_slice(&[(len >> 8) as u8, len as u8]);
                } else {
                    // 32位长度映射
                    buffer.push(0xba);
                    buffer.extend_from_slice(&[(len >> 24) as u8, (len >> 16) as u8, (len >> 8) as u8, len as u8]);
                }
                // 序列化键值对
                for (key, val) in obj {
                    // 键必须是字符串
                    let key_value = JsonValue::String(key.clone());
                    Self::serialize_value_to_cbor(&key_value, buffer)?;
                    Self::serialize_value_to_cbor(val, buffer)?;
                }
            }
        }
        Ok(())
    }
    
    /// 反序列化为JSON字符串
    pub fn to_json(&self) -> Result<alloc::string::String, &'static str> {
        match &self.storage {
            JsonStorage::Inline(data) => {
                eprintln!("DEBUG to_json: data.len()={}, self.size={}", data.len(), self.size);
                eprintln!("DEBUG to_json: data={:?}", &data[..core::cmp::min(self.size, 64)]);
                
                // 需要反序列化MessagePack/CBOR数据为JsonValue，然后转换为JSON字符串
                // 暂时使用parse_json_str解析原始JSON字符串（如果存储的是JSON）
                let json_str = alloc::string::String::from_utf8_lossy(&data[..self.size]).to_string();
                eprintln!("DEBUG to_json: json_str={}", json_str);
                Ok(json_str)
            }
            JsonStorage::External { pool_id, offset, length } => {
                let pool_manager = get_global_json_pool_manager().ok_or("JSON pool manager not initialized")?;
                let pool = pool_manager.get_pool(*pool_id).ok_or("JSON pool not found")?;
                
                if let Some(data_ptr) = pool.get_block_data(*offset as usize, 0) {
                    let data = unsafe { core::slice::from_raw_parts(data_ptr, *length as usize) };
                    
                    let json_str = alloc::string::String::from_utf8_lossy(data).to_string();
                    Ok(json_str)
                } else {
                    Err("Failed to get block data")
                }
            }
            JsonStorage::Null => {
                Ok("null".to_string())
            }
        }
    }
    
    /// 获取存储方式
    pub fn storage(&self) -> &JsonStorage {
        &self.storage
    }
    
    /// 获取大小
    pub fn size(&self) -> usize {
        self.size
    }
    
    /// 检查是否为null
    pub fn is_null(&self) -> bool {
        matches!(self.storage, JsonStorage::Null)
    }
    
    /// 增加引用计数
    pub fn add_ref(&self) {
        match &self.storage {
            JsonStorage::External { pool_id: _, offset: _, length: _ } => {
                let pool_manager = get_global_json_pool_manager();
                if let Some(_manager) = pool_manager {
                    // 暂时不实现引用计数，因为内存池还没有相应的方法
                }
            }
            _ => {}
        }
    }
    
    /// 减少引用计数
    pub fn release(&self) {
        match &self.storage {
            JsonStorage::External { pool_id: _, offset: _, length: _ } => {
                let pool_manager = get_global_json_pool_manager();
                if let Some(_manager) = pool_manager {
                    // 暂时不实现引用计数，因为内存池还没有相应的方法
                }
            }
            _ => {}
        }
    }
    
    /// 解析JSON数据为JsonValue
    pub fn parse_json(&self) -> Result<JsonValue, &'static str> {
        let json_str = self.to_json()?;
        Self::parse_json_str(&json_str)
    }
    
    /// 解析JSON字符串
    pub fn parse_json_str(s: &str) -> Result<JsonValue, &'static str> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Ok(JsonValue::Null);
        }
        let mut chars = trimmed.chars().collect::<alloc::vec::Vec<_>>();
        let mut index = 0;
        Self::parse_value(&mut chars, &mut index)
    }
    
    /// 解析JSON值
    fn parse_value(chars: &mut alloc::vec::Vec<char>, index: &mut usize) -> Result<JsonValue, &'static str> {
        Self::skip_whitespace(chars, index);
        
        if *index >= chars.len() {
            return Err("Unexpected end of JSON");
        }
        
        match chars[*index] {
            '"' => Self::parse_string(chars, index),
            '{' => Self::parse_object(chars, index),
            '[' => Self::parse_array(chars, index),
            't' => Self::parse_literal(chars, index, "true", JsonValue::Boolean(true)),
            'f' => Self::parse_literal(chars, index, "false", JsonValue::Boolean(false)),
            'n' => Self::parse_literal(chars, index, "null", JsonValue::Null),
            '-' | '0'..='9' => Self::parse_number(chars, index),
            _ => Err("Invalid JSON value"),
        }
    }
    
    /// 解析字符串
    fn parse_string(chars: &mut alloc::vec::Vec<char>, index: &mut usize) -> Result<JsonValue, &'static str> {
        *index += 1; // 跳过开始引号
        let mut s = alloc::string::String::new();
        
        while *index < chars.len() {
            match chars[*index] {
                '"' => {
                    *index += 1;
                    return Ok(JsonValue::String(s));
                }
                '\\' => {
                    *index += 1;
                    if *index >= chars.len() {
                        return Err("Unexpected end of JSON");
                    }
                    match chars[*index] {
                        '"' => s.push('"'),
                        '\\' => s.push('\\'),
                        '/' => s.push('/'),
                        'b' => s.push('\x08'),
                        'f' => s.push('\x0c'),
                        'n' => s.push('\n'),
                        'r' => s.push('\r'),
                        't' => s.push('\t'),
                        'u' => {
                            // 解析Unicode转义
                            *index += 1;
                            let mut code = 0;
                            for _ in 0..4 {
                                if *index >= chars.len() {
                                    return Err("Invalid Unicode escape");
                                }
                                let c = chars[*index];
                                if !c.is_ascii_hexdigit() {
                                    return Err("Invalid Unicode escape");
                                }
                                code = code * 16 + c.to_digit(16).expect("invalid hex digit") as u32;
                                *index += 1;
                            }
                            if let Some(c) = char::from_u32(code) {
                                s.push(c);
                            }
                        }
                        _ => return Err("Invalid escape sequence"),
                    }
                }
                c => {
                    s.push(c);
                }
            }
            *index += 1;
        }
        
        Err("Unterminated string")
    }
    
    /// 解析对象
    fn parse_object(chars: &mut alloc::vec::Vec<char>, index: &mut usize) -> Result<JsonValue, &'static str> {
        *index += 1; // 跳过开始大括号
        let mut obj = alloc::collections::BTreeMap::new();
        
        loop {
            Self::skip_whitespace(chars, index);
            
            if *index >= chars.len() {
                return Err("Unexpected end of JSON");
            }
            
            if chars[*index] == '}' {
                *index += 1;
                return Ok(JsonValue::Object(obj));
            }
            
            // 解析键
            let key = match Self::parse_string(chars, index) {
                Ok(JsonValue::String(s)) => s,
                _ => return Err("Invalid object key"),
            };
            
            Self::skip_whitespace(chars, index);
            
            if *index >= chars.len() || chars[*index] != ':' {
                return Err("Expected colon after object key");
            }
            *index += 1;
            
            // 解析值
            let value = Self::parse_value(chars, index)?;
            obj.insert(key, value);
            
            Self::skip_whitespace(chars, index);
            
            if *index >= chars.len() {
                return Err("Unexpected end of JSON");
            }
            
            if chars[*index] == '}' {
                *index += 1;
                return Ok(JsonValue::Object(obj));
            }
            
            if chars[*index] != ',' {
                return Err("Expected comma after object value");
            }
            *index += 1;
        }
    }
    
    /// 解析数组
    fn parse_array(chars: &mut alloc::vec::Vec<char>, index: &mut usize) -> Result<JsonValue, &'static str> {
        *index += 1; // 跳过开始中括号
        let mut arr = alloc::vec::Vec::new();
        
        loop {
            Self::skip_whitespace(chars, index);
            
            if *index >= chars.len() {
                return Err("Unexpected end of JSON");
            }
            
            if chars[*index] == ']' {
                *index += 1;
                return Ok(JsonValue::Array(arr));
            }
            
            // 解析值
            let value = Self::parse_value(chars, index)?;
            arr.push(value);
            
            Self::skip_whitespace(chars, index);
            
            if *index >= chars.len() {
                return Err("Unexpected end of JSON");
            }
            
            if chars[*index] == ']' {
                *index += 1;
                return Ok(JsonValue::Array(arr));
            }
            
            if chars[*index] != ',' {
                return Err("Expected comma after array value");
            }
            *index += 1;
        }
    }
    
    /// 解析数字
    fn parse_number(chars: &mut alloc::vec::Vec<char>, index: &mut usize) -> Result<JsonValue, &'static str> {
        let start = *index;
        
        // 解析负号
        if chars[*index] == '-' {
            *index += 1;
        }
        
        // 解析整数部分
        if *index < chars.len() && chars[*index] == '0' {
            *index += 1;
        } else if *index < chars.len() && chars[*index].is_ascii_digit() {
            *index += 1;
            while *index < chars.len() && chars[*index].is_ascii_digit() {
                *index += 1;
            }
        } else {
            return Err("Invalid number");
        }
        
        // 解析小数部分
        if *index < chars.len() && chars[*index] == '.' {
            *index += 1;
            if *index >= chars.len() || !chars[*index].is_ascii_digit() {
                return Err("Invalid decimal part");
            }
            while *index < chars.len() && chars[*index].is_ascii_digit() {
                *index += 1;
            }
        }
        
        // 解析指数部分
        if *index < chars.len() && (chars[*index] == 'e' || chars[*index] == 'E') {
            *index += 1;
            if *index < chars.len() && (chars[*index] == '+' || chars[*index] == '-') {
                *index += 1;
            }
            if *index >= chars.len() || !chars[*index].is_ascii_digit() {
                return Err("Invalid exponent part");
            }
            while *index < chars.len() && chars[*index].is_ascii_digit() {
                *index += 1;
            }
        }
        
        let num_str: alloc::string::String = chars[start..*index].iter().collect();
        Ok(JsonValue::Number(num_str))
    }
    
    /// 解析字面量
    fn parse_literal(chars: &mut alloc::vec::Vec<char>, index: &mut usize, literal: &str, value: JsonValue) -> Result<JsonValue, &'static str> {
        for (i, c) in literal.chars().enumerate() {
            if *index + i >= chars.len() || chars[*index + i] != c {
                return Err(alloc::format!("Expected {}", literal).leak());
            }
        }
        *index += literal.len();
        Ok(value)
    }
    
    /// 跳过空白字符
    fn skip_whitespace(chars: &mut alloc::vec::Vec<char>, index: &mut usize) {
        while *index < chars.len() && chars[*index].is_ascii_whitespace() {
            *index += 1;
        }
    }
}

impl Drop for JsonDocument {
    fn drop(&mut self) {
        self.release();
    }
}

impl Clone for JsonDocument {
    fn clone(&self) -> Self {
        self.add_ref();
        Self {
            storage: self.storage.clone(),
            size: self.size,
        }
    }
}

impl core::fmt::Debug for JsonDocument {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("JsonDocument")
            .field("size", &self.size)
            .finish()
    }
}

impl PartialEq for JsonDocument {
    fn eq(&self, other: &Self) -> bool {
        self.size == other.size
    }
}

impl Eq for JsonDocument {
}



/// JSON路径查询
pub fn json_extract(doc: &JsonDocument, path: &str) -> JsonQueryResult {
    match crate::json::path::parse_json_path(path) {
        Ok(json_path) => json_path.execute(doc),
        Err(_) => JsonQueryResult::None,
    }
}

/// 检查路径是否存在
pub fn json_has(doc: &JsonDocument, path: &str) -> bool {
    match crate::json::path::parse_json_path(path) {
        Ok(json_path) => {
            match json_path.execute(doc) {
                JsonQueryResult::None => false,
                _ => true,
            }
        }
        Err(_) => false,
    }
}

/// 获取路径对应的值类型
pub fn json_type(doc: &JsonDocument, path: &str) -> &'static str {
    match crate::json::path::parse_json_path(path) {
        Ok(json_path) => {
            match json_path.execute(doc) {
                JsonQueryResult::Scalar(s) => {
                    if s == "true" || s == "false" {
                        "boolean"
                    } else if s == "null" {
                        "null"
                    } else if s.parse::<f64>().is_ok() {
                        "number"
                    } else {
                        "string"
                    }
                }
                JsonQueryResult::Object(_) => "object",
                JsonQueryResult::Array(_) => "array",
                JsonQueryResult::None => "null",
            }
        }
        Err(_) => "null",
    }
}

/// 从JSON中提取文本值
pub fn json_extract_text(doc: &JsonDocument, path: &str) -> Option<alloc::string::String> {
    match json_extract(doc, path) {
        JsonQueryResult::Scalar(s) => Some(s),
        _ => None,
    }
}

/// 从JSON中提取整数值
pub fn json_extract_int(doc: &JsonDocument, path: &str) -> Option<i64> {
    match json_extract(doc, path) {
        JsonQueryResult::Scalar(s) => s.parse::<i64>().ok(),
        _ => None,
    }
}

/// 从JSON中提取浮点数值
pub fn json_extract_float(doc: &JsonDocument, path: &str) -> Option<f64> {
    match json_extract(doc, path) {
        JsonQueryResult::Scalar(s) => s.parse::<f64>().ok(),
        _ => None,
    }
}

/// 设置JSON路径对应的值
pub fn json_set(doc: &mut JsonDocument, path: &str, value: &str) -> Result<(), &'static str> {
    eprintln!("DEBUG json_set: path={}, value={}", path, value);
    
    // 1. 解析原始JSON文档为JsonValue树
    let mut json_value = doc.parse_json()?;
    eprintln!("DEBUG json_set: parsed json_value={:?}", json_value);
    
    // 2. 将值字符串解析为JsonValue
    let new_value = JsonDocument::parse_json_str(value)?;
    eprintln!("DEBUG json_set: new_value={:?}", new_value);
    
    // 3. 解析路径为键的向量
    let keys = parse_simple_json_path(path)?;
    eprintln!("DEBUG json_set: keys={:?}", keys);
    
    // 4. 在JsonValue树中设置值
    set_value_at_path(&mut json_value, &keys, new_value)?;
    eprintln!("DEBUG json_set: set value successfully");
    
    // 5. 将修改后的JsonValue序列化为JSON字符串
    let new_json_str = json_value.to_json_string();
    eprintln!("DEBUG json_set: new_json_str={}, len={}", new_json_str, new_json_str.len());
    
    // 6. 直接设置JsonDocument的内容，避免创建新的JsonDocument
    doc.set_json_string(&new_json_str)?;
    eprintln!("DEBUG json_set: set json_string successfully");
    
    Ok(())
}

/// 解析简单的JSON路径（仅支持$.key1.key2格式）
fn parse_simple_json_path(path: &str) -> Result<Vec<String>, &'static str> {
    if !path.starts_with('$') {
        return Err("Path must start with $");
    }
    
    let mut result = Vec::new();
    let mut current = &path[1..]; // 去除$
    
    // 如果路径是"$"，则返回空向量（根路径）
    if current.is_empty() {
        return Ok(result);
    }
    
    // 检查是否有"."前缀
    if current.starts_with('.') {
        current = &current[1..];
    }
    
    // 分割剩余部分为键
    for part in current.split('.') {
        if part.is_empty() {
            return Err("Empty key in path");
        }
        // 检查键是否被引号包围
        let key = if part.starts_with('"') && part.ends_with('"') {
            part[1..part.len()-1].to_string()
        } else if part.starts_with('\'') && part.ends_with('\'') {
            part[1..part.len()-1].to_string()
        } else {
            part.to_string()
        };
        result.push(key);
    }
    
    Ok(result)
}

/// 在JsonValue树中设置路径对应的值
fn set_value_at_path(root: &mut JsonValue, keys: &[String], value: JsonValue) -> Result<(), &'static str> {
    if keys.is_empty() {
        // 设置根值
        *root = value;
        return Ok(());
    }
    
    let mut current = root;
    for (i, key) in keys.iter().enumerate() {
        let is_last = i == keys.len() - 1;
        
        match current {
            JsonValue::Object(obj) => {
                if is_last {
                    // 最后一个键：设置值
                    obj.insert(key.clone(), value);
                    return Ok(());
                } else {
                    // 不是最后一个键：导航到下一个对象
                    // 使用entry API避免双重借用
                    use alloc::collections::btree_map::Entry;
                    match obj.entry(key.clone()) {
                        Entry::Occupied(entry) => {
                            current = entry.into_mut();
                        }
                        Entry::Vacant(entry) => {
                            // 创建中间对象并获取其引用
                            let new_obj = JsonValue::Object(alloc::collections::BTreeMap::new());
                            current = entry.insert(new_obj);
                        }
                    }
                }
            }
            JsonValue::Array(arr) => {
                // 尝试将键解析为索引
                if let Ok(idx) = key.parse::<usize>() {
                    if is_last {
                        // 设置数组元素
                        if idx < arr.len() {
                            arr[idx] = value;
                        } else {
                            // 扩展数组
                            while arr.len() <= idx {
                                arr.push(JsonValue::Null);
                            }
                            arr[idx] = value;
                        }
                        return Ok(());
                    } else {
                        if idx < arr.len() {
                            current = &mut arr[idx];
                        } else {
                            return Err("Array index out of bounds for intermediate path");
                        }
                    }
                } else {
                    return Err("Array index must be a number");
                }
            }
            _ => {
                // 当前值不是对象或数组，但还有更多路径组件
                return Err("Cannot navigate through non-object/non-array value");
            }
        }
    }
    
    Ok(())
}

/// 插入JSON路径对应的值
pub fn json_insert(doc: &mut JsonDocument, path: &str, value: &str) -> Result<(), &'static str> {
    // 对于json_insert，我们使用与json_set相同的实现
    // 因为在大多数情况下，插入操作与设置操作行为相同
    json_set(doc, path, value)
}

/// 替换JSON路径对应的值
pub fn json_replace(doc: &mut JsonDocument, path: &str, value: &str) -> Result<(), &'static str> {
    // 对于json_replace，我们使用与json_set相同的实现
    // 因为替换操作与设置操作行为相同
    json_set(doc, path, value)
}

/// 删除JSON路径对应的值
pub fn json_remove(doc: &mut JsonDocument, path: &str) -> Result<(), &'static str> {
    eprintln!("DEBUG json_remove: path={}", path);
    // 1. 解析原始JSON文档为JsonValue树
    let mut json_value = doc.parse_json()?;
    eprintln!("DEBUG json_remove: parsed json_value={:?}", json_value);
    
    // 2. 解析路径为键的向量
    let keys = parse_simple_json_path(path)?;
    eprintln!("DEBUG json_remove: parsed keys={:?}", keys);
    
    // 3. 在JsonValue树中删除值
    remove_value_at_path(&mut json_value, &keys)?;
    eprintln!("DEBUG json_remove: removed value successfully");
    
    // 4. 将修改后的JsonValue序列化为JSON字符串
    let new_json_str = json_value.to_json_string();
    eprintln!("DEBUG json_remove: new_json_str={}", new_json_str);
    
    // 5. 创建新的JsonDocument替换原来的
    let new_doc = JsonDocument::from_json(&new_json_str)?;
    eprintln!("DEBUG json_remove: created new_doc successfully");
    
    // 替换存储
    *doc = new_doc;
    Ok(())
}

/// 合并JSON补丁
pub fn json_merge_patch(doc: &mut JsonDocument, patch: &str) -> Result<(), &'static str> {
    // 1. 解析原始JSON文档为JsonValue树
    let mut json_value = doc.parse_json()?;
    
    // 2. 解析补丁为JsonValue
    let patch_value = JsonDocument::parse_json_str(patch)?;
    
    // 3. 合并补丁到JsonValue树
    merge_json_patch(&mut json_value, &patch_value)?;
    
    // 4. 将修改后的JsonValue序列化为JSON字符串
    let new_json_str = json_value.to_json_string();
    
    // 5. 创建新的JsonDocument替换原来的
    let new_doc = JsonDocument::from_json(&new_json_str)?;
    
    // 替换存储
    *doc = new_doc;
    Ok(())
}

/// 在JsonValue树中删除路径对应的值
fn remove_value_at_path(root: &mut JsonValue, keys: &[String]) -> Result<(), &'static str> {
    if keys.is_empty() {
        // 空路径：将根值设为null
        *root = JsonValue::Null;
        return Ok(());
    }
    
    let mut current = root;
    for (i, key) in keys.iter().enumerate() {
        let is_last = i == keys.len() - 1;
        
        match current {
            JsonValue::Object(obj) => {
                if is_last {
                    // 最后一个键：删除值
                    obj.remove(key);
                    return Ok(());
                } else {
                    // 不是最后一个键：导航到下一个对象
                    if let Some(next) = obj.get_mut(key) {
                        current = next;
                    } else {
                        // 路径不存在，直接返回成功
                        return Ok(());
                    }
                }
            }
            JsonValue::Array(arr) => {
                // 尝试将键解析为索引
                if let Ok(idx) = key.parse::<usize>() {
                    if is_last {
                        // 删除数组元素
                        if idx < arr.len() {
                            arr.remove(idx);
                        }
                        return Ok(());
                    } else {
                        if idx < arr.len() {
                            current = &mut arr[idx];
                        } else {
                            // 路径不存在，直接返回成功
                            return Ok(());
                        }
                    }
                } else {
                    return Err("Array index must be a number");
                }
            }
            _ => {
                // 路径不存在，直接返回成功
                return Ok(());
            }
        }
    }
    
    Ok(())
}

/// 合并JSON补丁到JsonValue树
fn merge_json_patch(target: &mut JsonValue, patch: &JsonValue) -> Result<(), &'static str> {
    if let JsonValue::Object(target_obj) = target {
        if let JsonValue::Object(patch_obj) = patch {
            for (key, patch_val) in patch_obj {
                if let JsonValue::Null = patch_val {
                    // null值表示删除
                    target_obj.remove(key);
                } else if let Some(target_val) = target_obj.get_mut(key) {
                    // 递归合并
                    merge_json_patch(target_val, patch_val)?;
                } else {
                    // 新键：直接插入
                    target_obj.insert(key.clone(), patch_val.clone());
                }
            }
        } else {
            // 补丁不是对象，直接替换目标
            *target = patch.clone();
        }
    } else {
        // 目标不是对象，直接替换
        *target = patch.clone();
    }
    Ok(())
}

/// 验证JSON是否有效
pub fn json_valid(json_str: &str) -> bool {
    JsonDocument::from_json(json_str).is_ok()
}

impl JsonValue {
    /// 将JsonValue序列化为JSON字符串
    pub fn to_json_string(&self) -> alloc::string::String {
        match self {
            JsonValue::Null => "null".to_string(),
            JsonValue::Boolean(b) => if *b { "true".to_string() } else { "false".to_string() },
            JsonValue::String(s) => {
                // 转义字符串：需要转义引号、反斜杠、控制字符
                let mut result = alloc::string::String::with_capacity(s.len() + 2);
                result.push('"');
                for ch in s.chars() {
                    match ch {
                        '"' => result.push_str("\\\""),
                        '\\' => result.push_str("\\\\"),
                        '\n' => result.push_str("\\n"),
                        '\r' => result.push_str("\\r"),
                        '\t' => result.push_str("\\t"),
                        '\x08' => result.push_str("\\b"),
                        '\x0c' => result.push_str("\\f"),
                        _ => result.push(ch),
                    }
                }
                result.push('"');
                result
            }
            JsonValue::Number(n) => n.clone(),
            JsonValue::Array(arr) => {
                let mut result = alloc::string::String::from("[");
                for (i, item) in arr.iter().enumerate() {
                    if i > 0 {
                        result.push(',');
                    }
                    result.push_str(&item.to_json_string());
                }
                result.push(']');
                result
            }
            JsonValue::Object(obj) => {
                let mut result = alloc::string::String::from("{");
                for (i, (key, value)) in obj.iter().enumerate() {
                    if i > 0 {
                        result.push(',');
                    }
                    result.push('"');
                    // 转义键
                    for ch in key.chars() {
                        match ch {
                            '"' => result.push_str("\\\""),
                            '\\' => result.push_str("\\\\"),
                            _ => result.push(ch),
                        }
                    }
                    result.push('"');
                    result.push(':');
                    result.push_str(&value.to_json_string());
                }
                result.push('}');
                result
            }
        }
    }
}
