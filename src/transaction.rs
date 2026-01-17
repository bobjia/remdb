use core::ptr::NonNull;
use crate::types::{Result, RemDbError};
use crate::platform::{memcpy, memset};
use crate::defer;

// 引入alloc模块
extern crate alloc;
use alloc::vec::Vec;
use alloc::sync::Arc;

/// 事务隔离级别
#[derive(PartialEq)]
#[repr(u8)]
pub enum IsolationLevel {
    /// 未提交读
    ReadUncommitted = 0,
    /// 提交读
    ReadCommitted = 1,
    /// 可重复读
    RepeatableRead = 2,
    /// 串行化
    Serializable = 3,
}

/// 事务类型
#[derive(PartialEq)]
#[repr(u8)]
pub enum TransactionType {
    /// 只读事务
    ReadOnly = 0,
    /// 读写事务
    ReadWrite = 1,
}

/// 事务状态
#[derive(PartialEq)]
#[repr(u8)]
pub enum TransactionStatus {
    /// 活跃
    Active = 0,
    /// 已提交
    Committed = 1,
    /// 已回滚
    RolledBack = 2,
    /// 已准备
    Prepared = 3,
}

/// 事务日志操作类型
#[repr(u8)]
#[derive(Copy, Clone, Debug)]
pub enum LogOperation {
    /// 插入记录
    Insert = 0,
    /// 删除记录
    Delete = 1,
    /// 更新记录
    Update = 2,
    /// 时序数据插入
    TimeSeriesInsert = 3,
    /// 创建表
    CreateTable = 4,
    /// 事务提交
    Commit = 5,
    /// 事务回滚
    Abort = 6,
    /// 检查点
    Checkpoint = 7,
    /// 创建索引
    CreateIndex = 8,
}

/// 日志文件头
#[repr(C)]
#[derive(Copy, Clone)]
pub struct LogHeader {
    /// 魔数
    pub magic: u32, // 'LOGM'
    /// 版本号
    pub version: u32,
    /// 创建时间戳（微秒）
    pub created_at: u64,
    /// 日志记录数
    pub record_count: u32,
    /// 校验和
    pub checksum: u32,
}

/// 事务日志项
#[repr(C)]
#[derive(Copy, Clone)]
pub struct LogItem {
    /// 操作类型
    pub op_type: LogOperation,
    /// 表ID
    pub table_id: u8,
    /// 记录ID
    pub record_id: u16,
    /// 数据大小
    pub data_size: u16,
    /// 旧数据（用于回滚）
    pub old_data: [u8; 512], // 最大记录大小512字节
    /// 新数据
    pub new_data: [u8; 512],
    /// 事务ID
    pub tx_id: u32,
    /// 时间戳（微秒）
    pub timestamp: u64,
    /// 校验和
    pub checksum: u32,
}

/// 日志检查点
#[repr(C)]
#[derive(Copy, Clone)]
pub struct LogCheckpoint {
    /// 检查点时间戳
    pub timestamp: u64,
    /// 已处理的日志记录数
    pub processed_records: u32,
    /// 校验和
    pub checksum: u32,
}

/// 事务上下文
pub struct Transaction {
    /// 事务ID
    pub id: u32,
    /// 事务类型
    pub tx_type: TransactionType,
    /// 事务状态
    pub status: TransactionStatus,
    /// 事务隔离级别
    pub isolation_level: IsolationLevel,
    /// 开始时间戳（微秒）
    pub start_time: u64,
    /// 日志项数组
    log_items: NonNull<LogItem>,
    /// 最大日志项数量
    max_log_items: usize,
    /// 当前日志项数量
    log_item_count: usize,
    /// 嵌套深度（不支持嵌套事务，固定为1）
    pub depth: u8,
    /// 自旋锁
    lock: u32,
}

/// 日志缓冲区配置
#[repr(C)]
pub struct LogBufferConfig {
    /// 缓冲区大小（日志项数量）
    pub size: usize,
    /// 刷新阈值（当缓冲区使用率超过此阈值时自动刷新）
    pub flush_threshold: usize,
}

/// 日志管理器
pub struct LogManager {
    /// 日志文件路径
    log_path: &'static str,
    /// 日志文件句柄
    log_handle: crate::platform::FileHandle,
    /// 日志头
    header: LogHeader,
    /// 检查点
    checkpoint: LogCheckpoint,
    /// 自旋锁
    lock: u32,
    /// 日志模式
    log_mode: crate::config::LogMode,
    /// 日志缓冲区
    log_buffer: alloc::vec::Vec<LogItem>,
    /// 缓冲区配置
    buffer_config: LogBufferConfig,
    /// 上次刷新时间
    last_flush_time: u64,
    /// 上次检查点时间
    last_checkpoint_time: u64,
    /// 检查点间隔
    checkpoint_interval_ms: u64,
    /// 日志文件大小限制
    log_file_size_limit: usize,
    /// 日志分段大小
    log_segment_size: usize,
}

