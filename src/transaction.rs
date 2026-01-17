use core::ptr::NonNull;
use crate::types::{Result, RemDbError};
use crate::platform::{memcpy, memset};
use crate::defer;

// 引入alloc模块
extern crate alloc;
use alloc::vec::Vec;

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
                        // 执行创建表操作
                        // 从日志中解析表名
                        let name_len = log_item.new_data[0] as usize;
                        let table_name = core::str::from_utf8(&log_item.new_data[1..1+name_len]).unwrap_or("unknown");
                        
                        // 从日志中解析字段数量
                        let field_count = log_item.new_data[65] as usize;
                        
                        // 从日志中解析主键索引
                        let primary_key = log_item.new_data[66] as usize;
                        
                        // 解析字段定义
                        let mut offset = 67;
                        let mut fields = Vec::with_capacity(field_count);
                        
                        for _ in 0..field_count {
                            // 解析字段名
                            let field_name_len = log_item.new_data[offset] as usize;
                            offset += 1;
                            let field_name = core::str::from_utf8(&log_item.new_data[offset..offset+field_name_len]).unwrap_or("unknown");
                            offset += 32; // 跳过固定32字节字段名空间
                            
                            // 解析数据类型
                            let data_type = crate::types::DataType::from(log_item.new_data[offset]);
                            offset += 1;
                            
                            // 解析字段约束
                            let constraints = log_item.new_data[offset];
                            offset += 1;
                            let primary_key_flag = (constraints & 0b0001) != 0;
                            let not_null_flag = (constraints & 0b0010) != 0;
                            let unique_flag = (constraints & 0b0100) != 0;
                            let auto_increment_flag = (constraints & 0b1000) != 0;
                            
                            // 解析默认值存在标志
                            let has_default = log_item.new_data[offset] != 0;
                            offset += 1;
                            
                            // 解析默认值（如果有）
                            let default_value = if has_default {
                                // 根据数据类型解析默认值
                                let mut value = crate::types::Value { u64: 0 };
                                match data_type {
                                    crate::types::DataType::Bool => {
                                        let bool_value = log_item.new_data[offset] != 0;
                                        offset += 1;
                                        unsafe { value.bool = bool_value; }
                                    },
                                    crate::types::DataType::Int8 => {
                                        let i8_value = i8::from_le_bytes([log_item.new_data[offset]]);
                                        offset += 1;
                                        unsafe { value.i8 = i8_value; }
                                    },
                                    crate::types::DataType::UInt8 => {
                                        let u8_value = log_item.new_data[offset];
                                        offset += 1;
                                        unsafe { value.u8 = u8_value; }
                                    },
                                    crate::types::DataType::Int16 => {
                                        let i16_value = i16::from_le_bytes([log_item.new_data[offset], log_item.new_data[offset+1]]);
                                        offset += 2;
                                        unsafe { value.i16 = i16_value; }
                                    },
                                    crate::types::DataType::UInt16 => {
                                        let u16_value = u16::from_le_bytes([log_item.new_data[offset], log_item.new_data[offset+1]]);
                                        offset += 2;
                                        unsafe { value.u16 = u16_value; }
                                    },
                                    crate::types::DataType::Int32 => {
                                        let i32_value = i32::from_le_bytes([log_item.new_data[offset], log_item.new_data[offset+1], log_item.new_data[offset+2], log_item.new_data[offset+3]]);
                                        offset += 4;
                                        unsafe { value.i32 = i32_value; }
                                    },
                                    crate::types::DataType::UInt32 => {
                                        let u32_value = u32::from_le_bytes([log_item.new_data[offset], log_item.new_data[offset+1], log_item.new_data[offset+2], log_item.new_data[offset+3]]);
                                        offset += 4;
                                        unsafe { value.u32 = u32_value; }
                                    },
                                    crate::types::DataType::Int64 => {
                                        let i64_value = i64::from_le_bytes([log_item.new_data[offset], log_item.new_data[offset+1], log_item.new_data[offset+2], log_item.new_data[offset+3], log_item.new_data[offset+4], log_item.new_data[offset+5], log_item.new_data[offset+6], log_item.new_data[offset+7]]);
                                        offset += 8;
                                        unsafe { value.i64 = i64_value; }
                                    },
                                    crate::types::DataType::UInt64 => {
                                        let u64_value = u64::from_le_bytes([log_item.new_data[offset], log_item.new_data[offset+1], log_item.new_data[offset+2], log_item.new_data[offset+3], log_item.new_data[offset+4], log_item.new_data[offset+5], log_item.new_data[offset+6], log_item.new_data[offset+7]]);
                                        offset += 8;
                                        unsafe { value.u64 = u64_value; }
                                    },
                                    crate::types::DataType::Float32 => {
                                        let float32_value = f32::from_le_bytes([log_item.new_data[offset], log_item.new_data[offset+1], log_item.new_data[offset+2], log_item.new_data[offset+3]]);
                                        offset += 4;
                                        unsafe { value.float32 = float32_value; }
                                    },
                                    crate::types::DataType::Float64 => {
                                        let float64_value = f64::from_le_bytes([log_item.new_data[offset], log_item.new_data[offset+1], log_item.new_data[offset+2], log_item.new_data[offset+3], log_item.new_data[offset+4], log_item.new_data[offset+5], log_item.new_data[offset+6], log_item.new_data[offset+7]]);
                                        offset += 8;
                                        unsafe { value.float64 = float64_value; }
                                    },
                                    crate::types::DataType::String => {
                                        let string_len = log_item.new_data[offset] as usize;
                                        offset += 1;
                                        let mut string_data = [0u8; 64];
                                        string_data[..string_len].copy_from_slice(&log_item.new_data[offset..offset+string_len]);
                                        offset += 64; // 跳过固定64字节字符串空间
                                        unsafe { value.string = string_data; }
                                    },
                                    crate::types::DataType::Timestamp | crate::types::DataType::TimestampTZ => {
                                        let timestamp_value = u64::from_le_bytes([log_item.new_data[offset], log_item.new_data[offset+1], log_item.new_data[offset+2], log_item.new_data[offset+3], log_item.new_data[offset+4], log_item.new_data[offset+5], log_item.new_data[offset+6], log_item.new_data[offset+7]]);
                                        offset += 8;
                                        unsafe { value.timestamp = timestamp_value; }
                                    },
                                    crate::types::DataType::Interval => {
                                        // 解析Interval类型，读取value、precision和flags
                                        let interval_value_bytes = log_item.new_data[offset..offset+8].try_into().unwrap();
                                        let interval_value = i64::from_le_bytes(interval_value_bytes);
                                        offset += 8;
                                        let precision = log_item.new_data[offset];
                                        offset += 1;
                                        let flags = log_item.new_data[offset];
                                        offset += 1;
                                        unsafe {
                                            value.interval = crate::types::db_interval {
                                                value: interval_value,
                                                precision,
                                                flags
                                            };
                                        }
                                    },
                                }
                                Some(value)
                            } else {
                                None
                            };
                            
                            // 添加字段到列表
                            fields.push((field_name, data_type, default_value));
                        }
                        
                        // 调用数据库的create_table方法
                        let _ = db.create_table(table_name, &fields, Some(primary_key));
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
            
            // 添加事务快照到活跃快照列表
            self.active_snapshots.push(ActiveSnapshot {
                tx_id,
                snapshot_version,
            });
            
            Ok(NonNull::new_unchecked(tx_buffer))
        } else {
            // JDBC服务器环境：只跟踪事务状态，不使用外部缓冲区
            // 创建一个简单的事务结构，使用内部状态管理
            // 注意：这种模式下不支持复杂的事务操作，只用于状态跟踪
            self.current_tx = Some(NonNull::dangling());
            
            // 添加事务快照到活跃快照列表
            self.active_snapshots.push(ActiveSnapshot {
                tx_id,
                snapshot_version,
            });
            
            Ok(NonNull::dangling())
        }
    }
    
    /// 提交事务
    pub unsafe fn commit(&mut self) -> Result<()> {
        // 增加已提交事务计数
        crate::get_global_db().map(|db| db.metrics.inc_committed_transactions());
        
        // 步骤1：获取活跃事务并更新其状态
        { // 锁作用域
            // 自旋锁保护
            crate::platform::spin_lock(&mut self.lock);
            
            // 检查是否有活跃事务
            let tx_ptr = match self.current_tx.take() {
                Some(tx) => tx,
                None => {
                    crate::platform::spin_unlock(&mut self.lock);
                    return Err(RemDbError::TransactionError);
                },
            };
            
            // 检查是否是悬垂指针（用于JDBC服务器）
            let is_dangling = tx_ptr.as_ptr() == NonNull::dangling().as_ptr();
            
            let tx_id = if !is_dangling {
                // 测试环境：更新事务状态
                let tx = &mut *tx_ptr.as_ptr();
                tx.status = TransactionStatus::Committed;
                tx.id
            } else {
                // JDBC服务器环境：获取事务ID
                self.tx_id_counter - 1
            };
            
            // 移除事务快照从活跃快照列表
            self.active_snapshots.retain(|snapshot| snapshot.tx_id != tx_id);
            
            // 增加全局快照版本号
            self.snapshot_version += 1;
            
            // 释放锁，避免与write_log_item中的锁冲突
            crate::platform::spin_unlock(&mut self.lock);
            
            // 步骤2：写入事务日志到WAL（在锁外执行，避免死锁）
            if !is_dangling {
                let tx = &mut *tx_ptr.as_ptr();
                
                // 记录提交日志
                if let Some(log_manager) = &mut self.log_manager {
                    // 先写入所有事务日志项
                    for i in 0..tx.log_item_count {
                        let log_ptr = tx.log_items.as_ptr().add(i);
                        let log_item = *log_ptr;
                        
                        // 写入日志项到WAL
                        log_manager.write_log_item(&log_item)?;
                    }
                    
                    // 创建提交日志项
                    let log_item = LogItem {
                        op_type: LogOperation::Commit,
                        table_id: 0, // 提交操作不关联特定表
                        record_id: 0, // 提交操作不关联特定记录
                        data_size: 0, // 提交操作没有数据
                        old_data: [0; 512],
                        new_data: [0; 512],
                        tx_id: tx.id,
                        timestamp: crate::platform::get_timestamp_us(),
                        checksum: 0, // 后面会计算
                    };
                    
                    // 计算校验和：直接基于字段计算，避免结构体填充问题
                    let calculated_checksum = Transaction::calculate_log_item_checksum(&log_item);
                    
                    let mut final_log_item = log_item;
                    final_log_item.checksum = calculated_checksum;
                    
                    // 写入提交日志
                    log_manager.write_log_item(&final_log_item)?;
                }
            }
        }
        
        Ok(())
    }
    
    /// 回滚事务
    pub unsafe fn rollback(&mut self, db: &mut crate::RemDb) -> Result<()> {
        // 增加已回滚事务计数
        crate::get_global_db().map(|db| db.metrics.inc_rolled_back_transactions());
        // 自旋锁保护
        crate::platform::spin_lock(&mut self.lock);
        defer! { crate::platform::spin_unlock(&mut self.lock); }
        
        // 检查是否有活跃事务
        let tx_ptr = match self.current_tx.take() {
            Some(tx) => tx,
            None => return Err(RemDbError::TransactionError),
        };
        
        // 检查是否是悬垂指针（用于JDBC服务器）
        let is_dangling = tx_ptr.as_ptr() == NonNull::dangling().as_ptr();
        
        if !is_dangling {
            // 测试环境：遍历事务日志，执行回滚操作
            let tx = &mut *tx_ptr.as_ptr();
            
            for i in (0..tx.log_item_count).rev() {
                let log_ptr = tx.log_items.as_ptr().add(i);
                let log_item = *log_ptr;
                
                // 根据日志类型执行相应的回滚操作
                match log_item.op_type {
                    LogOperation::Insert => {
                        // 回滚插入操作：删除记录
                        let table = match &mut db.tables[log_item.table_id as usize] {
                            Some(table) => table,
                            None => continue,
                        };
                        
                        let status_ptr = table.get_status_ptr(log_item.record_id as usize);
                        if (*status_ptr).status == crate::types::RecordStatus::Used {
                            // 执行删除操作
                            (*status_ptr).status = crate::types::RecordStatus::Free;
                            (*status_ptr).version += 1;
                            
                            let record_ptr = table.get_record_ptr_mut(log_item.record_id as usize);
                            crate::platform::memset(record_ptr, 0, log_item.data_size as usize);
                            
                            // 将空闲槽压回栈中
                            *table.free_slots.as_ptr().add(table.free_slot_count) = log_item.record_id as usize;
                            table.free_slot_count += 1;
                            
                            table.record_count -= 1;
                        }
                    },
                    LogOperation::Delete => {
                        // 回滚删除操作：恢复记录
                        let table = match &mut db.tables[log_item.table_id as usize] {
                            Some(table) => table,
                            None => continue,
                        };
                        
                        let status_ptr = table.get_status_ptr(log_item.record_id as usize);
                        if (*status_ptr).status == crate::types::RecordStatus::Free {
                            // 执行恢复操作
                            (*status_ptr).status = crate::types::RecordStatus::Used;
                            (*status_ptr).version += 1;
                            
                            let record_ptr = table.get_record_ptr_mut(log_item.record_id as usize);
                            crate::platform::memcpy(
                                record_ptr,
                                log_item.old_data.as_ptr(),
                                log_item.data_size as usize
                            );
                            
                            table.record_count += 1;
                        }
                    },
                    LogOperation::Update => {
                        // 回滚更新操作：恢复到旧值
                        let table = match &mut db.tables[log_item.table_id as usize] {
                            Some(table) => table,
                            None => continue,
                        };
                        
                        let status_ptr = table.get_status_ptr(log_item.record_id as usize);
                        if (*status_ptr).status == crate::types::RecordStatus::Used {
                            // 执行恢复操作
                            let record_ptr = table.get_record_ptr_mut(log_item.record_id as usize);
                            crate::platform::memcpy(
                                record_ptr,
                                log_item.old_data.as_ptr(),
                                log_item.data_size as usize
                            );
                            
                            (*status_ptr).version += 1;
                        }
                    },
                    LogOperation::TimeSeriesInsert => {
                        // 回滚时序数据插入：从分区中删除记录
                        let ts_table = match &mut db.time_series_tables[log_item.table_id as usize] {
                            Some(table) => table,
                            None => continue,
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
                        
                        // 获取分区
                        let partitions_guard = ts_table.partitions.lock().unwrap();
                        if let Some(partition) = partitions_guard.get_partition(record.timestamp) {
                            // 从分区中删除记录
                            let mut partition_guard = partition.lock().unwrap();
                            if let Some(index) = partition_guard.records.iter().position(|r| r.timestamp == record.timestamp) {
                                partition_guard.records.remove(index);
                                partition_guard.stats.record_count = partition_guard.records.len();
                                
                                // 更新索引
                                ts_table.index.remove(record.timestamp);
                            }
                        }
                    },
                    _ => continue,
                }
            }
            
            // 记录回滚日志
            if let Some(log_manager) = &mut self.log_manager {
                // 创建回滚日志项
                let log_item = LogItem {
                    op_type: LogOperation::Abort,
                    table_id: 0, // 回滚操作不关联特定表
                    record_id: 0, // 回滚操作不关联特定记录
                    data_size: 0, // 回滚操作没有数据
                    old_data: [0; 512],
                    new_data: [0; 512],
                    tx_id: tx.id,
                    timestamp: crate::platform::get_timestamp_us(),
                    checksum: 0, // 后面会计算
                };
                
                // 计算校验和：直接基于字段计算，避免结构体填充问题
                let calculated_checksum = Transaction::calculate_log_item_checksum(&log_item);
                
                let mut final_log_item = log_item;
                final_log_item.checksum = calculated_checksum;
                
                // 写入日志
                log_manager.write_log_item(&final_log_item)?;
            }
            
            // 更新事务状态
            tx.status = TransactionStatus::RolledBack;
        }
        
        // 获取事务ID
        let tx_id = if !is_dangling {
            let tx = &mut *tx_ptr.as_ptr();
            tx.id
        } else {
            // JDBC服务器环境：获取事务ID
            self.tx_id_counter - 1
        };
        
        // 移除事务快照从活跃快照列表
        self.active_snapshots.retain(|snapshot| snapshot.tx_id != tx_id);
        
        Ok(())
    }
    
    /// 获取当前事务
    pub fn get_current_tx(&self) -> Option<NonNull<Transaction>> {
        self.current_tx
    }
    
    /// 检查是否有活跃事务
    pub fn has_active_tx(&self) -> bool {
        self.current_tx.is_some()
    }
    
    /// 可见性判断：检查记录版本是否对当前事务可见
    pub fn is_visible(&self, create_tx_id: u32, delete_tx_id: u32, tx_id: u32) -> bool {
        // 可见性规则：
        // 1. 记录创建事务ID <= 当前事务ID，且记录删除事务ID > 当前事务ID或为0
        // 2. 记录创建事务已提交，且记录删除事务未提交或不存在
        create_tx_id <= tx_id && (delete_tx_id == 0 || delete_tx_id > tx_id)
    }
    
    /// 获取活跃事务中最小的事务ID
    pub fn get_min_active_tx_id(&self) -> u32 {
        if self.active_snapshots.is_empty() {
            self.tx_id_counter
        } else {
            self.active_snapshots.iter().map(|s| s.tx_id).min().unwrap_or(self.tx_id_counter)
        }
    }
    
    /// 垃圾回收检查：判断记录版本是否可以被回收
    pub fn can_recycle(&self, create_tx_id: u32, delete_tx_id: u32) -> bool {
        // 回收规则：
        // 1. 记录创建事务ID < 最小活跃事务ID
        // 2. 记录删除事务ID < 最小活跃事务ID（如果有删除事务）
        let min_active_tx_id = self.get_min_active_tx_id();
        create_tx_id < min_active_tx_id && (delete_tx_id == 0 || delete_tx_id < min_active_tx_id)
    }
    
    /// 检测写入冲突：检查记录是否被其他事务修改
    pub fn detect_write_conflict(&self, create_tx_id: u32, current_tx_id: u32) -> bool {
        // 冲突规则：
        // 1. 记录创建事务ID > 当前事务ID
        // 2. 表示该记录已被更晚开始的事务修改
        create_tx_id > current_tx_id
    }
    
    /// 重置事务管理器
    pub unsafe fn reset(&mut self) {
        self.current_tx = None;
        self.tx_id_counter = 0;
        self.snapshot_version = 0;
        self.active_snapshots.clear();
    }
}

