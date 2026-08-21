// 发布者管理

use super::Result;
use super::PubSubError;
use super::protocol::{ProtocolFrame, FrameType};

// 待重传数据结构体
struct PendingData {
    // 帧数据
    frame: ProtocolFrame,
    // 发送时间戳
    send_time: u64,
    // 重传次数
    retransmit_count: usize,
    // 已确认标记
    acknowledged: bool,
}

// 发布者结构体
pub struct Publisher {
    // 是否启用NACK机制
    enable_nack: bool,
    // 重传超时时间
    retransmit_timeout: core::time::Duration,
    // 最大重传次数
    max_retransmits: usize,
    // 下一个序列号
    next_seq_num: u32,
    // 待重传数据列表
    pending_data: alloc::vec::Vec<PendingData>,
}

impl Publisher {
    /// 创建新的发布者
    pub fn new(
        enable_nack: bool,
        retransmit_timeout: core::time::Duration,
        max_retransmits: usize
    ) -> Result<Self> {
        Ok(Self {
            enable_nack,
            retransmit_timeout,
            max_retransmits,
            next_seq_num: 1, // 序列号从1开始
            pending_data: alloc::vec::Vec::new(),
        })
    }
    
    /// 创建数据帧
    pub fn create_frame(&mut self, topic_id: u16, data: &[u8]) -> Result<ProtocolFrame> {
        // 获取当前序列号
        let seq_num = self.next_seq_num;
        self.next_seq_num += 1;
        
        // 创建数据帧
        let frame = ProtocolFrame::new_data_frame(seq_num, topic_id, data)?;
        
        // 如果启用NACK机制，将帧添加到待重传列表
        if self.enable_nack {
            self.pending_data.push(PendingData {
                frame: frame.clone(),
                send_time: Self::get_current_time(),
                retransmit_count: 0,
                acknowledged: false,
            });
        }
        
        Ok(frame)
    }
    
    /// 处理NACK帧
    pub fn handle_nack(&mut self, seq_num: u32, topic_id: u16) -> Result<Vec<ProtocolFrame>> {
        // 如果未启用NACK机制，直接返回
        if !self.enable_nack {
            return Ok(Vec::new());
        }
        
        // 查找待重传的数据
        let mut frames_to_retransmit = Vec::new();
        
        for pending in &mut self.pending_data {
            if pending.frame.seq_num() == seq_num && pending.frame.topic_id() == topic_id {
                // 检查重传次数
                if pending.retransmit_count < self.max_retransmits {
                    // 增加重传次数
                    pending.retransmit_count += 1;
                    // 更新发送时间
                    pending.send_time = Self::get_current_time();
                    // 添加到重传列表
                    frames_to_retransmit.push(pending.frame.clone());
                } else {
                    // 达到最大重传次数，标记为已确认（不再重传）
                    pending.acknowledged = true;
                }
                break;
            }
        }
        
        Ok(frames_to_retransmit)
    }
    
    /// 检查并处理超时的待重传数据
    pub fn check_timeouts(&mut self) -> Result<Vec<ProtocolFrame>> {
        // 如果未启用NACK机制，直接返回
        if !self.enable_nack {
            return Ok(Vec::new());
        }
        
        // 获取当前时间
        let current_time = Self::get_current_time();
        let timeout_ms = self.retransmit_timeout.as_millis() as u64;
        
        // 查找超时的数据
        let mut frames_to_retransmit = Vec::new();
        
        for pending in &mut self.pending_data {
            if !pending.acknowledged {
                let elapsed = current_time - pending.send_time;
                if elapsed > timeout_ms {
                    // 检查重传次数
                    if pending.retransmit_count < self.max_retransmits {
                        // 增加重传次数
                        pending.retransmit_count += 1;
                        // 更新发送时间
                        pending.send_time = current_time;
                        // 添加到重传列表
                        frames_to_retransmit.push(pending.frame.clone());
                    } else {
                        // 达到最大重传次数，标记为已确认（不再重传）
                        pending.acknowledged = true;
                    }
                }
            }
        }
        
        // 清理已确认的数据
        self.pending_data.retain(|pending| !pending.acknowledged);
        
        Ok(frames_to_retransmit)
    }
    
    /// 创建心跳帧
    pub fn create_heartbeat_frame(&self) -> ProtocolFrame {
        ProtocolFrame::new_heartbeat_frame()
    }
    
    /// 获取当前时间（毫秒）
    /// 注意：在baremetal平台上，需要用户提供时间实现
    fn get_current_time() -> u64 {
        #[cfg(feature = "std")] {
            // 使用系统时间
            let now = std::time::SystemTime::now();
            let duration = now.duration_since(std::time::UNIX_EPOCH).unwrap();
            duration.as_millis() as u64
        }
        #[cfg(not(feature = "std"))] {
            0
        }
    }
    
    /// 清除所有待重传数据
    pub fn clear_pending(&mut self) {
        self.pending_data.clear();
    }
    
    /// 获取待重传数据数量
    pub fn pending_count(&self) -> usize {
        self.pending_data.len()
    }
}

// 测试用例（仅在测试模式下编译）
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_create_frame() {
        // 创建发布者
        let mut publisher = Publisher::new(true, core::time::Duration::from_millis(100), 3).unwrap();
        
        // 创建数据帧
        let frame = publisher.create_frame(0, b"test data").unwrap();
        
        // 检查帧属性
        assert_eq!(frame.frame_type(), FrameType::Data);
        assert_eq!(frame.seq_num(), 1);
        assert_eq!(frame.topic_id(), 0);
        assert_eq!(frame.payload(), b"test data");
    }
    
    #[test]
    fn test_handle_nack() {
        // 创建发布者
        let mut publisher = Publisher::new(true, core::time::Duration::from_millis(100), 3).unwrap();
        
        // 创建数据帧
        let original_frame = publisher.create_frame(0, b"test data").unwrap();
        
        // 处理NACK
        let retransmit_frames = publisher.handle_nack(original_frame.seq_num(), original_frame.topic_id()).unwrap();
        
        // 检查重传帧
        assert_eq!(retransmit_frames.len(), 1);
        assert_eq!(retransmit_frames[0].seq_num(), original_frame.seq_num());
        assert_eq!(retransmit_frames[0].topic_id(), original_frame.topic_id());
        assert_eq!(retransmit_frames[0].payload(), original_frame.payload());
    }
    
    #[test]
    fn test_check_timeouts() {
        // 创建发布者，使用非常短的超时时间
        let mut publisher = Publisher::new(true, core::time::Duration::from_millis(1), 3).unwrap();
        
        // 创建数据帧
        let original_frame = publisher.create_frame(0, b"test data").unwrap();
        
        // 等待超时
        std::thread::sleep(std::time::Duration::from_millis(2));
        
        // 检查超时
        let retransmit_frames = publisher.check_timeouts().unwrap();
        
        // 检查重传帧
        assert_eq!(retransmit_frames.len(), 1);
        assert_eq!(retransmit_frames[0].seq_num(), original_frame.seq_num());
    }
    
    #[test]
    fn test_create_heartbeat_frame() {
        // 创建发布者
        let publisher = Publisher::new(false, core::time::Duration::from_millis(100), 3).unwrap();
        
        // 创建心跳帧
        let frame = publisher.create_heartbeat_frame();
        
        // 检查帧属性
        assert_eq!(frame.frame_type(), FrameType::Heartbeat);
        assert_eq!(frame.seq_num(), 0);
        assert_eq!(frame.topic_id(), 0);
        assert_eq!(frame.payload(), b"");
    }
}