impl LogManager {
    /// 创建新的日志管理器
    pub unsafe fn new(config: &crate::config::DbConfig) -> Result<Self> {
        // 构造完整的日志文件路径：log_path目录 + remdb.wal文件名
        let log_dir = config.wal_config.log_path;
        
        // 在no_std环境下使用alloc::format宏
        use alloc::format;
        let wal_file_path = format!("{}/remdb.wal", log_dir);
        
        // 确保日志目录存在（仅在std环境下）
        #[cfg(feature = "std")]
        {
            use std::path::Path;
            use std::fs;
            
            let log_path = Path::new(log_dir);
            if !log_path.exists() {
                fs::create_dir_all(log_path).unwrap_or(());
            }
        }
        
        // 尝试打开日志文件，如果不存在则创建
        let log_handle = crate::platform::file_open(
            wal_file_path.as_str(),
            crate::platform::FileMode::ReadWrite
        ).map_err(|_| RemDbError::FileIoError)?;
        
        // 获取当前时间
        let now = crate::platform::get_timestamp_us();
        let now_ms = now / 1000;
        
        let mut manager = LogManager {
            log_path: config.wal_config.log_path,
            log_handle,
            header: LogHeader {
                magic: 0x4C4F474D, // 'LOGM'
                version: 1,
                created_at: now,
                record_count: 0,
                checksum: 0,
            },
            checkpoint: LogCheckpoint {
                timestamp: 0,
                processed_records: 0,
                checksum: 0,
            },
            lock: 0,
            log_mode: config.wal_config.log_mode,
            log_buffer: alloc::vec::Vec::new(), // 默认缓冲区大小1024
            buffer_config: LogBufferConfig {
                size: 1024,
                flush_threshold: 800, // 80%使用率时刷新
            },
            last_flush_time: now,
            last_checkpoint_time: now_ms,
            checkpoint_interval_ms: config.wal_config.checkpoint_interval_ms,
            log_file_size_limit: config.wal_config.log_file_size_limit,
            log_segment_size: config.wal_config.log_segment_size,
        };
        
        // 预分配缓冲区空间
        manager.log_buffer.reserve(1024);
        
        // 读取日志头，如果文件为空或格式不正确则写入新的日志头
        let mut header_buffer = [0u8; core::mem::size_of::<LogHeader>()];
        let read = crate::platform::file_read(
            log_handle,
            header_buffer.as_mut_ptr(),
            header_buffer.len()
        ).map_err(|_| RemDbError::FileIoError)?;
        
        let mut header_valid = false;
        if read >= core::mem::size_of::<LogHeader>() {
            // 尝试读取日志头
            let header = core::ptr::read_unaligned(header_buffer.as_ptr() as *const LogHeader);
            
            // 验证魔数和版本号
            if header.magic == 0x4C4F474D && header.version == 1 {
                manager.header = header;
                // 尝试读取检查点
                if manager.read_checkpoint().is_ok() {
                    header_valid = true;
                }
            }
        }
        
        if !header_valid {
            // 文件为空或格式不正确，写入新的日志头
            // 直接回到文件开头，准备写入新的日志头
            crate::platform::file_seek(
                log_handle,
                0,
                crate::platform::SeekWhence::SeekSet
            ).map_err(|_| RemDbError::FileIoError)?;
            
            // 如果配置了预分配大小，则预分配文件空间
            if config.wal_config.log_prealloc_size > 0 {
                // 定位到预分配大小位置
                crate::platform::file_seek(
                    log_handle,
                    config.wal_config.log_prealloc_size as i64 - 1,
                    crate::platform::SeekWhence::SeekSet
                ).map_err(|_| RemDbError::FileIoError)?;
                
                // 写入一个字节来扩展文件
                let zero_byte = [0u8; 1];
                crate::platform::file_write(
                    log_handle,
                    zero_byte.as_ptr(),
                    1
                ).map_err(|_| RemDbError::FileIoError)?;
                
                // 回到文件开头
                crate::platform::file_seek(
                    log_handle,
                    0,
                    crate::platform::SeekWhence::SeekSet
                ).map_err(|_| RemDbError::FileIoError)?;
            }
            
            // 写入新的日志头
            manager.write_header()?;
        }
        
        Ok(manager)
    }
    
    /// 写入日志头
    pub unsafe fn write_header(&mut self) -> Result<()> {
        // 计算日志头校验和
        let mut header_bytes = [0u8; core::mem::size_of::<LogHeader>()];
        core::ptr::write_unaligned(
            header_bytes.as_mut_ptr() as *mut LogHeader,
            self.header
        );
        
        // 清除旧的校验和
        let checksum_ptr = header_bytes.as_mut_ptr().add(16) as *mut u32;
        *checksum_ptr = 0;
        
        // 计算新的校验和
        self.header.checksum = Transaction::calculate_checksum(&header_bytes);
        
        // 重新写入完整的日志头
        core::ptr::write_unaligned(
            header_bytes.as_mut_ptr() as *mut LogHeader,
            self.header
        );
        
        // 定位到文件开头
        crate::platform::file_seek(
            self.log_handle,
            0,
            crate::platform::SeekWhence::SeekSet
        ).map_err(|_| RemDbError::FileIoError)?;

        // 写入日志头
        let written = crate::platform::file_write(
            self.log_handle,
            header_bytes.as_ptr(),
            header_bytes.len()
        ).map_err(|_| RemDbError::FileIoError)?;
        
        if written != header_bytes.len() {
            return Err(RemDbError::FileIoError);
        }
        
        Ok(())
    }
    
    /// 读取检查点
    pub unsafe fn read_checkpoint(&mut self) -> Result<()> {
        // 定位到检查点位置（日志头之后）
        let checkpoint_offset = core::mem::size_of::<LogHeader>();
        crate::platform::file_seek(
            self.log_handle,
            checkpoint_offset as i64,
            crate::platform::SeekWhence::SeekSet
        ).map_err(|_| RemDbError::FileIoError)?;

        // 读取检查点
        let mut checkpoint_buffer = [0u8; core::mem::size_of::<LogCheckpoint>()];
        let read = crate::platform::file_read(
            self.log_handle,
            checkpoint_buffer.as_mut_ptr(),
            checkpoint_buffer.len()
        ).map_err(|_| RemDbError::FileIoError)?;
        
        if read == 0 {
            // 检查点不存在，使用默认值
            self.checkpoint = LogCheckpoint {
                timestamp: 0,
                processed_records: 0,
                checksum: 0,
            };
        } else {
            // 读取检查点
            self.checkpoint = core::ptr::read_unaligned(checkpoint_buffer.as_ptr() as *const LogCheckpoint);
        }
        
        Ok(())
    }
    
    /// 写入检查点
    pub unsafe fn write_checkpoint(&mut self) -> Result<()> {
        // 计算检查点校验和
        let mut checkpoint_bytes = [0u8; core::mem::size_of::<LogCheckpoint>()];
        core::ptr::write_unaligned(
            checkpoint_bytes.as_mut_ptr() as *mut LogCheckpoint,
            self.checkpoint
        );
        
        // 清除旧的校验和
        let checksum_ptr = checkpoint_bytes.as_mut_ptr().add(12) as *mut u32;
        *checksum_ptr = 0;
        
        // 计算新的校验和
        self.checkpoint.checksum = Transaction::calculate_checksum(&checkpoint_bytes);
        
        // 重新写入完整的检查点
        core::ptr::write_unaligned(
            checkpoint_bytes.as_mut_ptr() as *mut LogCheckpoint,
            self.checkpoint
        );
        
        // 定位到检查点位置
        let checkpoint_offset = core::mem::size_of::<LogHeader>();
        crate::platform::file_seek(
            self.log_handle,
            checkpoint_offset as i64,
            crate::platform::SeekWhence::SeekSet
        ).map_err(|_| RemDbError::FileIoError)?;

        // 写入检查点
        let written = crate::platform::file_write(
            self.log_handle,
            checkpoint_bytes.as_ptr(),
            checkpoint_bytes.len()
        ).map_err(|_| RemDbError::FileIoError)?;
        
        if written != checkpoint_bytes.len() {
            return Err(RemDbError::FileIoError);
        }
        
        Ok(())
    }
    