impl Transaction {
    /// 计算数据校验和（通用）
    pub fn calculate_checksum(data: &[u8]) -> u32 {
        // 简单的XOR校验和实现，适合嵌入式环境
        let mut checksum = 0u32;
        for &byte in data {
            checksum ^= byte as u32;
        }
        checksum
    }
    
    /// 计算LogItem的校验和（直接基于字段，避免结构体填充问题）
    pub fn calculate_log_item_checksum(log_item: &LogItem) -> u32 {
        let mut checksum = 0u32;
        
        // 计算各个字段的校验和
        checksum ^= log_item.op_type as u32;
        checksum ^= log_item.table_id as u32;
        checksum ^= log_item.record_id as u32;
        checksum ^= log_item.data_size as u32;
        
        // 计算old_data的校验和
        for &byte in &log_item.old_data {
            checksum ^= byte as u32;
        }
        
        // 计算new_data的校验和
        for &byte in &log_item.new_data {
            checksum ^= byte as u32;
        }
        
        checksum ^= log_item.tx_id;
        checksum ^= log_item.timestamp as u32;
        // 不包含checksum字段本身
        
        checksum
    }
    
    /// 添加日志项
    pub unsafe fn add_log_item(
        &mut self,
        op_type: LogOperation,
        table_id: u8,
        record_id: u16,
        old_data: *const u8,
        new_data: *const u8,
        data_size: usize
    ) -> Result<()> {
        // 自旋锁保护
        crate::platform::spin_lock(&mut self.lock);
        defer! { crate::platform::spin_unlock(&mut self.lock); }
        
        // 检查事务状态
        if self.status != TransactionStatus::Active {
            return Err(RemDbError::TransactionError);
        }
        
        // 检查日志项数量
        if self.log_item_count >= self.max_log_items {
            return Err(RemDbError::OutOfMemory);
        }
        
        // 检查数据大小
        if data_size > 512 {
            return Err(RemDbError::UnsupportedOperation);
        }
        
        // 获取日志项指针
        let log_ptr = self.log_items.as_ptr().add(self.log_item_count);
        
        // 设置日志项基本信息
        (*log_ptr).op_type = op_type;
        (*log_ptr).table_id = table_id;
        (*log_ptr).record_id = record_id;
        (*log_ptr).data_size = data_size as u16;
        (*log_ptr).tx_id = self.id;
        (*log_ptr).timestamp = crate::platform::get_timestamp_us();
        
        // 拷贝旧数据
        if old_data.is_null() {
            memset((*log_ptr).old_data.as_mut_ptr(), 0, data_size);
        } else {
            memcpy((*log_ptr).old_data.as_mut_ptr(), old_data, data_size);
        }
        
        // 拷贝新数据
        if new_data.is_null() {
            memset((*log_ptr).new_data.as_mut_ptr(), 0, data_size);
        } else {
            memcpy((*log_ptr).new_data.as_mut_ptr(), new_data, data_size);
        }
        
        // 计算校验和：使用基于字段的校验和计算方法
        let calculated_checksum = Transaction::calculate_log_item_checksum(&*log_ptr);
        (*log_ptr).checksum = calculated_checksum;
        
        // 更新日志项计数
        self.log_item_count += 1;
        
        Ok(())
    }
    
