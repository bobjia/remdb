use core::ptr::NonNull;
use crate::types::{Result, RemDbError};
use crate::platform::{memcpy, memset};
use crate::defer;

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
#[derive(Copy, Clone)]
pub enum LogOperation {
    /// 插入记录
    Insert = 0,
    /// 删除记录
    Delete = 1,
    /// 更新记录
    Update = 2,
    /// 时序数据插入
    TimeSeriesInsert = 3,
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
    pub unsafe fn new(log_path: &'static str, config: &crate::config::DbConfig) -> Result<Self> {
        // 尝试打开日志文件，如果不存在则创建
        let log_handle = crate::platform::file_open(
            log_path,
            crate::platform::FileMode::ReadWrite
        ).map_err(|_| RemDbError::FileIoError)?;
        
        // 获取当前时间
        let now = crate::platform::get_timestamp_us();
        let now_ms = now / 1000;
        
        let mut manager = LogManager {
            log_path,
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
            log_mode: config.log_mode,
            log_buffer: alloc::vec::Vec::new(), // 默认缓冲区大小1024
            buffer_config: LogBufferConfig {
                size: 1024,
                flush_threshold: 800, // 80%使用率时刷新
            },
            last_flush_time: now,
            last_checkpoint_time: now_ms,
            checkpoint_interval_ms: config.checkpoint_interval_ms,
            log_file_size_limit: config.log_file_size_limit,
            log_segment_size: config.log_segment_size,
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
            if config.log_prealloc_size > 0 {
                // 定位到预分配大小位置
                crate::platform::file_seek(
                    log_handle,
                    config.log_prealloc_size as i64 - 1,
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
                
                Ok(())
            }
        }
    }
    
    /// 复制WAL日志到从节点
    unsafe fn replicate_wal(&self, log_item: &LogItem) -> Result<()> {
        // 尝试获取HA管理器
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
        
        let handle = crate::platform::file_open(
            self.log_path,
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
        
        // 验证校验和
        let mut check_bytes = log_bytes.clone();
        let checksum_ptr = check_bytes.as_mut_ptr().add(core::mem::size_of::<LogItem>() - 4) as *mut u32;
        *checksum_ptr = 0;
        
        let calculated_checksum = Transaction::calculate_checksum(&check_bytes);
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
        
        // 释放锁，避免在write_checkpoint中再次借用冲突
        crate::platform::spin_unlock(&mut self.lock);
        
        // 写入检查点
        self.write_checkpoint()?;
        
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
        // 读取所有未处理的日志记录
        for i in self.checkpoint.processed_records..self.header.record_count {
            let log_item = self.read_log_item(i)?;
            
            // 根据日志类型执行相应的恢复操作
            match log_item.op_type {
                LogOperation::Insert => {
                    // 执行插入操作
                    let table = match &mut db.tables[log_item.table_id as usize] {
                        Some(table) => table,
                        None => return Err(RemDbError::TableNotFound),
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
                        table.inc_record_count();
                    }
                },
                LogOperation::Delete => {
                    // 执行删除操作
                    let table = match &mut db.tables[log_item.table_id as usize] {
                        Some(table) => table,
                        None => return Err(RemDbError::TableNotFound),
                    };
                    
                    // 检查记录是否存在
                    let status_ptr = table.get_status_ptr(log_item.record_id as usize);
                    if (*status_ptr).status == crate::types::RecordStatus::Used {
                        // 记录存在，执行删除
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
                LogOperation::Update => {
                    // 执行更新操作
                    let table = match &mut db.tables[log_item.table_id as usize] {
                        Some(table) => table,
                        None => return Err(RemDbError::TableNotFound),
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
                    let partition = ts_table.partitions.get_or_create_partition(record.timestamp);
                    
                    // 写入记录到分区
                    let mut partition_guard = partition.lock().unwrap();
                    partition_guard.records.push(record);
                    partition_guard.stats.record_count = partition_guard.records.len();
                    
                    // 更新索引
                    ts_table.index.insert(record.timestamp, partition_guard.records.len() - 1);
                },
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

/// 事务管理器
pub struct TransactionManager {
    /// 当前事务
    current_tx: Option<NonNull<Transaction>>,
    /// 事务ID计数器
    tx_id_counter: u32,
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
        
        // 初始化事务上下文
        let mut tx_ptr = NonNull::new_unchecked(tx_buffer);
        let tx_mut = tx_ptr.as_mut();
        
        tx_mut.id = self.tx_id_counter;
        tx_mut.tx_type = tx_type;
        tx_mut.status = TransactionStatus::Active;
        tx_mut.isolation_level = isolation_level;
        tx_mut.start_time = crate::platform::get_timestamp_us();
        tx_mut.log_items = NonNull::new_unchecked(log_buffer);
        tx_mut.max_log_items = max_log_items;
        tx_mut.log_item_count = 0;
        tx_mut.depth = 1;
        tx_mut.lock = 0;
        
        // 初始化日志缓冲区
        for i in 0..max_log_items {
            let log_ptr = log_buffer.add(i);
            (*log_ptr).op_type = LogOperation::Insert;
            (*log_ptr).table_id = 0;
            (*log_ptr).record_id = 0;
            (*log_ptr).data_size = 0;
            (*log_ptr).tx_id = 0;
            (*log_ptr).timestamp = 0;
            (*log_ptr).checksum = 0;
            memset((*log_ptr).old_data.as_mut_ptr(), 0, 512);
            memset((*log_ptr).new_data.as_mut_ptr(), 0, 512);
        }
        
        // 更新事务ID计数器
        self.tx_id_counter += 1;
        
        // 设置当前事务
        self.current_tx = Some(tx_ptr);
        
        Ok(tx_ptr)
    }
    
    /// 提交事务
    pub unsafe fn commit(&mut self) -> Result<()> {
        // 增加已提交事务计数
        crate::get_global_db().map(|db| db.metrics.inc_committed_transactions());
        // 自旋锁保护
        crate::platform::spin_lock(&mut self.lock);
        defer! { crate::platform::spin_unlock(&mut self.lock); }
        
        // 检查是否有活跃事务
        let mut tx_ptr = match self.current_tx {
            Some(tx) => tx,
            None => return Err(RemDbError::TransactionError),
        };
        
        let tx_mut = tx_ptr.as_mut();
        
        // 检查事务状态
        if tx_mut.status != TransactionStatus::Active {
            return Err(RemDbError::TransactionError);
        }
        
        // 对于只读事务，直接提交
        if tx_mut.tx_type == TransactionType::ReadOnly {
            tx_mut.status = TransactionStatus::Committed;
            self.current_tx = None;
            return Ok(());
        }
        
        // 对于读写事务，写入所有日志项
        if let Some(log_manager) = &mut self.log_manager {
            if self.low_power_mode {
                // 低功耗模式：优化日志写入
                // 这里简单实现：每10个事务只写入一次日志
                if self.tx_id_counter % 10 == 0 {
                    // 写入所有日志项
                    for i in 0..tx_mut.log_item_count {
                        let log_item = &tx_mut.log_items.as_ptr().add(i).read();
                        log_manager.write_log_item(log_item)?;
                    }
                    // 在低功耗模式下，每10个事务写入日志后，直接创建检查点
                    log_manager.create_checkpoint()?;
                } else {
                    // 跳过日志写入，减少磁盘I/O
                }
            } else {
                // 正常模式：写入所有日志项
                for i in 0..tx_mut.log_item_count {
                    let log_item = &tx_mut.log_items.as_ptr().add(i).read();
                    log_manager.write_log_item(log_item)?;
                }
                
                // 检查日志模式，如果是同步模式，不需要额外操作
                // 如果是异步模式，根据需要刷新缓冲区
                match log_manager.log_mode {
                    crate::config::LogMode::Async => {
                        // 检查是否需要立即刷新（如缓冲区接近满）
                        if log_manager.log_buffer.len() >= log_manager.buffer_config.flush_threshold {
                            log_manager.flush_buffer()?;
                        }
                    },
                    _ => {}
                }
            }
        }
        
        // 更新事务状态
        tx_mut.status = TransactionStatus::Committed;
        
        // 清除当前事务
        self.current_tx = None;
        
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
        let mut tx_ptr = match self.current_tx {
            Some(tx) => tx,
            None => return Err(RemDbError::TransactionError),
        };
        
        let tx_mut = tx_ptr.as_mut();
        
        // 检查事务状态
        if tx_mut.status != TransactionStatus::Active {
            return Err(RemDbError::TransactionError);
        }
        
        // 对于只读事务，直接回滚
        if tx_mut.tx_type == TransactionType::ReadOnly {
            tx_mut.status = TransactionStatus::RolledBack;
            self.current_tx = None;
            return Ok(());
        }
        
        // 对于读写事务，回滚所有日志操作
        // 从后往前回滚，确保正确的回滚顺序
        for i in (0..tx_mut.log_item_count).rev() {
            let log_item = &tx_mut.log_items.as_ptr().add(i).read();
            
            // 根据日志类型执行相应的回滚操作
            match log_item.op_type {
                LogOperation::Insert => {
                    // 回滚Insert操作：直接删除已插入的记录，不添加日志
                    let table_id = log_item.table_id as usize;
                    let table = match &mut db.tables[table_id] {
                        Some(table) => table,
                        None => return Err(RemDbError::TableNotFound),
                    };
                    
                    let record_id = log_item.record_id as usize;
                    
                    // 检查record_id是否有效
                    if record_id >= table.def.max_records {
                        continue;
                    }
                    
                    let status_ptr = table.get_status_ptr(record_id);
                    if (*status_ptr).status == crate::types::RecordStatus::Used {
                        // 标记为空闲
                        (*status_ptr).status = crate::types::RecordStatus::Free;
                        (*status_ptr).version += 1;
                        
                        // 清空记录数据
                        let record_ptr = table.get_record_ptr_mut(record_id);
                        crate::platform::memset(record_ptr, 0, table.record_size);
                        
                        // 将空闲槽压回栈中，确保不超过数组大小
                        if table.free_slot_count < table.def.max_records {
                            *table.free_slots.as_ptr().add(table.free_slot_count) = record_id;
                            table.free_slot_count += 1;
                        }
                        
                        // 更新记录计数
                        if table.record_count > 0 {
                            table.record_count -= 1;
                        }
                    }
                },
                LogOperation::Delete => {
                    // 回滚Delete操作：直接恢复被删除的记录，不添加日志
                    let table_id = log_item.table_id as usize;
                    let table = match &mut db.tables[table_id] {
                        Some(table) => table,
                        None => return Err(RemDbError::TableNotFound),
                    };
                    
                    let record_id = log_item.record_id as usize;
                    
                    // 检查record_id是否有效
                    if record_id >= table.def.max_records {
                        continue;
                    }
                    
                    let status_ptr = table.get_status_ptr(record_id);
                    if (*status_ptr).status != crate::types::RecordStatus::Used {
                        // 记录已被释放，需要重新插入
                        let record_ptr = table.get_record_ptr_mut(record_id);
                        crate::platform::memcpy(
                            record_ptr,
                            log_item.old_data.as_ptr(),
                            log_item.data_size as usize
                        );
                        
                        // 更新状态
                        (*status_ptr).status = crate::types::RecordStatus::Used;
                        (*status_ptr).version += 1;
                        
                        // 更新记录计数，确保不超过最大记录数
                        if table.record_count < table.def.max_records {
                            table.record_count += 1;
                        }
                        
                        // 从空闲槽栈中移除该槽，确保不超过数组大小
                        if table.free_slot_count > 0 {
                            table.free_slot_count -= 1;
                        }
                    }
                },
                LogOperation::Update => {
                    // 回滚Update操作：直接恢复旧数据，不添加日志
                    let table_id = log_item.table_id as usize;
                    let table = match &mut db.tables[table_id] {
                        Some(table) => table,
                        None => return Err(RemDbError::TableNotFound),
                    };
                    
                    let record_id = log_item.record_id as usize;
                    
                    // 检查record_id是否有效
                    if record_id >= table.def.max_records {
                        continue;
                    }
                    
                    let record_ptr = table.get_record_ptr_mut(record_id);
                    let data_size = core::cmp::min(log_item.data_size as usize, table.record_size);
                    crate::platform::memcpy(
                        record_ptr,
                        log_item.old_data.as_ptr(),
                        data_size
                    );
                    
                    // 更新版本号
                    let status_ptr = table.get_status_ptr(record_id);
                    (*status_ptr).version += 1;
                },
                LogOperation::TimeSeriesInsert => {
                    // 回滚时间序列插入操作：直接从对应分区中删除记录
                    let table_id = log_item.table_id as usize;
                    let ts_table = match &mut db.time_series_tables[table_id] {
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
                    
                    // 获取对应分区
                    if let Some(partition) = ts_table.partitions.get_partition(record.timestamp) {
                        let mut partition_guard = partition.lock().unwrap();
                        
                        // 查找并删除记录
                        if let Some(pos) = partition_guard.records.iter().position(|r| r.timestamp == record.timestamp) {
                            partition_guard.records.remove(pos);
                            partition_guard.stats.record_count = partition_guard.records.len();
                        }
                    }
                    
                    // 更新索引
                    ts_table.index.remove(record.timestamp);
                },
            }
        }
        
        // 更新事务状态
        tx_mut.status = TransactionStatus::RolledBack;
        
        // 清除当前事务
        self.current_tx = None;
        
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
    
    /// 重置事务管理器
    pub unsafe fn reset(&mut self) {
        self.current_tx = None;
        self.tx_id_counter = 0;
    }
}

impl Transaction {
    /// 计算数据校验和
    fn calculate_checksum(data: &[u8]) -> u32 {
        // 简单的XOR校验和实现，适合嵌入式环境
        let mut checksum = 0u32;
        for &byte in data {
            checksum ^= byte as u32;
        }
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
        
        // 计算校验和
        let mut checksum_data = [0u8; 1024];
        let mut offset = 0;
        
        // 拷贝操作类型和元数据
        checksum_data[offset] = (*log_ptr).op_type as u8;
        offset += 1;
        checksum_data[offset] = (*log_ptr).table_id;
        offset += 1;
        checksum_data[offset..offset+2].copy_from_slice(&(*log_ptr).record_id.to_le_bytes());
        offset += 2;
        checksum_data[offset..offset+2].copy_from_slice(&(*log_ptr).data_size.to_le_bytes());
        offset += 2;
        
        // 拷贝数据
        checksum_data[offset..offset+data_size].copy_from_slice(&(&(*log_ptr).old_data)[0..data_size]);
        offset += data_size;
        checksum_data[offset..offset+data_size].copy_from_slice(&(&(*log_ptr).new_data)[0..data_size]);
        offset += data_size;
        
        // 计算校验和
        (*log_ptr).checksum = Self::calculate_checksum(&checksum_data[0..offset]);
        
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