    /// 刷新日志缓冲区到磁盘
    pub unsafe fn flush_buffer(&mut self) -> Result<()> {
        if self.log_buffer.is_empty() {
            return Ok(());
        }
        
        // 自旋锁保护
        crate::platform::spin_lock(&mut self.lock);
        
        // 定位到日志记录区域的末尾
        let log_offset = core::mem::size_of::<LogHeader>() + 
                        core::mem::size_of::<LogCheckpoint>() + 
                        (self.header.record_count as usize) * core::mem::size_of::<LogItem>();
        
        crate::platform::file_seek(
            self.log_handle,
            log_offset as i64,
            crate::platform::SeekWhence::SeekSet
        ).map_err(|_| RemDbError::FileIoError)?;

        // 批量写入日志项
        let buffer_size = self.log_buffer.len();
        for i in 0..buffer_size {
            let log_item = &self.log_buffer[i];
            let mut log_bytes = [0u8; core::mem::size_of::<LogItem>()];
            core::ptr::write_unaligned(
                log_bytes.as_mut_ptr() as *mut LogItem,
                *log_item
            );
            
            let written = crate::platform::file_write(
                self.log_handle,
                log_bytes.as_ptr(),
                log_bytes.len()
            ).map_err(|_| RemDbError::FileIoError)?;
            
            if written != log_bytes.len() {
                crate::platform::spin_unlock(&mut self.lock);
                return Err(RemDbError::FileIoError);
            }
            
            // 更新日志头记录计数
            self.header.record_count += 1;
        }
        
        // 清空缓冲区
        self.log_buffer.clear();
        
        // 更新日志头校验和
        self.header.checksum = 0; // 会在write_header中重新计算
        
        // 释放锁，避免在write_header中再次借用冲突
        crate::platform::spin_unlock(&mut self.lock);
        
        self.write_header()?;
        
        // 更新上次刷新时间
        self.last_flush_time = crate::platform::get_timestamp_us();
        
        Ok(())
    }
    
    /// 写入日志项
    pub unsafe fn write_log_item(&mut self, log_item: &LogItem) -> Result<()> {
        // 检查是否需要刷新缓冲区或创建检查点
        self.check_flush_and_checkpoint()?;
        
        match self.log_mode {
            crate::config::LogMode::Sync => {
                // 自旋锁保护
                crate::platform::spin_lock(&mut self.lock);
                
                // 定位到日志记录区域的末尾
                let log_offset = core::mem::size_of::<LogHeader>() + 
                                core::mem::size_of::<LogCheckpoint>() + 
                                (self.header.record_count as usize) * core::mem::size_of::<LogItem>();
                
                crate::platform::file_seek(
                    self.log_handle,
                    log_offset as i64,
                    crate::platform::SeekWhence::SeekSet
                ).map_err(|_| RemDbError::FileIoError)?;

                // 写入日志项
                let mut log_bytes = [0u8; core::mem::size_of::<LogItem>()];
                core::ptr::write_unaligned(
                    log_bytes.as_mut_ptr() as *mut LogItem,
                    *log_item
                );
                
                let written = crate::platform::file_write(
                    self.log_handle,
                    log_bytes.as_ptr(),
                    log_bytes.len()
                ).map_err(|_| RemDbError::FileIoError)?;
                
                if written != log_bytes.len() {
                    crate::platform::spin_unlock(&mut self.lock);
                    return Err(RemDbError::FileIoError);
                }
                
                // 更新日志头
                self.header.record_count += 1;
                self.header.checksum = 0; // 会在write_header中重新计算
                
                // 释放锁，避免在write_header中再次借用冲突
                crate::platform::spin_unlock(&mut self.lock);
                
                self.write_header()?;
                
                // 触发WAL复制
                self.replicate_wal(log_item)?;
                
                // Publish to pubsub
                #[cfg(feature = "pubsub")]
                self.publish_to_pubsub(log_item)?;
                
                Ok(())
            },
            crate::config::LogMode::Async => {
                // 异步模式：先写入缓冲区
                self.log_buffer.push(*log_item);
                
                // 检查是否需要立即刷新
                if self.log_buffer.len() >= self.buffer_config.flush_threshold {
                    self.flush_buffer()?;
                }
                
                // 触发WAL复制
                self.replicate_wal(log_item)?;
                
                // Publish to pubsub
                #[cfg(feature = "pubsub")]
                self.publish_to_pubsub(log_item)?;
                
                Ok(())
            }
        }
    }
    
    /// 发布WAL日志到pubsub
    #[cfg(feature = "pubsub")]
    unsafe fn publish_to_pubsub(&self, log_item: &LogItem) -> Result<()> {
        use crate::pubsub::topics::*;
        
        // Serialize log_item to bytes
        let mut log_bytes = [0u8; core::mem::size_of::<LogItem>()];
        core::ptr::write_unaligned(
            log_bytes.as_mut_ptr() as *mut LogItem,
            *log_item
        );
        
        // Only publish to WAL_TOPIC
        if let Some(topic_id) = crate::pubsub::get_topic_id(WAL_TOPIC) {
            let _ = crate::pubsub::publish(topic_id, &log_bytes);
        }
        
        Ok(())
    }
    
    /// 复制WAL日志到从节点
    unsafe fn replicate_wal(&self, log_item: &LogItem) -> Result<()> {
        // 尝试获取HA管理器
        #[cfg(feature = "ha")]
        {
            if let Some(ha_manager) = crate::ha::get_ha_manager() {
                // 调用HA管理器复制WAL日志
                match ha_manager.replicate_wal(log_item) {
                    Ok(_) => Ok(()),
                    Err(_) => Ok(()), // 复制失败不影响主节点操作
                }
            } else {
                Ok(()) // HA管理器未初始化，跳过复制
            }
        }
        
        #[cfg(not(feature = "ha"))]
        Ok(()) // HA功能未启用，跳过复制
    }
    
    /// 检查是否需要刷新缓冲区或创建检查点
    pub unsafe fn check_flush_and_checkpoint(&mut self) -> Result<()> {
        let now = crate::platform::get_timestamp_us();
        let now_ms = now / 1000;
        
        // 检查是否需要刷新缓冲区（超过1秒未刷新）
        if (now - self.last_flush_time) > 1_000_000 && !self.log_buffer.is_empty() {
            self.flush_buffer()?;
        }
        
        // 检查是否需要创建检查点
        if (now_ms - self.last_checkpoint_time) >= self.checkpoint_interval_ms {
            self.create_checkpoint()?;
        }
        
        Ok(())
    }
    
