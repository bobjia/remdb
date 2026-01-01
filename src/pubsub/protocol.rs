// 协议帧定义与解析

use super::Result;
use super::PubSubError;
use super::crc32::calculate_crc32;

// 魔术字常量（4字节）
const MAGIC_WORD: u32 = 0x55AA55AA;

// 当前协议版本
const PROTOCOL_VERSION: u8 = 1;

// 帧类型枚举
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FrameType {
    // 数据帧
    Data = 0x01,
    // NACK帧（否定确认）
    Nack = 0x02,
    // 心跳帧
    Heartbeat = 0x03,
}

impl From<u8> for FrameType {
    fn from(value: u8) -> Self {
        match value {
            0x01 => FrameType::Data,
            0x02 => FrameType::Nack,
            0x03 => FrameType::Heartbeat,
            _ => FrameType::Data, // 默认值
        }
    }
}

// 协议帧结构体
#[derive(Debug, Clone)]
pub struct ProtocolFrame {
    // 魔术字（4字节）
    magic: u32,
    // 版本（1字节）
    version: u8,
    // 类型（1字节）
    frame_type: FrameType,
    // 序列号（4字节）
    seq_num: u32,
    // 主题ID（2字节）
    topic_id: u16,
    // 数据长度（2字节）
    data_len: u16,
    // CRC32校验和（4字节）
    crc32: u32,
    // 载荷数据
    payload: alloc::vec::Vec<u8>,
}

impl ProtocolFrame {
    // 协议帧头部大小
    pub const HEADER_SIZE: usize = 4 + 1 + 1 + 4 + 2 + 2 + 4;
    // 最大数据长度
    pub const MAX_DATA_LEN: usize = 4096;
    // 最大帧大小
    pub const MAX_FRAME_SIZE: usize = Self::HEADER_SIZE + Self::MAX_DATA_LEN;
    
    /// 创建新的数据帧
    pub fn new_data_frame(seq_num: u32, topic_id: u16, data: &[u8]) -> Result<Self> {
        // 检查数据长度
        if data.len() > Self::MAX_DATA_LEN {
            return Err(PubSubError::InvalidParameter);
        }
        
        // 计算CRC32
        let crc32 = calculate_crc32(data);
        
        Ok(Self {
            magic: MAGIC_WORD,
            version: PROTOCOL_VERSION,
            frame_type: FrameType::Data,
            seq_num,
            topic_id,
            data_len: data.len() as u16,
            crc32,
            payload: data.to_vec(),
        })
    }
    
    /// 创建新的NACK帧
    pub fn new_nack_frame(seq_num: u32, topic_id: u16) -> Self {
        Self {
            magic: MAGIC_WORD,
            version: PROTOCOL_VERSION,
            frame_type: FrameType::Nack,
            seq_num,
            topic_id,
            data_len: 0,
            crc32: 0, // NACK帧不需要CRC
            payload: alloc::vec::Vec::new(),
        }
    }
    
    /// 创建新的心跳帧
    pub fn new_heartbeat_frame() -> Self {
        Self {
            magic: MAGIC_WORD,
            version: PROTOCOL_VERSION,
            frame_type: FrameType::Heartbeat,
            seq_num: 0,
            topic_id: 0,
            data_len: 0,
            crc32: 0, // 心跳帧不需要CRC
            payload: alloc::vec::Vec::new(),
        }
    }
    
    /// 从字节数组解析协议帧
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        // 检查最小帧长度
        if bytes.len() < Self::HEADER_SIZE {
            return Err(PubSubError::InvalidFrameFormat);
        }
        
        // 解析魔术字
        let magic = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        if magic != MAGIC_WORD {
            return Err(PubSubError::InvalidFrameFormat);
        }
        
        // 解析版本
        let version = bytes[4];
        if version != PROTOCOL_VERSION {
            return Err(PubSubError::InvalidFrameFormat);
        }
        
        // 解析类型
        let frame_type = FrameType::from(bytes[5]);
        
        // 解析序列号
        let seq_num = u32::from_le_bytes(bytes[6..10].try_into().unwrap());
        
        // 解析主题ID
        let topic_id = u16::from_le_bytes(bytes[10..12].try_into().unwrap());
        
        // 解析数据长度
        let data_len = u16::from_le_bytes(bytes[12..14].try_into().unwrap());
        
        // 检查总帧长度
        if bytes.len() < Self::HEADER_SIZE + data_len as usize {
            return Err(PubSubError::InvalidFrameFormat);
        }
        
        // 解析CRC32
        let crc32 = u32::from_le_bytes(bytes[14..18].try_into().unwrap());
        
        // 解析载荷数据
        let payload = bytes[18..18 + data_len as usize].to_vec();
        
        // CRC校验（仅对数据帧）
        if frame_type == FrameType::Data {
            let calculated_crc = calculate_crc32(&payload);
            if calculated_crc != crc32 {
                return Err(PubSubError::CrcCheckFailed);
            }
        }
        
        Ok(Self {
            magic,
            version,
            frame_type,
            seq_num,
            topic_id,
            data_len,
            crc32,
            payload,
        })
    }
    
    /// 将协议帧转换为字节数组
    pub fn to_bytes(&self) -> alloc::vec::Vec<u8> {
        let mut bytes = alloc::vec::Vec::with_capacity(Self::HEADER_SIZE + self.data_len as usize);
        
        // 写入魔术字
        bytes.extend_from_slice(&self.magic.to_le_bytes());
        
        // 写入版本
        bytes.push(self.version);
        
        // 写入类型
        bytes.push(self.frame_type as u8);
        
        // 写入序列号
        bytes.extend_from_slice(&self.seq_num.to_le_bytes());
        
        // 写入主题ID
        bytes.extend_from_slice(&self.topic_id.to_le_bytes());
        
        // 写入数据长度
        bytes.extend_from_slice(&self.data_len.to_le_bytes());
        
        // 写入CRC32
        bytes.extend_from_slice(&self.crc32.to_le_bytes());
        
        // 写入载荷数据
        bytes.extend_from_slice(&self.payload);
        
        bytes
    }
    
    /// 获取帧类型
    pub fn frame_type(&self) -> FrameType {
        self.frame_type
    }
    
    /// 获取序列号
    pub fn seq_num(&self) -> u32 {
        self.seq_num
    }
    
    /// 获取主题ID
    pub fn topic_id(&self) -> u16 {
        self.topic_id
    }
    
    /// 获取载荷数据
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
    
    /// 获取数据长度
    pub fn data_len(&self) -> u16 {
        self.data_len
    }
}