    /// 获取事务持续时间（微秒）
    pub fn duration_us(&self) -> u64 {
        let current_time = crate::platform::get_timestamp_us();
        current_time - self.start_time
    }
    
    /// 获取日志项数量
    pub fn log_item_count(&self) -> usize {
        self.log_item_count
    }
    
    /// 检查事务是否只读
    pub fn is_read_only(&self) -> bool {
        self.tx_type == TransactionType::ReadOnly
    }
    
    /// 检查事务是否活跃
    pub fn is_active(&self) -> bool {
        self.status == TransactionStatus::Active
    }
}

/// 全局事务管理器
pub static mut TX_MANAGER: TransactionManager = TransactionManager::new();

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
pub unsafe fn rollback(db: &mut crate::RemDb) -> Result<()> {
    TX_MANAGER.rollback(db)
}

/// 获取当前事务
pub fn get_current_tx() -> Option<NonNull<Transaction>> {
    unsafe { TX_MANAGER.get_current_tx() }
}

/// 检查是否有活跃事务
pub fn has_active_tx() -> bool {
    unsafe { TX_MANAGER.has_active_tx() }
}

/// 设置事务管理器低功耗模式
pub unsafe fn set_low_power_mode(enabled: bool) {
    TX_MANAGER.set_low_power_mode(enabled);
}

/// 设置日志管理器
pub unsafe fn set_log_manager(log_manager: LogManager) {
    TX_MANAGER.set_log_manager(log_manager);
}