    /// 读取日志项
    pub unsafe fn read_log_item(&self, index: u32) -> Result<LogItem> {
        // 检查索引有效性
        if index >= self.header.record_count {
            return Err(RemDbError::LogRecordNotFound);
        }
        
        // 定位到日志项位置
        let log_offset = core::mem::size_of::<LogHeader>() + 
                        core::mem::size_of::<LogCheckpoint>() + 
                        (index as usize) * core::mem::size_of::<LogItem>();
        
        // 构造完整的日志文件路径：log_path目录 + remdb.wal文件名
        use alloc::format;
        let wal_file_path = format!("{}/remdb.wal", self.log_path);
        
        let handle = crate::platform::file_open(
            wal_file_path.as_str(),
            crate::platform::FileMode::Read
        ).map_err(|_| RemDbError::FileIoError)?;
        
        defer! {
            let _ = crate::platform::file_close(handle);
        };
        
        crate::platform::file_seek(
            handle,
            log_offset as i64,
            crate::platform::SeekWhence::SeekSet
        ).map_err(|_| RemDbError::FileIoError)?;
        
        // 读取日志项
        let mut log_bytes = [0u8; core::mem::size_of::<LogItem>()];
        let read = crate::platform::file_read(
            handle,
            log_bytes.as_mut_ptr(),
            log_bytes.len()
        ).map_err(|_| RemDbError::FileIoError)?;
        
        if read != log_bytes.len() {
            return Err(RemDbError::FileIoError);
        }
        
        let log_item = core::ptr::read_unaligned(log_bytes.as_ptr() as *const LogItem);
        
        // 验证校验和：只使用新的基于字段的校验和计算方法
        let calculated_checksum = Transaction::calculate_log_item_checksum(&log_item);
        
        if log_item.checksum != calculated_checksum {
            return Err(RemDbError::LogChecksumError);
        }
        
        Ok(log_item)
    }
    
    /// 创建检查点
    pub unsafe fn create_checkpoint(&mut self) -> Result<()> {
        // 自旋锁保护
        crate::platform::spin_lock(&mut self.lock);
        
        // 更新检查点
        let now = crate::platform::get_timestamp_us();
        self.checkpoint = LogCheckpoint {
            timestamp: now,
            processed_records: self.header.record_count,
            checksum: 0, // 会在write_checkpoint中重新计算
        };
        
        // 创建检查点日志项
        let log_item = LogItem {
            op_type: LogOperation::Checkpoint,
            table_id: 0, // 检查点操作不关联特定表
            record_id: 0, // 检查点操作不关联特定记录
            data_size: 0, // 检查点操作没有数据
            old_data: [0; 512],
            new_data: [0; 512],
            tx_id: 0, // 检查点操作不关联特定事务
            timestamp: now,
            checksum: 0, // 后面会计算
        };
        
        // 计算校验和：直接基于字段计算，避免结构体填充问题
        let calculated_checksum = Transaction::calculate_log_item_checksum(&log_item);
        
        let mut final_log_item = log_item;
        final_log_item.checksum = calculated_checksum;
        
        // 写入检查点日志
        let log_offset = core::mem::size_of::<LogHeader>() + 
                        core::mem::size_of::<LogCheckpoint>() + 
                        (self.header.record_count as usize) * core::mem::size_of::<LogItem>();
        
        crate::platform::file_seek(
            self.log_handle,
            log_offset as i64,
            crate::platform::SeekWhence::SeekSet
        ).map_err(|_| RemDbError::FileIoError)?;

        // 写入日志项
        let mut log_bytes = [0u8; core::mem::size_of::<LogItem>()];
        core::ptr::write_unaligned(
            log_bytes.as_mut_ptr() as *mut LogItem,
            final_log_item
        );
        
        let written = crate::platform::file_write(
            self.log_handle,
            log_bytes.as_ptr(),
            log_bytes.len()
        ).map_err(|_| RemDbError::FileIoError)?;
        
        if written != log_bytes.len() {
            crate::platform::spin_unlock(&mut self.lock);
            return Err(RemDbError::FileIoError);
        }
        
        // 更新日志头记录计数
        self.header.record_count += 1;
        self.header.checksum = 0; // 会在write_header中重新计算
        
        // 释放锁，避免在write_checkpoint中再次借用冲突
        crate::platform::spin_unlock(&mut self.lock);
        
        // 写入检查点
        self.write_checkpoint()?;
        
        // 更新日志头
        self.write_header()?;
        
        // 更新上次检查点时间
        self.last_checkpoint_time = now / 1000;
        
        Ok(())
    }
    
    /// 关闭日志管理器
    pub unsafe fn close(&mut self) -> Result<()> {
        // 刷新所有缓冲的日志
        self.flush_buffer()?;
        
        // 写入最终的检查点
        self.create_checkpoint()?;
        
        // 关闭文件句柄
        crate::platform::file_close(self.log_handle).map_err(|_| RemDbError::FileIoError)?;
        
        Ok(())
    }
    
    /// 恢复日志
    pub unsafe fn recover(&self, db: &mut crate::RemDb) -> Result<()> {
        // 构造完整的日志文件路径：log_path目录 + remdb.wal文件名
        use alloc::format;
        let wal_file_path = format!("{}/remdb.wal", self.log_path);
        
        // 获取文件大小
        let file_size = crate::platform::file_size(wal_file_path.as_str())
            .map_err(|_| RemDbError::FileIoError)?;
        
        // 计算实际可读取的日志项数量
        let header_size = core::mem::size_of::<LogHeader>();
        let checkpoint_size = core::mem::size_of::<LogCheckpoint>();
        let log_item_size = core::mem::size_of::<LogItem>();
        
        let total_header_size = header_size + checkpoint_size;
        let available_size = if file_size > total_header_size {
            file_size - total_header_size
        } else {
            0
        };
        
        let actual_record_count = available_size / log_item_size;
        let actual_record_count = actual_record_count as u32;
        
        // 总是从0开始处理所有日志记录，确保CreateTable操作被正确处理
        let start_index = 0;
        let end_index = actual_record_count;
        
        // 读取所有未处理的日志记录
        for i in start_index..end_index {
            match self.read_log_item(i) {
                Ok(log_item) => {
            
            // 根据日志类型执行相应的恢复操作
                match log_item.op_type {
                    LogOperation::Insert => {
                        // 执行插入操作
                        // 检查table_id是否在有效范围内
                        let table_id = log_item.table_id as usize;
                        if table_id >= db.tables.len() {
                            // 表可能还未创建，跳过当前日志项
                            println!("Warning: Table ID {} out of bounds (tables.len() = {}), skipping Insert log item", table_id, db.tables.len());
                            continue;
                        }
                        let table = match &mut db.tables[table_id] {
                            Some(table) => table,
                            None => {
                                println!("Warning: Table ID {} exists but is None, skipping Insert log item", table_id);
                                continue;
                            },
                        };
                        
                        // 检查记录是否已存在
                        let status_ptr = table.get_status_ptr(log_item.record_id as usize);
                        if (*status_ptr).status != crate::types::RecordStatus::Used {
                            // 记录不存在，执行插入
                            let record_ptr = table.get_record_ptr_mut(log_item.record_id as usize);
                            crate::platform::memcpy(
                                record_ptr,
                                log_item.new_data.as_ptr(),
                                log_item.data_size as usize
                            );
                            
                            (*status_ptr).status = crate::types::RecordStatus::Used;
                            (*status_ptr).version += 1;
                            (*status_ptr).create_tx_id = log_item.tx_id;
                            (*status_ptr).delete_tx_id = 0;
                            (*status_ptr).next_version_ptr = 0;
                            table.inc_record_count();
                        }
                    },
                    LogOperation::Delete => {
                        // 执行删除操作
                        // 检查table_id是否在有效范围内
                        let table_id = log_item.table_id as usize;
                        if table_id >= db.tables.len() {
                            // 表可能还未创建，跳过当前日志项
                            println!("Warning: Table ID {} out of bounds (tables.len() = {}), skipping Delete log item", table_id, db.tables.len());
                            continue;
                        }
                        let table = match &mut db.tables[table_id] {
                            Some(table) => table,
                            None => {
                                println!("Warning: Table ID {} exists but is None, skipping Delete log item", table_id);
                                continue;
                            },
                        };
                        
                        // 检查记录是否存在
                        let status_ptr = table.get_status_ptr(log_item.record_id as usize);
                        if (*status_ptr).status == crate::types::RecordStatus::Used {
                            // 与实际delete方法保持一致：直接标记为Free
                            (*status_ptr).status = crate::types::RecordStatus::Free;
                            (*status_ptr).version += 1;
                            
                            // 清空记录数据
                            let record_ptr = table.get_record_ptr_mut(log_item.record_id as usize);
                            crate::platform::memset(record_ptr, 0, table.record_size);
                            
                            // 将空闲槽压回栈中，确保不超过数组大小
                            if table.free_slot_count < table.def.max_records {
                                *table.free_slots.as_ptr().add(table.free_slot_count) = log_item.record_id as usize;
                                table.free_slot_count += 1;
                            }
                            
                            // 更新记录计数
                            table.record_count -= 1;
                        }
                    },
                    LogOperation::Update => {
                        // 执行更新操作
                        // 检查table_id是否在有效范围内
                        let table_id = log_item.table_id as usize;
                        if table_id >= db.tables.len() {
                            // 表可能还未创建，跳过当前日志项
                            println!("Warning: Table ID {} out of bounds (tables.len() = {}), skipping Update log item", table_id, db.tables.len());
                            continue;
                        }
                        let table = match &mut db.tables[table_id] {
                            Some(table) => table,
                            None => {
                                println!("Warning: Table ID {} exists but is None, skipping Update log item", table_id);
                                continue;
                            },
                        };
                        
                        // 检查记录是否存在
                        let status_ptr = table.get_status_ptr(log_item.record_id as usize);
                        if (*status_ptr).status == crate::types::RecordStatus::Used {
                            // 记录存在，执行更新
                            let record_ptr = table.get_record_ptr_mut(log_item.record_id as usize);
                            crate::platform::memcpy(
                                record_ptr,
                                log_item.new_data.as_ptr(),
                                log_item.data_size as usize
                            );
                            
                            (*status_ptr).version += 1;
                            (*status_ptr).create_tx_id = log_item.tx_id;
                        }
                    },
                    LogOperation::TimeSeriesInsert => {
                        // 执行时间序列插入操作
                        let ts_table = match &mut db.time_series_tables[log_item.table_id as usize] {
                            Some(table) => table,
                            None => return Err(RemDbError::TableNotFound),
                        };
                        
                        // 从日志中解析出时间序列记录
                        let mut record = crate::time_series::TimeSeriesRecord {
                            timestamp: 0,
                            value: 0.0,
                            tag_count: 0,
                            tags: [0; 8],
                        };
                        crate::platform::memcpy(
                            &mut record as *mut _ as *mut u8,
                            log_item.new_data.as_ptr(),
                            core::mem::size_of::<crate::time_series::TimeSeriesRecord>()
                        );
                        
                        // 获取或创建分区
                        let mut partitions_guard = ts_table.partitions.lock().unwrap();
                        let partition = partitions_guard.get_or_create_partition(record.timestamp);
                        
                        // 写入记录到分区
                        let mut partition_guard = partition.lock().unwrap();
                        partition_guard.records.push(record);
                        partition_guard.stats.record_count = partition_guard.records.len();
                        
                        // 更新索引
                        ts_table.index.insert(record.timestamp, partition_guard.records.len() - 1);
                    },
                    LogOperation::CreateTable => {
                        // 检查表是否已经存在
                        let table_id = log_item.table_id as usize;
                        if table_id < db.tables.len() && db.tables[table_id].is_some() {
                            // 表已经存在（从配置创建），跳过CreateTable操作
                            println!("Skipping CreateTable operation for table_id {} (table already exists)", log_item.table_id);
                            continue;
                        }
                        
                        // 表不存在，需要从WAL恢复CreateTable操作
                        // 从日志中解析表定义
                        let mut table_def = core::mem::MaybeUninit::<crate::TableDef>::uninit();
                        unsafe {
                            crate::platform::memcpy(
                                table_def.as_mut_ptr() as *mut u8,
                                log_item.new_data.as_ptr(),
                                core::mem::size_of::<crate::TableDef>()
                            );
                        };
                        let mut table_def = unsafe { table_def.assume_init() };
                        
                        // 创建新表
                        println!("Creating table from WAL for table_id {} (table name: {})", log_item.table_id, table_def.name);
                        let table = match crate::table::MemoryTable::new(alloc::sync::Arc::new(table_def)) {
                            Ok(table) => table,
                            Err(err) => {
                                println!("Warning: Failed to create table from WAL: {:?}, skipping CreateTable operation for table_id {}", err, log_item.table_id);
                                continue;
                            }
                        };
                        
                        // 确保表数组有足够的空间
                        while db.tables.len() <= table_id {
                            db.tables.push(None);
                            db.primary_indices.push(None);
                            db.secondary_indices.push(None);
                        }
                        
                        // 插入新创建的表
                        db.tables[table_id] = Some(table);
                    },
                    LogOperation::CreateIndex => {
                        // 执行创建索引操作
                        // 从日志中解析表名和字段名
                        let table_name_len = log_item.new_data[0] as usize;
                        let table_name = core::str::from_utf8(&log_item.new_data[1..1+table_name_len]).unwrap_or("unknown");
                        
                        let field_name_len = log_item.new_data[65] as usize;
                        let field_name = core::str::from_utf8(&log_item.new_data[66..66+field_name_len]).unwrap_or("unknown");
                        
                        let index_type: crate::types::IndexType = log_item.new_data[130].into();
                        
                        // 调用数据库的create_index方法
                        let _ = db.create_index(table_name, field_name, index_type);
                    },
                    LogOperation::Commit => {
                        // 提交操作不需要特殊处理，只需要确保事务的一致性
                        // 事务日志已经包含了所有需要的操作
                    },
                    LogOperation::Abort => {
                        // 回滚操作不需要特殊处理，因为恢复过程会重新执行所有未提交的操作
                        // 而提交的事务已经被应用到数据库中
                    },
                    LogOperation::Checkpoint => {
                        // 检查点操作不需要特殊处理，它只是标记了一个恢复点
                    },
                }
                },
                Err(err) => {
                    // 遇到错误（如校验和错误），跳过当前日志项，继续处理下一个
                    println!("Warning: Failed to read log item {}: {:?}, skipping...", i, err);
                }
            }
        }
        
        Ok(())
    }
}

/// 为 LogManager 添加 Drop 实现，确保在丢弃时关闭日志文件句柄
impl Drop for LogManager {
    fn drop(&mut self) {
        // 关闭日志文件句柄
        unsafe {
            let _ = crate::platform::file_close(self.log_handle);
        }
    }
}

/// 活跃事务快照
#[derive(Copy, Clone)]
pub struct ActiveSnapshot {
    /// 事务ID
    pub tx_id: u32,
    /// 快照版本号
    pub snapshot_version: u32,
}

/// 事务管理器
pub struct TransactionManager {
    /// 当前事务
    current_tx: Option<NonNull<Transaction>>,
    /// 事务ID计数器
    pub tx_id_counter: u32,
    /// 全局快照版本号
    pub snapshot_version: u32,
    /// 活跃事务快照列表
    active_snapshots: alloc::vec::Vec<ActiveSnapshot>,
    /// 自旋锁
    lock: u32,
    /// 日志管理器
    log_manager: Option<LogManager>,
    /// 是否处于低功耗模式
    low_power_mode: bool,
}

impl TransactionManager {
    /// 创建新的事务管理器
    pub const fn new() -> Self {
        TransactionManager {
            current_tx: None,
            tx_id_counter: 0,
            snapshot_version: 0,
            active_snapshots: alloc::vec::Vec::new(),
            lock: 0,
            log_manager: None,
            low_power_mode: false,
        }
    }
    
    /// 设置日志管理器
    pub unsafe fn set_log_manager(&mut self, log_manager: LogManager) {
        self.log_manager = Some(log_manager);
    }
    
    /// 刷新日志缓冲区
    pub unsafe fn flush_logs(&mut self) -> Result<()> {
        if let Some(log_manager) = &mut self.log_manager {
            log_manager.flush_buffer()
        } else {
            Ok(())
        }
    }
    
    /// 获取日志管理器
    pub fn get_log_manager(&self) -> Option<&LogManager> {
        self.log_manager.as_ref()
    }
    
    /// 获取日志管理器（可变）
    pub fn get_log_manager_mut(&mut self) -> Option<&mut LogManager> {
        self.log_manager.as_mut()
    }
    
    /// 清除日志管理器，释放资源
    pub fn clear_log_manager(&mut self) {
        self.log_manager = None;
    }
    
    /// 设置低功耗模式
    pub fn set_low_power_mode(&mut self, enabled: bool) {
        self.low_power_mode = enabled;
    }
    
    /// 获取低功耗模式状态
    pub fn is_low_power_mode(&self) -> bool {
        self.low_power_mode
    }
    
    /// 检查是否有活跃事务
    pub fn has_active_tx(&self) -> bool {
        self.current_tx.is_some()
    }
    
    /// 开始事务
    pub unsafe fn begin(
        &mut self,
        tx_type: TransactionType,
        isolation_level: IsolationLevel,
        tx_buffer: *mut Transaction,
        log_buffer: *mut LogItem,
        max_log_items: usize
    ) -> Result<NonNull<Transaction>> {
        // 增加事务计数
        crate::get_global_db().map(|db| db.metrics.inc_transactions());
        // 自旋锁保护
        crate::platform::spin_lock(&mut self.lock);
        defer! { crate::platform::spin_unlock(&mut self.lock); }
        
        // 检查是否已经有活跃事务（不支持嵌套事务）
        if self.current_tx.is_some() {
            return Err(RemDbError::TransactionError);
        }
        
        // 更新事务ID计数器
        let tx_id = self.tx_id_counter;
        self.tx_id_counter += 1;
        
        // 生成快照版本号
        let snapshot_version = self.snapshot_version;
        
        // 检查外部缓冲区是否有效
        if !tx_buffer.is_null() && !log_buffer.is_null() && max_log_items > 0 {
            // 测试环境：使用外部提供的缓冲区初始化事务对象
            // 设置事务属性
            (*tx_buffer).id = tx_id;
            (*tx_buffer).tx_type = tx_type;
            (*tx_buffer).status = TransactionStatus::Active;
            (*tx_buffer).isolation_level = isolation_level;
            (*tx_buffer).start_time = crate::platform::get_timestamp_us();
            (*tx_buffer).log_items = NonNull::new_unchecked(log_buffer);
            (*tx_buffer).max_log_items = max_log_items;
            (*tx_buffer).log_item_count = 0;
            (*tx_buffer).depth = 1;
            (*tx_buffer).lock = 0;
            
            // 保存当前事务引用
            self.current_tx = Some(NonNull::new_unchecked(tx_buffer));
            
            Ok(NonNull::new_unchecked(tx_buffer))
        } else {
            Err(RemDbError::TransactionError)
        }
    }
    
    /// 提交事务
    pub unsafe fn commit(&mut self) -> Result<()> {
        // 自旋锁保护
        crate::platform::spin_lock(&mut self.lock);
        
        // 检查是否有活跃事务
        let mut current_tx = match self.current_tx.take() {
            Some(tx) => tx,
            None => {
                crate::platform::spin_unlock(&mut self.lock);
                return Err(RemDbError::TransactionError);
            },
        };
        
        // 更新事务状态
        current_tx.as_mut().status = TransactionStatus::Committed;
        
        // 更新快照版本号
        self.snapshot_version += 1;
        
        // 清除活跃事务引用
        self.current_tx = None;
        
        // 解锁，因为flush_logs可能会重新获取锁
        crate::platform::spin_unlock(&mut self.lock);
        
        // 刷新日志
        self.flush_logs()?;
        
        Ok(())
    }
    
    /// 回滚事务
    pub unsafe fn rollback(&mut self) -> Result<()> {
        // 自旋锁保护
        crate::platform::spin_lock(&mut self.lock);
        
        // 检查是否有活跃事务
        let mut current_tx = match self.current_tx.take() {
            Some(tx) => tx,
            None => {
                crate::platform::spin_unlock(&mut self.lock);
                return Err(RemDbError::TransactionError);
            },
        };
        
        let tx = current_tx.as_mut();
        
        // 解锁事务管理器锁，因为我们需要调用其他函数可能会获取自己的锁
        crate::platform::spin_unlock(&mut self.lock);
        
        // 执行实际的回滚操作：遍历日志项，从后往前恢复数据
        for i in (0..tx.log_item_count).rev() {
            let log_item = &*(tx.log_items.as_ptr().add(i));
            
            // 获取全局数据库实例（在循环内，避免长时间持有引用）
            if let Some(db) = crate::get_global_db() {
                // 根据日志操作类型执行相应的回滚操作
                match log_item.op_type {
                    LogOperation::Insert => {
                        // 回滚插入操作：删除记录
                        let table_id = log_item.table_id as usize;
                        if table_id < db.tables.len() {
                            if let Some(ref mut table) = db.tables[table_id] {
                                table.delete(log_item.record_id as usize)?;
                            }
                        }
                    },
                    LogOperation::Update => {
                        // 回滚更新操作：恢复旧数据
                        let table_id = log_item.table_id as usize;
                        if table_id < db.tables.len() {
                            if let Some(ref mut table) = db.tables[table_id] {
                                let record_ptr = table.get_record_ptr_mut(log_item.record_id as usize);
                                crate::platform::memcpy(
                                    record_ptr,
                                    log_item.old_data.as_ptr(),
                                    log_item.data_size as usize
                                );
                                
                                // 更新记录状态版本
                                let status_ptr = table.get_status_ptr(log_item.record_id as usize);
                                (*status_ptr).version += 1;
                            }
                        }
                    },
                    LogOperation::Delete => {
                        // 回滚删除操作：重新插入记录
                        let table_id = log_item.table_id as usize;
                        if table_id < db.tables.len() {
                            if let Some(ref mut table) = db.tables[table_id] {
                                table.insert(log_item.old_data.as_ptr())?;
                            }
                        }
                    },
                    // 其他操作类型暂时不需要特殊回滚处理
                    _ => {}
                }
            }
        }
        
        // 重新获取锁来更新事务状态
        crate::platform::spin_lock(&mut self.lock);
        
        // 更新事务状态
        tx.status = TransactionStatus::RolledBack;
        
        // 清除活跃事务引用
        self.current_tx = None;
        
        // 解锁
        crate::platform::spin_unlock(&mut self.lock);
        
        Ok(())
    }
    
    /// 重置事务管理器状态
    pub fn reset(&mut self) {
        // 自旋锁保护
        crate::platform::spin_lock(&mut self.lock);
        defer! { crate::platform::spin_unlock(&mut self.lock); }
        
        // 清除所有活跃事务
        self.current_tx = None;
        self.active_snapshots.clear();
        
        // 重置事务计数
        self.tx_id_counter = 0;
        self.snapshot_version = 0;
    }
    
    /// 获取当前活跃事务
    pub fn get_current_tx(&self) -> Option<NonNull<Transaction>> {
        self.current_tx
    }
    
    /// 获取当前活跃事务（可变）
    pub fn get_current_tx_mut(&mut self) -> Option<NonNull<Transaction>> {
        self.current_tx
    }
    
    /// 检查记录是否对当前事务可见（MVCC实现）
    pub fn is_visible(&self, create_tx_id: u32, delete_tx_id: u32, current_tx_id: u32) -> bool {
        // MVCC可见性规则：
        // 1. 记录是由当前事务创建的，对当前事务可见
        // 2. 记录是由已提交事务创建的，且未被删除或被当前事务删除
        // 3. 记录是由已提交事务创建的，且删除事务尚未提交
        
        // 如果当前事务是创建者，记录可见
        if create_tx_id == current_tx_id {
            return true;
        }
        
        // 如果记录已被删除，且删除事务已提交，记录不可见
        if delete_tx_id > 0 && delete_tx_id < current_tx_id {
            return false;
        }
        
        // 如果记录是由已提交事务创建的，且未被删除，记录可见
        if create_tx_id < current_tx_id {
            return true;
        }
        
        // 其他情况，记录不可见
        false
    }
    
    /// 获取当前事务ID计数器
    pub fn tx_id_counter(&self) -> u32 {
        self.tx_id_counter
    }
}

impl Transaction {
    /// 计算校验和
    pub fn calculate_checksum(buffer: &[u8]) -> u32 {
        let mut checksum: u32 = 0;
        for byte in buffer {
            checksum = checksum.wrapping_add(*byte as u32);
        }
        checksum
    }
    
    /// 直接基于字段计算校验和，避免结构体填充问题
    pub fn calculate_log_item_checksum(log_item: &LogItem) -> u32 {
        let mut checksum: u32 = 0;
        
        // 计算操作类型的校验和
        checksum = checksum.wrapping_add(log_item.op_type as u32);
        
        // 计算表ID的校验和
        checksum = checksum.wrapping_add(log_item.table_id as u32);
        
        // 计算记录ID的校验和
        checksum = checksum.wrapping_add(log_item.record_id as u32);
        
        // 计算数据大小的校验和
        checksum = checksum.wrapping_add(log_item.data_size as u32);
        
        // 计算新数据的校验和
        for byte in log_item.new_data.iter().take(log_item.data_size as usize) {
            checksum = checksum.wrapping_add(*byte as u32);
        }
        
        // 计算事务ID的校验和
        checksum = checksum.wrapping_add(log_item.tx_id);
        
        // 计算时间戳的校验和
        checksum = checksum.wrapping_add((log_item.timestamp & 0xffffffff) as u32);
        checksum = checksum.wrapping_add((log_item.timestamp >> 32) as u32);
        
        checksum
    }
    
    /// 计算日志项的校验和
    pub unsafe fn calculate_checksum_for_item(&self, log_item: &LogItem) -> u32 {
        let mut buffer = [0u8; core::mem::size_of::<LogItem>()];
        core::ptr::write_unaligned(buffer.as_mut_ptr() as *mut LogItem, *log_item);
        
        Transaction::calculate_checksum(&buffer)
    }
    
    /// 开始事务日志项
    pub unsafe fn begin_log_item(
        &mut self,
        tx_id: u32,
        op_type: LogOperation,
        table_id: u8,
        record_id: u16,
        data_size: u16,
        old_data: Option<&[u8]>,
        new_data: Option<&[u8]>
    ) -> Option<NonNull<LogItem>> {
        if self.log_item_count >= self.max_log_items {
            return None;
        }
        
        let log_item_ptr = self.log_items.as_ptr().add(self.log_item_count);
        let log_item = log_item_ptr.as_mut().unwrap();
        
        // 初始化日志项
        log_item.op_type = op_type;
        log_item.table_id = table_id;
        log_item.record_id = record_id;
        log_item.data_size = data_size;
        log_item.old_data = [0; 512];
        log_item.new_data = [0; 512];
        log_item.tx_id = tx_id;
        log_item.timestamp = crate::platform::get_timestamp_us();
        log_item.checksum = 0;
        
        // 复制旧数据（如果有）
        if let Some(data) = old_data {
            let copy_len = core::cmp::min(data.len(), 512);
            crate::platform::memcpy(log_item.old_data.as_mut_ptr(), data.as_ptr(), copy_len);
        }
        
        // 复制新数据（如果有）
        if let Some(data) = new_data {
            let copy_len = core::cmp::min(data.len(), 512);
            crate::platform::memcpy(log_item.new_data.as_mut_ptr(), data.as_ptr(), copy_len);
        }
        
        // 计算校验和：直接基于字段计算，避免结构体填充问题
        let calculated_checksum = Transaction::calculate_log_item_checksum(log_item);
        log_item.checksum = calculated_checksum;
        
        self.log_item_count += 1;
        Some(NonNull::new_unchecked(log_item_ptr))
    }
    
    /// 提交事务日志项
    pub unsafe fn commit_log_item(&mut self) -> Result<()> {
        // 检查是否有活跃事务
        if self.status != TransactionStatus::Active {
            return Err(RemDbError::TransactionError);
        }
        
        // 遍历所有日志项
        for i in 0..self.log_item_count {
            let log_item = &self.log_items.as_ptr().add(i).as_ref().unwrap();
            
            // 写入日志项到日志管理器
            if let Some(log_manager) = crate::transaction::TX_MANAGER.get_log_manager_mut() {
                log_manager.write_log_item(log_item)?;
            }
        }
        
        Ok(())
    }
    
    /// 获取日志项计数
    pub fn log_item_count(&self) -> usize {
        self.log_item_count
    }
    
    /// 获取日志项
    pub unsafe fn get_log_item(&self, index: usize) -> Option<&LogItem> {
        if index < self.log_item_count {
            Some(&self.log_items.as_ptr().add(index).as_ref().unwrap())
        } else {
            None
        }
    }
    
    /// 检查事务是否活跃
    pub fn is_active(&self) -> bool {
        self.status == TransactionStatus::Active
    }
    
    /// 检查事务是否为只读
    pub fn is_read_only(&self) -> bool {
        self.tx_type == TransactionType::ReadOnly
    }
}

/// 事务管理器全局实例
static mut TX_MANAGER: TransactionManager = TransactionManager::new();

/// 获取全局事务管理器
pub fn get_tx_manager() -> &'static mut TransactionManager {
    unsafe {
        &mut TX_MANAGER
    }
}

/// 初始化事务管理器
pub fn init_tx_manager() {
    unsafe {
        TX_MANAGER.reset();
    }
}

/// 刷新所有日志
pub unsafe fn flush_all_logs() -> Result<()> {
    TX_MANAGER.flush_logs()
}

/// 设置全局日志管理器
pub unsafe fn set_log_manager(log_manager: LogManager) {
    TX_MANAGER.set_log_manager(log_manager);
}

/// 获取全局日志管理器
pub fn get_log_manager() -> Option<&'static mut LogManager> {
    unsafe {
        TX_MANAGER.get_log_manager_mut()
    }
}

/// 重置全局日志管理器
pub fn reset_log_manager() {
    unsafe {
        TX_MANAGER.clear_log_manager();
    }
}

/// 设置低功耗模式
pub fn set_low_power_mode(enabled: bool) {
    unsafe {
        TX_MANAGER.set_low_power_mode(enabled);
    }
}

/// 获取低功耗模式状态
pub fn is_low_power_mode() -> bool {
    unsafe {
        TX_MANAGER.is_low_power_mode()
    }
}

/// 检查是否有活跃事务
pub fn has_active_tx() -> bool {
    unsafe {
        TX_MANAGER.has_active_tx()
    }
}

/// 获取当前事务
pub unsafe fn get_current_tx() -> Option<NonNull<Transaction>> {
    TX_MANAGER.get_current_tx()
}

/// 开始事务
pub unsafe fn begin(
    tx_type: TransactionType,
    isolation_level: IsolationLevel,
    tx_buffer: *mut Transaction,
    log_buffer: *mut LogItem,
    max_log_items: usize
) -> Result<NonNull<Transaction>> {
    TX_MANAGER.begin(tx_type, isolation_level, tx_buffer, log_buffer, max_log_items)
}

/// 提交事务
pub unsafe fn commit() -> Result<()> {
    TX_MANAGER.commit()
}

/// 回滚事务
pub unsafe fn rollback() -> Result<()> {
    TX_MANAGER.rollback()
}

/// 检查记录是否对当前事务可见（MVCC实现）
pub fn is_visible(create_tx_id: u32, delete_tx_id: u32, current_tx_id: u32) -> bool {
    unsafe {
        TX_MANAGER.is_visible(create_tx_id, delete_tx_id, current_tx_id)
    }
}

/// 获取当前事务ID计数器
pub fn tx_id_counter() -> u32 {
    unsafe {
        TX_MANAGER.tx_id_counter()
    }
}