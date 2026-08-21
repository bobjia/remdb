use crate::try_lock;

use crate::defer;
use crate::types::{RemDbError, Result, DEFAULT_TEXT_SIZE, DEFAULT_JSON_SIZE};
use core::default::Default;
use core::ptr::NonNull;
use crate::DdlExecutor;

// 引入alloc模块
extern crate alloc;
use alloc::vec::Vec;

#[cfg(feature = "log")]
use crate::log::{error, info, warn};

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
#[derive(Copy, Clone, Debug, PartialEq)]
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
    /// 进入低功耗模式
    EnterLowPowerMode = 9,
    /// 退出低功耗模式
    ExitLowPowerMode = 10,
    /// 修改表结构
    AlterTable = 11,
    /// 删除表
    DropTable = 12,
    /// 创建数据库
    CreateDatabase = 13,
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
    /// 压缩类型 (0=None, 1=LZ4, 2=ZSTD)
    pub compression_type: u8,
}

/// 事务日志数据块大小（已废弃，保留用于向后兼容）
pub const LOG_DATA_BULK_SIZE: usize = 512;

/// 事务日志项头部（仅包含元数据，不包含数据）
#[repr(C)]
#[derive(Copy, Clone)]
pub struct LogItem {
    /// 操作类型
    pub op_type: LogOperation,
    /// 表ID
    pub table_id: u8,
    /// 记录ID
    pub record_id: u16,
    /// 旧数据大小
    pub old_data_size: u16,
    /// 新数据大小
    pub new_data_size: u16,
    /// 事务ID
    pub tx_id: u32,
    /// 时间戳（微秒）
    pub timestamp: u64,
    /// 校验和
    pub checksum: u32,
}

/// 可变大小日志项（唯一支持的WAL格式）
#[derive(Clone)]
pub struct VariableSizeLogItem {
    /// 固定头部
    pub header: LogItem,
    /// 旧数据（用于回滚）
    pub old_data: Vec<u8>,
    /// 新数据
    pub new_data: Vec<u8>,
}

/// 为LogItem实现Default trait
impl Default for LogItem {
    fn default() -> Self {
        Self {
            op_type: LogOperation::Insert,
            table_id: 0,
            record_id: 0,
            old_data_size: 0,
            new_data_size: 0,
            tx_id: 0,
            timestamp: 0,
            checksum: 0,
        }
    }
}

impl Default for VariableSizeLogItem {
    fn default() -> Self {
        Self {
            header: LogItem::default(),
            old_data: Vec::new(),
            new_data: Vec::new(),
        }
    }
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
    /// 日志项数组（使用 VariableSizeLogItem 以支持数据存储）
    log_items: NonNull<VariableSizeLogItem>,
    /// 最大日志项数量
    max_log_items: usize,
    /// 当前日志项数量
    log_item_count: usize,
    /// 嵌套深度（不支持嵌套事务，固定为1）
    pub depth: u8,
    /// 自旋锁
    lock: u32,
}

/// 为Transaction实现Default trait
impl Default for Transaction {
    fn default() -> Self {
        unsafe {
            Transaction {
                id: 0,
                tx_type: TransactionType::ReadWrite,
                status: TransactionStatus::Active,
                isolation_level: IsolationLevel::RepeatableRead,
                start_time: 0,
                log_items: NonNull::dangling(),
                max_log_items: 0,
                log_item_count: 0,
                depth: 1,
                lock: 0,
            }
        }
    }
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
    /// 日志缓冲区（使用 VariableSizeLogItem 以支持数据存储）
    log_buffer: alloc::vec::Vec<VariableSizeLogItem>,
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
    /// 当前日志项的偏移量（用于可变大小日志项）
    current_log_offset: usize,
    /// 恢复时最大连续无效记录数
    max_consecutive_invalid: u32,
    /// 恢复时跳过预分配空间的阈值
    skip_threshold: u32,
    /// 恢复时跳过的块大小
    skip_block_size: usize,
    /// 恢复时最大跳过尝试次数
    max_skip_attempts: u32,
    /// WAL压缩类型
    compression_type: crate::config::WALCompressionType,
    /// 压缩级别
    compression_level: u8,
}

impl Transaction {
    /// 计算可变大小日志项的完整校验和（包括头部和数据部分）
    pub unsafe fn calculate_variable_size_log_item_checksum(var_log_item: &VariableSizeLogItem) -> u32 {
        let mut checksum = 0u32;

        checksum ^= (var_log_item.header.op_type as u32).to_le();
        checksum ^= (var_log_item.header.table_id as u32).to_le();
        checksum ^= (var_log_item.header.record_id as u32).to_le();
        checksum ^= (var_log_item.header.old_data_size as u32).to_le();
        checksum ^= (var_log_item.header.new_data_size as u32).to_le();
        checksum ^= (var_log_item.header.tx_id as u32).to_le();
        checksum ^= (var_log_item.header.timestamp as u32).to_le();

        for i in 0..var_log_item.old_data.len() {
            checksum ^= (var_log_item.old_data[i] as u32).to_le();
        }

        for i in 0..var_log_item.new_data.len() {
            checksum ^= (var_log_item.new_data[i] as u32).to_le();
        }

        checksum
    }

    /// 计算LogItem头部的校验和（不包含数据部分）
    pub unsafe fn calculate_log_item_checksum(log_item: &LogItem) -> u32 {
        let mut checksum = 0u32;

        checksum ^= (log_item.op_type as u32).to_le();
        checksum ^= (log_item.table_id as u32).to_le();
        checksum ^= (log_item.record_id as u32).to_le();
        checksum ^= (log_item.old_data_size as u32).to_le();
        checksum ^= (log_item.new_data_size as u32).to_le();
        checksum ^= (log_item.tx_id as u32).to_le();
        checksum ^= (log_item.timestamp as u32).to_le();

        checksum
    }
}

impl LogManager {
    /// 创建新的日志管理器
    pub unsafe fn new(config: &crate::config::DbConfig) -> Result<Self> {
        // 构造完整的日志文件路径：log_path目录 + remdb.wal文件名
        let log_dir = &config.wal_config.log_path;

        // 在no_std环境下使用alloc::format宏
        use alloc::format;
        let wal_file_path = format!("{}/remdb.wal", log_dir);

        // 确保日志目录存在（仅在std环境下）
        #[cfg(feature = "std")]
        {
            use std::fs;
            use std::path::Path;

            let log_path = Path::new(&log_dir);
            if !log_path.exists() {
                fs::create_dir_all(log_path).unwrap_or(());
            }
        }

        // 尝试打开日志文件，如果不存在则创建
        let log_handle = crate::platform::file_open(
            wal_file_path.as_str(),
            crate::platform::FileMode::ReadWrite,
        )
        .map_err(|_| RemDbError::FileIoError)?;

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
                compression_type: match config.wal_config.compression_type {
                    crate::config::WALCompressionType::None => 0,
                    crate::config::WALCompressionType::LZ4 => 1,
                    crate::config::WALCompressionType::ZSTD => 2,
                },
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
            current_log_offset: core::mem::size_of::<LogHeader>() + core::mem::size_of::<LogCheckpoint>(),
            max_consecutive_invalid: config.wal_config.max_consecutive_invalid,
            skip_threshold: config.wal_config.skip_threshold,
            skip_block_size: config.wal_config.skip_block_size,
            max_skip_attempts: config.wal_config.max_skip_attempts,
            compression_type: config.wal_config.compression_type,
            compression_level: config.wal_config.compression_level,
        };

        // 预分配缓冲区空间
        manager.log_buffer.reserve(1024);

        // 读取日志头，如果文件为空或格式不正确则写入新的日志头
        let mut header_buffer = [0u8; core::mem::size_of::<LogHeader>()];
        let read =
            crate::platform::file_read(log_handle, header_buffer.as_mut_ptr(), header_buffer.len())
                .map_err(|_| RemDbError::FileIoError)?;

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
            crate::platform::file_seek(log_handle, 0, crate::platform::SeekWhence::SeekSet)
                .map_err(|_| RemDbError::FileIoError)?;

            // 如果配置了预分配大小，则预分配文件空间
            if config.wal_config.log_prealloc_size > 0 {
                // 定位到预分配大小位置
                crate::platform::file_seek(
                    log_handle,
                    config.wal_config.log_prealloc_size as i64 - 1,
                    crate::platform::SeekWhence::SeekSet,
                )
                .map_err(|_| RemDbError::FileIoError)?;

                // 写入一个字节来扩展文件
                let zero_byte = [0u8; 1];
                crate::platform::file_write(log_handle, zero_byte.as_ptr(), 1)
                    .map_err(|_| RemDbError::FileIoError)?;

                // 回到文件开头
                crate::platform::file_seek(log_handle, 0, crate::platform::SeekWhence::SeekSet)
                    .map_err(|_| RemDbError::FileIoError)?;
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
        core::ptr::write_unaligned(header_bytes.as_mut_ptr() as *mut LogHeader, self.header);

        // 清除旧的校验和 (checksum offset = 4 + 4 + 8 + 4 = 20)
        let checksum_ptr = header_bytes.as_mut_ptr().add(20) as *mut u32;
        *checksum_ptr = 0;

        // 计算新的校验和
        self.header.checksum = Transaction::calculate_checksum(&header_bytes);

        // 重新写入完整的日志头
        core::ptr::write_unaligned(header_bytes.as_mut_ptr() as *mut LogHeader, self.header);

        // 定位到文件开头
        crate::platform::file_seek(self.log_handle, 0, crate::platform::SeekWhence::SeekSet)
            .map_err(|_| RemDbError::FileIoError)?;

        // 写入日志头
        let written =
            crate::platform::file_write(self.log_handle, header_bytes.as_ptr(), header_bytes.len())
                .map_err(|_| RemDbError::FileIoError)?;

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
            crate::platform::SeekWhence::SeekSet,
        )
        .map_err(|_| RemDbError::FileIoError)?;

        // 读取检查点
        let mut checkpoint_buffer = [0u8; core::mem::size_of::<LogCheckpoint>()];
        let read = crate::platform::file_read(
            self.log_handle,
            checkpoint_buffer.as_mut_ptr(),
            checkpoint_buffer.len(),
        )
        .map_err(|_| RemDbError::FileIoError)?;

        if read == 0 {
            // 检查点不存在，使用默认值
            self.checkpoint = LogCheckpoint {
                timestamp: 0,
                processed_records: 0,
                checksum: 0,
            };
        } else {
            // 读取检查点
            self.checkpoint =
                core::ptr::read_unaligned(checkpoint_buffer.as_ptr() as *const LogCheckpoint);
        }

        Ok(())
    }

    /// 写入检查点
    pub unsafe fn write_checkpoint(&mut self) -> Result<()> {
        // 计算检查点校验和
        let mut checkpoint_bytes = [0u8; core::mem::size_of::<LogCheckpoint>()];
        core::ptr::write_unaligned(
            checkpoint_bytes.as_mut_ptr() as *mut LogCheckpoint,
            self.checkpoint,
        );

        // 清除旧的校验和
        let checksum_ptr = checkpoint_bytes.as_mut_ptr().add(12) as *mut u32;
        *checksum_ptr = 0;

        // 计算新的校验和
        self.checkpoint.checksum = Transaction::calculate_checksum(&checkpoint_bytes);

        // 重新写入完整的检查点
        core::ptr::write_unaligned(
            checkpoint_bytes.as_mut_ptr() as *mut LogCheckpoint,
            self.checkpoint,
        );

        // 定位到检查点位置
        let checkpoint_offset = core::mem::size_of::<LogHeader>();
        crate::platform::file_seek(
            self.log_handle,
            checkpoint_offset as i64,
            crate::platform::SeekWhence::SeekSet,
        )
        .map_err(|_| RemDbError::FileIoError)?;

        // 写入检查点
        let written = crate::platform::file_write(
            self.log_handle,
            checkpoint_bytes.as_ptr(),
            checkpoint_bytes.len(),
        )
        .map_err(|_| RemDbError::FileIoError)?;

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
        let log_offset = core::mem::size_of::<LogHeader>()
            + core::mem::size_of::<LogCheckpoint>()
            + (self.header.record_count as usize) * core::mem::size_of::<LogItem>();

        crate::platform::file_seek(
            self.log_handle,
            log_offset as i64,
            crate::platform::SeekWhence::SeekSet,
        )
        .map_err(|_| RemDbError::FileIoError)?;

        // 批量写入日志项
        let buffer_size = self.log_buffer.len();
        for i in 0..buffer_size {
            let var_log_item = &self.log_buffer[i];
            
            // 计算可变大小日志项的总大小
            let header_size = core::mem::size_of::<LogItem>();
            let total_size = header_size + var_log_item.old_data.len() + var_log_item.new_data.len();
            
            // 分配缓冲区
            let mut log_bytes = vec![0u8; total_size];
            
            // 写入头部
            let header_ptr = log_bytes.as_mut_ptr() as *mut LogItem;
            core::ptr::write_unaligned(header_ptr, var_log_item.header);
            
            // 写入旧数据
            let old_data_start = header_size;
            if !var_log_item.old_data.is_empty() {
                log_bytes[old_data_start..old_data_start + var_log_item.old_data.len()]
                    .copy_from_slice(&var_log_item.old_data);
            }
            
            // 写入新数据
            let new_data_start = old_data_start + var_log_item.old_data.len();
            if !var_log_item.new_data.is_empty() {
                log_bytes[new_data_start..new_data_start + var_log_item.new_data.len()]
                    .copy_from_slice(&var_log_item.new_data);
            }

            let written =
                crate::platform::file_write(self.log_handle, log_bytes.as_ptr(), log_bytes.len())
                    .map_err(|_| RemDbError::FileIoError)?;

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

    /// 写入日志项（可变大小版本）
    pub unsafe fn write_variable_size_log_item(&mut self, var_log_item: &VariableSizeLogItem) -> Result<()> {
        // 检查是否需要刷新缓冲区或创建检查点
        self.check_flush_and_checkpoint()?;

        match self.log_mode {
            crate::config::LogMode::Sync => {
                // 自旋锁保护
                crate::platform::spin_lock(&mut self.lock);

                // 定位到日志记录区域的末尾（使用当前偏移量）
                crate::platform::file_seek(
                    self.log_handle,
                    self.current_log_offset as i64,
                    crate::platform::SeekWhence::SeekSet,
                )
                .map_err(|_| RemDbError::FileIoError)?;

                // 写入日志项头部
                let mut log_bytes = [0u8; core::mem::size_of::<LogItem>()];
                core::ptr::write_unaligned(log_bytes.as_mut_ptr() as *mut LogItem, var_log_item.header);

                let written = crate::platform::file_write(
                    self.log_handle,
                    log_bytes.as_ptr(),
                    log_bytes.len(),
                )
                .map_err(|_| RemDbError::FileIoError)?;

                if written != log_bytes.len() {
                    crate::platform::spin_unlock(&mut self.lock);
                    return Err(RemDbError::FileIoError);
                }

                // 写入旧数据（压缩）
                if var_log_item.header.old_data_size > 0 {
                    let compressed_old_data = crate::wal_compression::compress_wal_data(
                        &var_log_item.old_data,
                        self.compression_type,
                        self.compression_level,
                    )?;
                    
                    // 写入压缩后的大小
                    let compressed_size = compressed_old_data.len() as u32;
                    let size_bytes = compressed_size.to_le_bytes();
                    let size_written = crate::platform::file_write(
                        self.log_handle,
                        size_bytes.as_ptr(),
                        size_bytes.len(),
                    )
                    .map_err(|_| RemDbError::FileIoError)?;

                    if size_written != size_bytes.len() {
                        crate::platform::spin_unlock(&mut self.lock);
                        return Err(RemDbError::FileIoError);
                    }

                    // 写入压缩后的数据
                    let old_data_written = crate::platform::file_write(
                        self.log_handle,
                        compressed_old_data.as_ptr(),
                        compressed_old_data.len(),
                    )
                    .map_err(|_| RemDbError::FileIoError)?;

                    if old_data_written != compressed_old_data.len() {
                        crate::platform::spin_unlock(&mut self.lock);
                        return Err(RemDbError::FileIoError);
                    }
                }

                // 写入新数据（压缩）
                if var_log_item.header.new_data_size > 0 {
                    let compressed_new_data = crate::wal_compression::compress_wal_data(
                        &var_log_item.new_data,
                        self.compression_type,
                        self.compression_level,
                    )?;
                    
                    // 写入压缩后的大小
                    let compressed_size = compressed_new_data.len() as u32;
                    let size_bytes = compressed_size.to_le_bytes();
                    let size_written = crate::platform::file_write(
                        self.log_handle,
                        size_bytes.as_ptr(),
                        size_bytes.len(),
                    )
                    .map_err(|_| RemDbError::FileIoError)?;

                    if size_written != size_bytes.len() {
                        crate::platform::spin_unlock(&mut self.lock);
                        return Err(RemDbError::FileIoError);
                    }

                    // 写入压缩后的数据
                    let new_data_written = crate::platform::file_write(
                        self.log_handle,
                        compressed_new_data.as_ptr(),
                        compressed_new_data.len(),
                    )
                    .map_err(|_| RemDbError::FileIoError)?;

                    if new_data_written != compressed_new_data.len() {
                        crate::platform::spin_unlock(&mut self.lock);
                        return Err(RemDbError::FileIoError);
                    }
                }

                // 更新日志头
                self.header.record_count += 1;
                self.header.checksum = 0;

                // 更新当前日志偏移量
                let mut data_size = core::mem::size_of::<LogItem>();
                if var_log_item.header.old_data_size > 0 {
                    let compressed_old_data = crate::wal_compression::compress_wal_data(
                        &var_log_item.old_data,
                        self.compression_type,
                        self.compression_level,
                    )?;
                    data_size += 4 + compressed_old_data.len();
                }
                if var_log_item.header.new_data_size > 0 {
                    let compressed_new_data = crate::wal_compression::compress_wal_data(
                        &var_log_item.new_data,
                        self.compression_type,
                        self.compression_level,
                    )?;
                    data_size += 4 + compressed_new_data.len();
                }
                self.current_log_offset += data_size;

                // 释放锁，避免在write_header中再次借用冲突
                crate::platform::spin_unlock(&mut self.lock);

                self.write_header()?;

                // 触发WAL复制
                self.replicate_variable_size_wal(var_log_item)?;

                // Publish to pubsub
                #[cfg(feature = "pubsub")]
                self.publish_variable_size_to_pubsub(var_log_item)?;

                Ok(())
            }
            crate::config::LogMode::Async => {
                // 异步模式：暂时不支持可变大小日志项的缓冲
                // 直接写入磁盘（使用同步写入）
                // 自旋锁保护
                crate::platform::spin_lock(&mut self.lock);

                // 定位到日志记录区域的末尾（使用当前偏移量）
                crate::platform::file_seek(
                    self.log_handle,
                    self.current_log_offset as i64,
                    crate::platform::SeekWhence::SeekSet,
                )
                .map_err(|_| RemDbError::FileIoError)?;

                // 写入日志项头部
                let mut log_bytes = [0u8; core::mem::size_of::<LogItem>()];
                core::ptr::write_unaligned(log_bytes.as_mut_ptr() as *mut LogItem, var_log_item.header);

                let written = crate::platform::file_write(
                    self.log_handle,
                    log_bytes.as_ptr(),
                    log_bytes.len(),
                )
                .map_err(|_| RemDbError::FileIoError)?;

                if written != log_bytes.len() {
                    crate::platform::spin_unlock(&mut self.lock);
                    return Err(RemDbError::FileIoError);
                }

                // 写入旧数据（压缩）
                if var_log_item.header.old_data_size > 0 {
                    let compressed_old_data = crate::wal_compression::compress_wal_data(
                        &var_log_item.old_data,
                        self.compression_type,
                        self.compression_level,
                    )?;
                    
                    // 写入压缩后的大小
                    let compressed_size = compressed_old_data.len() as u32;
                    let size_bytes = compressed_size.to_le_bytes();
                    let size_written = crate::platform::file_write(
                        self.log_handle,
                        size_bytes.as_ptr(),
                        size_bytes.len(),
                    )
                    .map_err(|_| RemDbError::FileIoError)?;

                    if size_written != size_bytes.len() {
                        crate::platform::spin_unlock(&mut self.lock);
                        return Err(RemDbError::FileIoError);
                    }

                    // 写入压缩后的数据
                    let old_data_written = crate::platform::file_write(
                        self.log_handle,
                        compressed_old_data.as_ptr(),
                        compressed_old_data.len(),
                    )
                    .map_err(|_| RemDbError::FileIoError)?;

                    if old_data_written != compressed_old_data.len() {
                        crate::platform::spin_unlock(&mut self.lock);
                        return Err(RemDbError::FileIoError);
                    }
                }

                // 写入新数据（压缩）
                if var_log_item.header.new_data_size > 0 {
                    let compressed_new_data = crate::wal_compression::compress_wal_data(
                        &var_log_item.new_data,
                        self.compression_type,
                        self.compression_level,
                    )?;
                    
                    // 写入压缩后的大小
                    let compressed_size = compressed_new_data.len() as u32;
                    let size_bytes = compressed_size.to_le_bytes();
                    let size_written = crate::platform::file_write(
                        self.log_handle,
                        size_bytes.as_ptr(),
                        size_bytes.len(),
                    )
                    .map_err(|_| RemDbError::FileIoError)?;

                    if size_written != size_bytes.len() {
                        crate::platform::spin_unlock(&mut self.lock);
                        return Err(RemDbError::FileIoError);
                    }

                    // 写入压缩后的数据
                    let new_data_written = crate::platform::file_write(
                        self.log_handle,
                        compressed_new_data.as_ptr(),
                        compressed_new_data.len(),
                    )
                    .map_err(|_| RemDbError::FileIoError)?;

                    if new_data_written != compressed_new_data.len() {
                        crate::platform::spin_unlock(&mut self.lock);
                        return Err(RemDbError::FileIoError);
                    }
                }

                // 更新日志头
                self.header.record_count += 1;
                self.header.checksum = 0;

                // 更新当前日志偏移量
                let mut data_size = core::mem::size_of::<LogItem>();
                if var_log_item.header.old_data_size > 0 {
                    let compressed_old_data = crate::wal_compression::compress_wal_data(
                        &var_log_item.old_data,
                        self.compression_type,
                        self.compression_level,
                    )?;
                    data_size += 4 + compressed_old_data.len();
                }
                if var_log_item.header.new_data_size > 0 {
                    let compressed_new_data = crate::wal_compression::compress_wal_data(
                        &var_log_item.new_data,
                        self.compression_type,
                        self.compression_level,
                    )?;
                    data_size += 4 + compressed_new_data.len();
                }
                self.current_log_offset += data_size;

                // 释放锁，避免在write_header中再次借用冲突
                crate::platform::spin_unlock(&mut self.lock);

                self.write_header()?;

                // 触发WAL复制
                self.replicate_variable_size_wal(var_log_item)?;

                // Publish to pubsub
                #[cfg(feature = "pubsub")]
                self.publish_variable_size_to_pubsub(var_log_item)?;

                Ok(())
            }
        }
    }

    /// 写入日志项（仅支持可变大小格式）
    pub unsafe fn write_log_item(&mut self, log_item: &LogItem) -> Result<()> {
        let var_log_item = VariableSizeLogItem {
            header: *log_item,
            old_data: Vec::new(),
            new_data: Vec::new(),
        };
        self.write_variable_size_log_item(&var_log_item)
    }

    /// 发布WAL日志到pubsub
    #[cfg(feature = "pubsub")]
    unsafe fn publish_to_pubsub(&self, log_item: &LogItem) -> Result<()> {
        use crate::pubsub::topics::*;

        // Serialize log_item to bytes
        let mut log_bytes = [0u8; core::mem::size_of::<LogItem>()];
        core::ptr::write_unaligned(log_bytes.as_mut_ptr() as *mut LogItem, *log_item);

        // Only publish to WAL_TOPIC
        if let Some(topic_id) = crate::pubsub::get_topic_id(WAL_TOPIC) {
            let _ = crate::pubsub::publish(topic_id, &log_bytes);
        }

        Ok(())
    }

    /// 发布可变大小WAL日志到pubsub
    #[cfg(feature = "pubsub")]
    unsafe fn publish_variable_size_to_pubsub(&self, var_log_item: &VariableSizeLogItem) -> Result<()> {
        use crate::pubsub::topics::*;

        // 计算总大小
        let header_size = core::mem::size_of::<LogItem>();
        let total_size = header_size + var_log_item.old_data.len() + var_log_item.new_data.len();

        // 分配缓冲区
        let mut log_bytes = alloc::vec::Vec::with_capacity(total_size);

        // 写入头部
        let header_bytes = core::slice::from_raw_parts(
            &var_log_item.header as *const LogItem as *const u8,
            header_size,
        );
        log_bytes.extend_from_slice(header_bytes);

        // 写入旧数据
        log_bytes.extend_from_slice(&var_log_item.old_data);

        // 写入新数据
        log_bytes.extend_from_slice(&var_log_item.new_data);

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

    /// 复制可变大小WAL日志到从节点
    unsafe fn replicate_variable_size_wal(&self, _var_log_item: &VariableSizeLogItem) -> Result<()> {
        // 尝试获取HA管理器
        #[cfg(feature = "ha")]
        {
            if let Some(_ha_manager) = crate::ha::get_ha_manager() {
                // 将可变大小日志项转换为固定大小日志项进行复制
                // 这里需要HA管理器支持可变大小日志项
                // 暂时跳过复制，避免兼容性问题
                Ok(())
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

    /// 读取可变大小日志项
    pub unsafe fn read_variable_size_log_item(&self, index: u32) -> Result<VariableSizeLogItem> {
        // 检查索引有效性
        if index >= self.header.record_count {
            return Err(RemDbError::LogRecordNotFound);
        }

        // 构造完整的日志文件路径
        use alloc::format;
        let wal_file_path = format!("{}/remdb.wal", self.log_path);

        let handle =
            crate::platform::file_open(wal_file_path.as_str(), crate::platform::FileMode::Read)
                .map_err(|_| RemDbError::FileIoError)?;

        defer! {
            let _ = crate::platform::file_close(handle);
        };

        // 计算可变大小日志项的偏移量
        // 需要遍历前面的所有日志项来计算正确的偏移量
        let mut log_offset = core::mem::size_of::<LogHeader>()
            + core::mem::size_of::<LogCheckpoint>();

        // 遍历前面的所有日志项来计算偏移量
        for _i in 0..index {
            // 读取当前日志项的头部
            let current_offset = log_offset;
            crate::platform::file_seek(
                handle,
                current_offset as i64,
                crate::platform::SeekWhence::SeekSet,
            )
            .map_err(|_| RemDbError::FileIoError)?;

            // 读取头部
            let mut log_bytes = [0u8; core::mem::size_of::<LogItem>()];
            let read = crate::platform::file_read(handle, log_bytes.as_mut_ptr(), log_bytes.len())
                .map_err(|_| RemDbError::FileIoError)?;

            if read != log_bytes.len() {
                return Err(RemDbError::FileIoError);
            }

            let header = core::ptr::read_unaligned(log_bytes.as_ptr() as *const LogItem);

            // 更新偏移量：头部大小
            log_offset += core::mem::size_of::<LogItem>();

            // 计算旧数据大小（包括大小前缀）
            if header.old_data_size > 0 {
                // 读取压缩大小
                let mut compressed_size_bytes = [0u8; 4];
                let size_read = crate::platform::file_read(
                    handle,
                    compressed_size_bytes.as_mut_ptr(),
                    4,
                )
                .map_err(|_| RemDbError::FileIoError)?;

                if size_read != 4 {
                    return Err(RemDbError::FileIoError);
                }

                let compressed_size = u32::from_le_bytes(compressed_size_bytes) as usize;
                log_offset += 4 + compressed_size;
            }

            // 计算新数据大小（包括大小前缀）
            if header.new_data_size > 0 {
                // 读取压缩大小
                let mut compressed_size_bytes = [0u8; 4];
                let size_read = crate::platform::file_read(
                    handle,
                    compressed_size_bytes.as_mut_ptr(),
                    4,
                )
                .map_err(|_| RemDbError::FileIoError)?;

                if size_read != 4 {
                    return Err(RemDbError::FileIoError);
                }

                let compressed_size = u32::from_le_bytes(compressed_size_bytes) as usize;
                log_offset += 4 + compressed_size;
            }
        }

        // 定位到目标日志项头部位置
        crate::platform::file_seek(
            handle,
            log_offset as i64,
            crate::platform::SeekWhence::SeekSet,
        )
        .map_err(|_| RemDbError::FileIoError)?;

        // 读取日志项头部
        let mut log_bytes = [0u8; core::mem::size_of::<LogItem>()];
        let read = crate::platform::file_read(handle, log_bytes.as_mut_ptr(), log_bytes.len())
            .map_err(|_| RemDbError::FileIoError)?;

        if read != log_bytes.len() {
            return Err(RemDbError::FileIoError);
        }

        let header = core::ptr::read_unaligned(log_bytes.as_ptr() as *const LogItem);

        // 更新偏移量：指向旧数据的起始位置
        log_offset += core::mem::size_of::<LogItem>();

        // 创建可变大小日志项
        let mut var_log_item = VariableSizeLogItem {
            header,
            old_data: Vec::new(),
            new_data: Vec::new(),
        };

        // 读取旧数据
        if header.old_data_size > 0 {
            crate::platform::file_seek(
                handle,
                log_offset as i64,
                crate::platform::SeekWhence::SeekSet,
            )
            .map_err(|_| RemDbError::FileIoError)?;

            // 读取压缩大小
            let mut compressed_size_bytes = [0u8; 4];
            let size_read = crate::platform::file_read(
                handle,
                compressed_size_bytes.as_mut_ptr(),
                4,
            )
            .map_err(|_| RemDbError::FileIoError)?;

            if size_read != 4 {
                return Err(RemDbError::FileIoError);
            }

            let compressed_size = u32::from_le_bytes(compressed_size_bytes) as usize;
            var_log_item.old_data.resize(compressed_size, 0);
            let old_data_read = crate::platform::file_read(
                handle,
                var_log_item.old_data.as_mut_ptr(),
                compressed_size,
            )
            .map_err(|_| RemDbError::FileIoError)?;

            if old_data_read != compressed_size {
                return Err(RemDbError::FileIoError);
            }

            // 解压缩数据
            var_log_item.old_data = crate::wal_compression::decompress_wal_data(&var_log_item.old_data, self.compression_type)?;

            // 更新偏移量：指向新数据的起始位置
            log_offset += 4 + compressed_size;
        }

        // 读取新数据
        if header.new_data_size > 0 {
            crate::platform::file_seek(
                handle,
                log_offset as i64,
                crate::platform::SeekWhence::SeekSet,
            )
            .map_err(|_| RemDbError::FileIoError)?;

            // 读取压缩大小
            let mut compressed_size_bytes = [0u8; 4];
            let size_read = crate::platform::file_read(
                handle,
                compressed_size_bytes.as_mut_ptr(),
                4,
            )
            .map_err(|_| RemDbError::FileIoError)?;

            if size_read != 4 {
                return Err(RemDbError::FileIoError);
            }

            let compressed_size = u32::from_le_bytes(compressed_size_bytes) as usize;
            var_log_item.new_data.resize(compressed_size, 0);
            let new_data_read = crate::platform::file_read(
                handle,
                var_log_item.new_data.as_mut_ptr(),
                compressed_size,
            )
            .map_err(|_| RemDbError::FileIoError)?;

            if new_data_read != compressed_size {
                return Err(RemDbError::FileIoError);
            }

            // 解压缩数据
            var_log_item.new_data = crate::wal_compression::decompress_wal_data(&var_log_item.new_data, self.compression_type)?;
        }

        // 验证校验和
        let calculated_checksum = Transaction::calculate_variable_size_log_item_checksum(&var_log_item);

        if header.checksum != calculated_checksum {
            return Err(RemDbError::LogChecksumError);
        }

        Ok(var_log_item)
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

        // 创建检查点日志项头部
        let checkpoint_header = LogItem {
            op_type: LogOperation::Checkpoint,
            table_id: 0,  // 检查点操作不关联特定表
            record_id: 0, // 检查点操作不关联特定记录
            old_data_size: 0, // 检查点操作没有数据
            new_data_size: 0, // 检查点操作没有数据
            tx_id: 0, // 检查点操作不关联特定事务
            timestamp: now,
            checksum: 0, // 后面会计算
        };

        // 计算校验和：直接基于字段计算，避免结构体填充问题
        let calculated_checksum = Transaction::calculate_log_item_checksum(&checkpoint_header);

        let mut final_checkpoint_header = checkpoint_header;
        final_checkpoint_header.checksum = calculated_checksum;

        // 写入检查点日志头部
        let log_offset = core::mem::size_of::<LogHeader>()
            + core::mem::size_of::<LogCheckpoint>()
            + (self.header.record_count as usize) * core::mem::size_of::<LogItem>();

        crate::platform::file_seek(
            self.log_handle,
            log_offset as i64,
            crate::platform::SeekWhence::SeekSet,
        )
        .map_err(|_| RemDbError::FileIoError)?;

        // 写入日志项头部
        let mut log_bytes = [0u8; core::mem::size_of::<LogItem>()];
        core::ptr::write_unaligned(log_bytes.as_mut_ptr() as *mut LogItem, final_checkpoint_header);

        let written =
            crate::platform::file_write(self.log_handle, log_bytes.as_ptr(), log_bytes.len())
                .map_err(|_| RemDbError::FileIoError)?;

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
        use alloc::format;
        let wal_file_path = format!("{}/remdb.wal", self.log_path);

        let file_size = crate::platform::file_size(wal_file_path.as_str())
            .map_err(|_| RemDbError::FileIoError)?;

        let header_size = core::mem::size_of::<LogHeader>() + core::mem::size_of::<LogCheckpoint>();
        let log_region_size = if file_size > header_size {
            file_size - header_size
        } else {
            0
        };

        #[cfg(feature = "log")]
        info!(
            "WAL recovery started: file size = {}, log region size = {}",
            file_size, log_region_size
        );

        let mut valid_log_items = alloc::vec::Vec::new();

        let handle = match crate::platform::file_open(
            wal_file_path.as_str(),
            crate::platform::FileMode::Read,
        ) {
            Ok(handle) => handle,
            Err(_) => {
                #[cfg(feature = "log")]
                warn!("Failed to open log file for recovery, skipping recovery process");
                return Ok(());
            }
        };

        let mut current_offset = header_size;
        let mut record_index = 0u32;
        let mut consecutive_invalid_records = 0u32;
        let max_consecutive_invalid = self.max_consecutive_invalid;
        let skip_block_size = self.skip_block_size;
        let skip_threshold = self.skip_threshold;
        let max_skip_attempts = self.max_skip_attempts;

        while current_offset < file_size {
            if let Err(_) = crate::platform::file_seek(
                handle,
                current_offset as i64,
                crate::platform::SeekWhence::SeekSet,
            ) {
                #[cfg(feature = "log")]
                warn!("Failed to seek to offset {}, stopping recovery", current_offset);
                break;
            }

            const HEADER_SIZE_BYTES: usize = core::mem::size_of::<LogItem>();
            let mut log_bytes = [0u8; HEADER_SIZE_BYTES];
            let read = match crate::platform::file_read(handle, log_bytes.as_mut_ptr(), log_bytes.len()) {
                Ok(read) => read,
                Err(_) => {
                    #[cfg(feature = "log")]
                    warn!("Failed to read log item header at offset {}, stopping recovery", current_offset);
                    break;
                }
            };

            if read != log_bytes.len() {
                #[cfg(feature = "log")]
                warn!("Incomplete log item header at offset {}, stopping recovery", current_offset);
                break;
            }

            let header = core::ptr::read_unaligned(log_bytes.as_ptr() as *const LogItem);

            let mut var_log_item = VariableSizeLogItem {
                header,
                old_data: Vec::new(),
                new_data: Vec::new(),
            };

            let mut data_offset = current_offset + HEADER_SIZE_BYTES;
            let mut read_success = true;

            // Determine compression type from log header
            let compression_type = match self.header.compression_type {
                0 => crate::config::WALCompressionType::None,
                1 => crate::config::WALCompressionType::LZ4,
                2 => crate::config::WALCompressionType::ZSTD,
                _ => crate::config::WALCompressionType::None,
            };

            if header.old_data_size > 0 {
                if let Err(_) = crate::platform::file_seek(
                    handle,
                    data_offset as i64,
                    crate::platform::SeekWhence::SeekSet,
                ) {
                    read_success = false;
                } else {
                    // Read compressed size (4 bytes) if compression is enabled
                    if matches!(compression_type, crate::config::WALCompressionType::None) {
                        var_log_item.old_data.resize(header.old_data_size as usize, 0);
                        let old_data_read = crate::platform::file_read(
                            handle,
                            var_log_item.old_data.as_mut_ptr(),
                            header.old_data_size as usize,
                        );

                        if old_data_read.map(|v| v != header.old_data_size as usize).unwrap_or(true) {
                            read_success = false;
                        } else {
                            data_offset += header.old_data_size as usize;
                        }
                    } else {
                        let mut compressed_size_bytes = [0u8; 4];
                        let size_read = crate::platform::file_read(
                            handle,
                            compressed_size_bytes.as_mut_ptr(),
                            4,
                        );
                        
                        if size_read.map(|v| v != 4).unwrap_or(true) {
                            read_success = false;
                        } else {
                            let compressed_size = u32::from_le_bytes(compressed_size_bytes) as usize;
                            var_log_item.old_data.resize(compressed_size, 0);
                            let old_data_read = crate::platform::file_read(
                                handle,
                                var_log_item.old_data.as_mut_ptr(),
                                compressed_size,
                            );

                            if old_data_read.map(|v| v != compressed_size).unwrap_or(true) {
                                read_success = false;
                            } else {
                                // Decompress the data
                                match crate::wal_compression::decompress_wal_data(&var_log_item.old_data, compression_type) {
                                    Ok(decompressed) => {
                                        var_log_item.old_data = decompressed;
                                        data_offset += 4 + compressed_size;
                                    }
                                    Err(_) => {
                                        read_success = false;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if read_success && header.new_data_size > 0 {
                if let Err(_) = crate::platform::file_seek(
                    handle,
                    data_offset as i64,
                    crate::platform::SeekWhence::SeekSet,
                ) {
                    read_success = false;
                } else {
                    // Read compressed size (4 bytes) if compression is enabled
                    if matches!(compression_type, crate::config::WALCompressionType::None) {
                        var_log_item.new_data.resize(header.new_data_size as usize, 0);
                        let new_data_read = crate::platform::file_read(
                            handle,
                            var_log_item.new_data.as_mut_ptr(),
                            header.new_data_size as usize,
                        );

                        if new_data_read.map(|v| v != header.new_data_size as usize).unwrap_or(true) {
                            read_success = false;
                        } else {
                            data_offset += header.new_data_size as usize;
                        }
                    } else {
                        let mut compressed_size_bytes = [0u8; 4];
                        let size_read = crate::platform::file_read(
                            handle,
                            compressed_size_bytes.as_mut_ptr(),
                            4,
                        );
                        
                        if size_read.map(|v| v != 4).unwrap_or(true) {
                            read_success = false;
                        } else {
                            let compressed_size = u32::from_le_bytes(compressed_size_bytes) as usize;
                            var_log_item.new_data.resize(compressed_size, 0);
                            let new_data_read = crate::platform::file_read(
                                handle,
                                var_log_item.new_data.as_mut_ptr(),
                                compressed_size,
                            );

                            if new_data_read.map(|v| v != compressed_size).unwrap_or(true) {
                                read_success = false;
                            } else {
                                // Decompress the data
                                match crate::wal_compression::decompress_wal_data(&var_log_item.new_data, compression_type) {
                                    Ok(decompressed) => {
                                        var_log_item.new_data = decompressed;
                                        data_offset += 4 + compressed_size;
                                    }
                                    Err(_) => {
                                        read_success = false;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if read_success {
                let calculated_checksum = Transaction::calculate_variable_size_log_item_checksum(&var_log_item);
                if header.checksum == calculated_checksum {
                    let is_zero_initialized = header.tx_id == 0 && header.timestamp == 0;
                    let is_valid_insert = header.op_type == LogOperation::Insert && (header.old_data_size > 0 || header.new_data_size > 0);
                    let has_valid_data = header.old_data_size > 0 || header.new_data_size > 0;
                    let is_system_op = matches!(
                        header.op_type,
                        LogOperation::CreateTable
                            | LogOperation::CreateIndex
                            | LogOperation::Checkpoint
                            | LogOperation::Commit
                            | LogOperation::Abort
                            | LogOperation::EnterLowPowerMode
                            | LogOperation::ExitLowPowerMode
                            | LogOperation::AlterTable
                    );

                    if !is_zero_initialized || is_valid_insert || has_valid_data || is_system_op {
                        if matches!(
                            header.op_type,
                            LogOperation::Insert
                                | LogOperation::Delete
                                | LogOperation::Update
                                | LogOperation::CreateTable
                                | LogOperation::CreateDatabase
                                | LogOperation::TimeSeriesInsert
                                | LogOperation::Commit
                                | LogOperation::Abort
                                | LogOperation::Checkpoint
                                | LogOperation::CreateIndex
                                | LogOperation::EnterLowPowerMode
                                | LogOperation::ExitLowPowerMode
                                | LogOperation::AlterTable
                                | LogOperation::DropTable
                        ) {
                            valid_log_items.push(var_log_item);
                            current_offset += HEADER_SIZE_BYTES + header.old_data_size as usize + header.new_data_size as usize;
                            record_index += 1;
                            consecutive_invalid_records = 0;
                            continue;
                        }
                    }
                }
            }

            consecutive_invalid_records += 1;
            
            // 如果连续无效记录达到跳过阈值，尝试跳过预分配的未使用空间
            if consecutive_invalid_records == skip_threshold {
                #[cfg(feature = "log")]
                warn!("Detected {} consecutive invalid records, attempting to skip preallocated space at offset {}", consecutive_invalid_records, current_offset);
                
                let mut skip_attempts = 0;
                let mut found_valid = false;
                
                // 尝试多次跳过，直到找到有效记录或达到最大尝试次数
                while skip_attempts < max_skip_attempts {
                    let new_offset = current_offset + skip_block_size;
                    if new_offset >= file_size {
                        break;
                    }
                    
                    #[cfg(feature = "log")]
                    warn!("Skip attempt {}: trying offset {} (+{} bytes)", skip_attempts + 1, new_offset, skip_block_size);
                    
                    // 检查新偏移量处是否有有效记录
                    // 这里我们直接跳过，让主循环的下一次迭代来检查
                    // 如果跳过后仍然是无效记录，计数器会增加，我们会继续尝试
                    current_offset = new_offset;
                    skip_attempts += 1;
                    
                    // 重置计数器，因为我们已经跳过了这些无效记录
                    consecutive_invalid_records = 0;
                    
                    // 继续主循环，让下一次迭代检查新位置
                    found_valid = true;
                    break;
                }
                
                if found_valid {
                    continue;
                } else {
                    #[cfg(feature = "log")]
                    warn!("Failed to find valid records after {} skip attempts, continuing normal recovery", skip_attempts);
                }
            }
            
            if consecutive_invalid_records > max_consecutive_invalid {
                #[cfg(feature = "log")]
                error!("Too many consecutive invalid records ({}), stopping recovery at offset {}", consecutive_invalid_records, current_offset);
                break;
            }

            #[cfg(feature = "log")]
            warn!("Invalid log item at offset {}, skipping to next record (consecutive invalid: {})", current_offset, consecutive_invalid_records);
            current_offset += HEADER_SIZE_BYTES;
        }

        let _ = crate::platform::file_close(handle);

        #[cfg(feature = "log")]
        info!(
            "Phase 1 completed: {} valid log items read",
            valid_log_items.len()
        );

        if valid_log_items.is_empty() {
            #[cfg(feature = "log")]
            info!("WAL file is empty, no data to recover");
            return Err(RemDbError::FileIoError);
        }

        let mut data_operations_count = 0;
        for log_item in &valid_log_items {
            match log_item.header.op_type {
                LogOperation::Insert | LogOperation::Update | LogOperation::Delete => {
                    data_operations_count += 1;
                }
                _ => {}
            }
        }

        // 如果没有有效的数据操作，返回错误
        if data_operations_count == 0 {
            #[cfg(feature = "log")]
            info!("WAL file contains only schema operations, no data operations");
            return Err(RemDbError::FileIoError);
        }

        // 阶段2：先处理所有的数据库创建、表创建和索引创建操作，确保数据库和表结构已建立
        #[cfg(feature = "log")]
        info!("Phase 2: Processing schema operations (CreateDatabase, CreateTable, CreateIndex)...");
        for log_item in &valid_log_items {
            match log_item.header.op_type {
                LogOperation::CreateDatabase => {
                    // 执行创建数据库操作
                    // 从日志中解析数据库名
                    let db_name_len = if log_item.new_data.len() > 0 {
                        log_item.new_data[0] as usize
                    } else {
                        continue;
                    };
                    let db_name =
                        core::str::from_utf8(&log_item.new_data[1..1 + db_name_len])
                            .unwrap_or_else(|_| "unknown");

                    #[cfg(feature = "log")]
                    info!("Creating database from WAL: {}", db_name);

                    // 调用数据库管理器的创建数据库方法
                    let _ = db.database_manager.create_database(db_name, "", None);
                }
                LogOperation::CreateTable => {
                    // 正确解析CreateTable日志项，参考lib.rs中的实现
                    // 从日志中解析表名
                    let name_len = if log_item.new_data.len() > 0 {
                        log_item.new_data[0] as usize
                    } else {
                        #[cfg(feature = "log")]
                        warn!("Empty new_data for CreateTable log item, skipping...");
                        continue;
                    };

                    // 检查name_len是否有效且不会导致越界
                    if name_len == 0 || name_len + 1 > log_item.new_data.len() {
                        #[cfg(feature = "log")]
                        warn!("Invalid name_len {} for CreateTable log item, skipping...", name_len);
                        continue;
                    }
                    
                    let table_name_str = core::str::from_utf8(&log_item.new_data[1..1 + name_len])
                        .unwrap_or_else(|_| "unknown");
                    let table_name = Box::leak(table_name_str.to_string().into_boxed_str());

                    // 参考lib.rs中的实现，使用固定偏移量解析
                    let new_data_len = log_item.new_data.len();
                    
                    // 从日志中解析字段数量（固定偏移65）
                    let field_count = if new_data_len > 65 {
                        log_item.new_data[65] as usize
                    } else {
                        #[cfg(feature = "log")]
                        warn!("Offset out of bounds when parsing field count, skipping...");
                        continue;
                    };

                    // 从日志中解析主键字段数量（固定偏移66）
                    let primary_key_count = if new_data_len > 66 {
                        log_item.new_data[66] as usize
                    } else {
                        #[cfg(feature = "log")]
                        warn!("Offset out of bounds when parsing primary key count, skipping...");
                        continue;
                    };

                    // 从日志中解析主键索引列表（从固定偏移67开始）
                    let mut primary_key_list = Vec::new();
                    for i in 0..primary_key_count {
                        // 检查offset是否超出边界，如果超出则停止解析
                        let pk_offset = 67 + i;
                        if pk_offset >= new_data_len {
                            #[cfg(feature = "log")]
                            warn!("Offset out of bounds while parsing primary key index, skipping remaining indices...");
                            break;
                        }
                        let pk_idx = log_item.new_data[pk_offset] as usize;
                        primary_key_list.push(pk_idx);
                    }
                    let mut fields = alloc::vec::Vec::with_capacity(field_count);

                    // 计算字段解析的起始偏移量（固定偏移67 + 主键数量）
                    let mut offset = 67 + primary_key_count;
                    
                    // 确保offset初始值不超出边界
                    if offset >= new_data_len {
                        #[cfg(feature = "log")]
                        warn!(
                            "Invalid initial offset for CreateTable log item, skipping..."
                        );
                        continue;
                    }

                    for _ in 0..field_count {
                        // 检查offset是否超出边界，如果超出则停止解析
                        if offset >= new_data_len {
                            #[cfg(feature = "log")]
                            warn!("Offset out of bounds while parsing field, skipping remaining fields...");
                            break;
                        }

                        // 解析字段名
                        let field_name_len = log_item.new_data[offset] as usize;
                        offset += 1;

                        if offset + 32 > new_data_len {
                            #[cfg(feature = "log")]
                            warn!("Offset out of bounds for field name, skipping field...");
                            continue;
                        }
                        
                        let field_name_str = core::str::from_utf8(
                            &log_item.new_data[offset..offset + field_name_len],
                        )
                        .unwrap_or_else(|_| "unknown");
                        let field_name = Box::leak(field_name_str.to_string().into_boxed_str());
                        offset += 32; // 固定32字节字段名空间

                        // 检查offset是否超出边界
                        if offset + 3 >= new_data_len {
                            #[cfg(feature = "log")]
                            warn!("Offset out of bounds while parsing field type/constraints, skipping field...");
                            continue;
                        }

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

                        // 解析字段大小（4字节）
                        let field_size = if offset + 4 <= new_data_len {
                            u32::from_le_bytes([
                                log_item.new_data[offset],
                                log_item.new_data[offset + 1],
                                log_item.new_data[offset + 2],
                                log_item.new_data[offset + 3],
                            ]) as usize
                        } else {
                            // 如果没有字段大小信息，使用默认值
                            0
                        };
                        offset += 4;

                        // 解析向量维度（2字节）
                        let vector_dimension = if offset + 1 < new_data_len {
                            u16::from_le_bytes([log_item.new_data[offset], log_item.new_data[offset+1]])
                        } else {
                            0
                        };
                        offset += 2;

                        // 解析默认值存在标志
                        let has_default = log_item.new_data[offset] != 0;
                        offset += 1;

                        // 解析默认值
                        let default_value = if has_default {
                            // 保存当前offset，用于解析默认值
                            let _default_offset = offset;

                            // 根据数据类型解析默认值
                            let value = unsafe {
                                match data_type {
                                    crate::types::DataType::Bool => {
                                        if offset + 1 <= new_data_len {
                                            let val = log_item.new_data[offset] != 0;
                                            offset += 1;
                                            crate::types::Value { bool: val }
                                        } else {
                                            offset += 1;
                                            crate::types::Value { bool: false }
                                        }
                                    }
                                    crate::types::DataType::Int8 => {
                                        if offset + 1 <= new_data_len {
                                            let val = log_item.new_data[offset] as i8;
                                            offset += 1;
                                            crate::types::Value { i8: val }
                                        } else {
                                            offset += 1;
                                            crate::types::Value { i8: 0 }
                                        }
                                    }
                                    crate::types::DataType::UInt8 => {
                                        if offset + 1 <= new_data_len {
                                            let val = log_item.new_data[offset];
                                            offset += 1;
                                            crate::types::Value { u8: val }
                                        } else {
                                            offset += 1;
                                            crate::types::Value { u8: 0 }
                                        }
                                    }
                                    crate::types::DataType::Int16 => {
                                        if offset + 2 <= new_data_len {
                                            let val = i16::from_le_bytes([
                                                log_item.new_data[offset],
                                                log_item.new_data[offset + 1],
                                            ]);
                                            offset += 2;
                                            crate::types::Value { i16: val }
                                        } else {
                                            offset += 2;
                                            crate::types::Value { i16: 0 }
                                        }
                                    }
                                    crate::types::DataType::UInt16 => {
                                        if offset + 2 <= new_data_len {
                                            let val = u16::from_le_bytes([
                                                log_item.new_data[offset],
                                                log_item.new_data[offset + 1],
                                            ]);
                                            offset += 2;
                                            crate::types::Value { u16: val }
                                        } else {
                                            offset += 2;
                                            crate::types::Value { u16: 0 }
                                        }
                                    }
                                    crate::types::DataType::Int32 => {
                                        if offset + 4 <= new_data_len {
                                            let val = i32::from_le_bytes([
                                                log_item.new_data[offset],
                                                log_item.new_data[offset + 1],
                                                log_item.new_data[offset + 2],
                                                log_item.new_data[offset + 3],
                                            ]);
                                            offset += 4;
                                            crate::types::Value { i32: val }
                                        } else {
                                            offset += 4;
                                            crate::types::Value { i32: 0 }
                                        }
                                    }
                                    crate::types::DataType::UInt32 => {
                                        if offset + 4 <= new_data_len {
                                            let val = u32::from_le_bytes([
                                                log_item.new_data[offset],
                                                log_item.new_data[offset + 1],
                                                log_item.new_data[offset + 2],
                                                log_item.new_data[offset + 3],
                                            ]);
                                            offset += 4;
                                            crate::types::Value { u32: val }
                                        } else {
                                            offset += 4;
                                            crate::types::Value { u32: 0 }
                                        }
                                    }
                                    crate::types::DataType::Float32 => {
                                        if offset + 4 <= new_data_len {
                                            let val = f32::from_le_bytes([
                                                log_item.new_data[offset],
                                                log_item.new_data[offset + 1],
                                                log_item.new_data[offset + 2],
                                                log_item.new_data[offset + 3],
                                            ]);
                                            offset += 4;
                                            crate::types::Value { float32: val }
                                        } else {
                                            offset += 4;
                                            crate::types::Value { float32: 0.0 }
                                        }
                                    }
                                    crate::types::DataType::Int64 => {
                                        if offset + 8 <= new_data_len {
                                            let val = i64::from_le_bytes([
                                                log_item.new_data[offset],
                                                log_item.new_data[offset + 1],
                                                log_item.new_data[offset + 2],
                                                log_item.new_data[offset + 3],
                                                log_item.new_data[offset + 4],
                                                log_item.new_data[offset + 5],
                                                log_item.new_data[offset + 6],
                                                log_item.new_data[offset + 7],
                                            ]);
                                            offset += 8;
                                            crate::types::Value { i64: val }
                                        } else {
                                            offset += 8;
                                            crate::types::Value { i64: 0 }
                                        }
                                    }
                                    crate::types::DataType::UInt64 => {
                                        if offset + 8 <= new_data_len {
                                            let val = u64::from_le_bytes([
                                                log_item.new_data[offset],
                                                log_item.new_data[offset + 1],
                                                log_item.new_data[offset + 2],
                                                log_item.new_data[offset + 3],
                                                log_item.new_data[offset + 4],
                                                log_item.new_data[offset + 5],
                                                log_item.new_data[offset + 6],
                                                log_item.new_data[offset + 7],
                                            ]);
                                            offset += 8;
                                            crate::types::Value { u64: val }
                                        } else {
                                            offset += 8;
                                            crate::types::Value { u64: 0 }
                                        }
                                    }
                                    crate::types::DataType::Float64 => {
                                        if offset + 8 <= new_data_len {
                                            let val = f64::from_le_bytes([
                                                log_item.new_data[offset],
                                                log_item.new_data[offset + 1],
                                                log_item.new_data[offset + 2],
                                                log_item.new_data[offset + 3],
                                                log_item.new_data[offset + 4],
                                                log_item.new_data[offset + 5],
                                                log_item.new_data[offset + 6],
                                                log_item.new_data[offset + 7],
                                            ]);
                                            offset += 8;
                                            crate::types::Value { float64: val }
                                        } else {
                                            offset += 8;
                                            crate::types::Value { float64: 0.0 }
                                        }
                                    }
                                    crate::types::DataType::VarChar | crate::types::DataType::Char | crate::types::DataType::Text => {
                                        // 确保有足够空间读取字符串长度
                                        if offset + 1 <= new_data_len {
                                            let _str_len = log_item.new_data[offset] as usize; // 1字节长度
                                            offset += 1;

                                            // 创建字符串默认值
                                            let mut str_val = [0u8; crate::types::MAX_STRING_LEN];

                                            // 字符串内容固定64字节
                                            let str_data_size = 64;

                                            // 只读取实际需要的字符串数据，不超过剩余空间
                                            let actual_data_size =
                                                if offset + str_data_size <= new_data_len {
                                                    str_data_size
                                                } else {
                                                    // 空间不足，只读取可用的数据
                                                    let remaining = new_data_len - offset;
                                                    remaining
                                                };

                                            // 复制字符串数据
                                            for i in 0..actual_data_size {
                                                if i < str_val.len() {
                                                    str_val[i] = log_item.new_data[offset + i];
                                                }
                                            }
                                            offset += str_data_size; // 固定64字节字符串空间

                                            crate::types::Value { string: str_val }
                                        } else {
                                            // 空间不足，跳过字符串长度
                                            offset += 1;
                                            offset += 64; // 跳过固定64字节字符串空间
                                            crate::types::Value {
                                                string: [0u8; crate::types::MAX_STRING_LEN],
                                            }
                                        }
                                    }
                                    _ => {
                                        // 默认跳过8字节，但确保不超出边界
                                        if offset + 7 < new_data_len {
                                            offset += 8;
                                        } else {
                                            // 空间不足，只跳过可用的数据
                                            offset = new_data_len;
                                        }
                                        crate::types::Value { u64: 0 }
                                    }
                                }
                            };
                            Some(value)
                        } else {
                            // 没有默认值
                            None
                        };

                        // 计算字段大小：优先使用从日志中读取的field_size，如果为0则根据数据类型计算
                        let final_field_size = if field_size > 0 {
                            field_size
                        } else {
                            match data_type {
                                crate::types::DataType::Bool
                                | crate::types::DataType::Int8
                                | crate::types::DataType::UInt8 => 1,
                                crate::types::DataType::Int16 | crate::types::DataType::UInt16 => 2,
                                crate::types::DataType::Int32
                                | crate::types::DataType::UInt32
                                | crate::types::DataType::Float32 => 4,
                                crate::types::DataType::Int64
                                | crate::types::DataType::UInt64
                                | crate::types::DataType::Float64
                                | crate::types::DataType::Timestamp
                                | crate::types::DataType::TimestampTZ => 8,
                                crate::types::DataType::VarChar | crate::types::DataType::Char => 64, // 默认64字节字符串
                                crate::types::DataType::Text => DEFAULT_TEXT_SIZE, // TEXT类型默认512字节
                                crate::types::DataType::Json => DEFAULT_JSON_SIZE, // JSON类型默认512字节
                                crate::types::DataType::Vector => {
                                    // 向量大小 = 维度 * 4字节（float32）
                                    if vector_dimension > 0 {
                                        vector_dimension as usize * 4
                                    } else {
                                        8 // 默认8字节
                                    }
                                }
                                crate::types::DataType::Interval => 10, // 8字节值 + 1字节精度 + 1字节标志
                                _ => 8,                                 // 默认8字节
                            }
                        };

                        // 创建向量元数据（如果是向量类型且维度大于0）
                        let vector_metadata = if data_type == crate::types::DataType::Vector && vector_dimension > 0 {
                            Some(crate::types::VectorMetadata {
                                dimension: vector_dimension,
                                distance_type: crate::types::DistanceType::L2, // 默认L2距离
                                index_type: crate::types::VectorIndexType::HNSW, // 默认HNSW索引
                                compression_enabled: false, // 默认不启用压缩
                                compression_scheme: 0, // 默认无压缩
                                compression_level: 3, // 默认压缩级别
                                hnsw_m: 16,
                                hnsw_ef_construction: 200,
                                hnsw_ef_search: 128,
                                ivf_nlist: 1024,
                                ivf_nprobe: 16,
                            })
                        } else {
                            None
                        };

                        // 创建字段定义
                        let field_def = crate::types::FieldDef {
                            name: field_name.to_string(),
                            data_type,
                            size: final_field_size,
                            string_length: None,
                            offset: 0, // 偏移量会在表创建时计算
                            primary_key: primary_key_flag,
                            not_null: not_null_flag,
                            unique: unique_flag,
                            auto_increment: auto_increment_flag,
                            default_value: default_value, // 使用解析出的默认值
                            vector_metadata: vector_metadata, // 设置向量元数据
                            json_metadata: None,
                        };

                        fields.push(field_def);
                    }

                    // 从日志中解析record_size和max_records，但确保不超出边界
                    let mut record_size = if offset + 1 < log_item.new_data.len() {
                        u16::from_le_bytes([
                            log_item.new_data[offset],
                            log_item.new_data[offset + 1],
                        ]) as usize
                    } else {
                        // 超出边界，使用默认值
                        0
                    };
                    offset += 2;

                    // 如果record_size为0，根据字段大小重新计算
                    if record_size == 0 {
                        record_size = fields.iter().fold(0, |acc, field| acc + field.size);
                    }

                    let mut max_records = if offset + 3 < log_item.new_data.len() {
                        u32::from_le_bytes([
                            log_item.new_data[offset],
                            log_item.new_data[offset + 1],
                            log_item.new_data[offset + 2],
                            log_item.new_data[offset + 3],
                        ]) as usize
                    } else {
                        // 超出边界，使用默认值
                        100000
                    };
                    offset += 4;

                    // 确保max_records至少为1，避免创建无法使用的表
                    if max_records == 0 {
                        max_records = 100000; // 使用默认值
                    }

                    // 检查并限制max_records，避免内存不足
                    // 优先使用低功耗模式限制，否则使用默认最大记录数
                    let max_allowed = if db.config.low_power_mode_supported {
                        db.config.low_power_max_records.unwrap_or(db.config.default_max_records)
                    } else {
                        db.config.default_max_records
                    };
                    
                    if max_records > max_allowed {
                        #[cfg(feature = "log")]
                        info!("Limiting table '{}' max_records from {} to {} to prevent memory exhaustion", 
                            table_name, max_records, max_allowed);
                        max_records = max_allowed;
                    }

                    // 创建表定义
                    #[cfg(feature = "log")]
                    info!(
                        "Creating table from WAL for table_id {} (table name: {})",
                        log_item.header.table_id, table_name
                    );

                    // 计算字段偏移量
                    let mut offset = 0;
                    for field in &mut fields {
                        field.offset = offset;
                        offset += field.size;
                    }

                    // 将字段定义转换为静态切片


                    // 创建表定义
                    let table_def = crate::types::TableDef {
                        id: log_item.header.table_id,
                        name: table_name.to_string(),
                        fields: fields,
                        primary_key: primary_key_list,
                        secondary_index: None,
                        secondary_index_type: crate::types::IndexType::SortedArray,
                        record_size: record_size,
                        max_records: max_records,
                        version: 1u32,
                        created_at: crate::platform::get_timestamp_us(),
                        updated_at: crate::platform::get_timestamp_us(),
                    };

                    // 直接实现从TableDef创建表的逻辑
                    unsafe {
                        // 检查表格是否已存在（同时检查ID和表名）
                        let table_exists_by_id = db.tables.len() > table_def.id as usize
                            && db.tables[table_def.id as usize].is_some();
                        
                        // 检查是否有同名的表（即使ID不同）
                        let table_exists_by_name = db.tables.iter().any(|t| {
                            if let Some(table) = t {
                                table.def.name.as_str() == table_name
                            } else {
                                false
                            }
                        });
                        
                        if table_exists_by_id || table_exists_by_name {
                            #[cfg(feature = "log")]
                            warn!("Skipping CreateTable operation for table_id {} (table '{}' already exists)", log_item.header.table_id, table_name);
                            continue;
                        }

                        // 确保tables向量有足够的容量
                        if table_def.id as usize >= db.tables.len() {
                            let new_capacity =
                                core::cmp::max(db.tables.len() * 2, table_def.id as usize + 1);
                            db.tables.resize_with(new_capacity, || None);
                            db.primary_indices.resize_with(new_capacity, || None);
                            db.secondary_indices.resize_with(new_capacity, || None);
                        }

                        // 创建内存表，跳过状态初始化，由WAL恢复过程处理
                        let table_def_arc = alloc::sync::Arc::new(table_def.clone());
                        match crate::table::MemoryTable::new_with_options(table_def_arc.clone(), true) {
                            Ok(table) => {
                                // 添加到表向量
                                db.tables[table_def.id as usize] = Some(table);

                                // 创建主键索引
                                let hash_table_size =
                                    (table_def.max_records * 2).next_power_of_two();
                                let index_memory_size =
                                    crate::index::PrimaryIndex::calculate_memory_size(
                                        &table_def,
                                        hash_table_size,
                                        table_def.max_records,
                                    );

                                match crate::memory::allocator::alloc(index_memory_size) {
                                    Ok(index_memory) => {
                                        let hash_table_start = index_memory.as_ptr()
                                            as *mut Option<
                                                core::ptr::NonNull<crate::index::PrimaryIndexItem>,
                                            >;
                                        let items_start = (index_memory.as_ptr() as usize
                                            + hash_table_size
                                                * core::mem::size_of::<
                                                    Option<
                                                        core::ptr::NonNull<
                                                            crate::index::PrimaryIndexItem,
                                                        >,
                                                    >,
                                                >(
                                                ))
                                            as *mut crate::index::PrimaryIndexItem;

                                        let primary_index = crate::index::PrimaryIndex::new(
                                            table_def_arc.clone(),
                                            hash_table_start,
                                            items_start,
                                            hash_table_size,
                                            table_def.max_records,
                                        );
                                        db.primary_indices[table_def.id as usize] =
                                            Some(primary_index);

                                        // 初始化辅助索引位置
                                        db.secondary_indices[table_def.id as usize] = None;
                                    }
                                    Err(err) => {
                                        #[cfg(feature = "log")]
                                        warn!("Failed to allocate memory for primary index: {:?}, skipping CreateTable operation for table_id {}", err, log_item.header.table_id);
                                        db.tables[table_def.id as usize] = None;
                                        continue;
                                    }
                                }
                            }
                            Err(err) => {
                                #[cfg(feature = "log")]
                                warn!("Failed to create MemoryTable: {:?}, skipping CreateTable operation for table_id {}", err, log_item.header.table_id);
                                continue;
                            }
                        }
                    }
                }
                LogOperation::CreateIndex => {
                    // 执行创建索引操作
                    // 从日志中解析表名和字段名
                    let table_name_len = if log_item.new_data.len() > 0 {
                        log_item.new_data[0] as usize
                    } else {
                        continue;
                    };
                    let table_name = 
                        core::str::from_utf8(&log_item.new_data[1..1 + table_name_len])
                            .unwrap_or_else(|_| "unknown");

                    let field_name_len = if log_item.new_data.len() > 65 {
                        log_item.new_data[65] as usize
                    } else {
                        continue;
                    };
                    let field_name = 
                        core::str::from_utf8(&log_item.new_data[66..66 + field_name_len])
                            .unwrap_or_else(|_| "unknown");

                    let index_type: crate::types::IndexType = if log_item.new_data.len() > 130 {
                        log_item.new_data[130].into()
                    } else {
                        continue;
                    };

                    // 调用数据库的create_index方法
                    let _ = db.create_index(table_name, field_name, index_type);
                }
                LogOperation::AlterTable => {
                    // 从日志中解析表名
                    let table_name_len = if log_item.new_data.len() > 0 {
                        log_item.new_data[0] as usize
                    } else {
                        continue;
                    };
                    let table_name = 
                        core::str::from_utf8(&log_item.new_data[1..1 + table_name_len])
                            .unwrap_or_else(|_| "unknown");

                    // 从日志中解析操作类型
                    let op_type = if log_item.new_data.len() > 65 {
                        log_item.new_data[65] as usize
                    } else {
                        continue;
                    };
                    
                    // 根据操作类型执行不同的恢复逻辑
                    let alter_operation = match op_type {
                        0 => {
                            // AddColumn
                            let col_name_len = if log_item.new_data.len() > 67 {
                                log_item.new_data[67] as usize
                            } else {
                                continue;
                            };
                            let col_name = core::str::from_utf8(&log_item.new_data[68..68 + col_name_len])
                                .unwrap_or_else(|_| "unknown")
                                .to_string();
                            
                            let data_type: crate::types::DataType = if log_item.new_data.len() > 132 {
                                log_item.new_data[132].into()
                            } else {
                                continue;
                            };
                            let size = if log_item.new_data.len() > 134 {
                                u16::from_le_bytes([log_item.new_data[133], log_item.new_data[134]])
                            } else {
                                continue;
                            };
                            
                            let constraints = if log_item.new_data.len() > 135 {
                                log_item.new_data[135]
                            } else {
                                continue;
                            };
                            let primary_key = (constraints & 0b0001) != 0;
                            let not_null = (constraints & 0b0010) != 0;
                            let unique = (constraints & 0b0100) != 0;
                            let auto_increment = (constraints & 0b1000) != 0;
                            
                            crate::AlterTableOperation::AddColumn {
                                name: col_name,
                                data_type,
                                size,
                                distance_type: None,
                                default_value: None,
                                constraints: crate::FieldConstraint {
                                    primary_key,
                                    not_null,
                                    unique,
                                    auto_increment,
                                },
                            }
                        },
                        1 => {
                            // DropColumn
                            let col_name_len = if log_item.new_data.len() > 67 {
                                log_item.new_data[67] as usize
                            } else {
                                continue;
                            };
                            let col_name = core::str::from_utf8(&log_item.new_data[68..68 + col_name_len])
                                .unwrap_or_else(|_| "unknown")
                                .to_string();
                            
                            crate::AlterTableOperation::DropColumn {
                                name: col_name,
                            }
                        },
                        2 => {
                            // ModifyColumn
                            let col_name_len = if log_item.new_data.len() > 67 {
                                log_item.new_data[67] as usize
                            } else {
                                continue;
                            };
                            let col_name = core::str::from_utf8(&log_item.new_data[68..68 + col_name_len])
                                .unwrap_or_else(|_| "unknown")
                                .to_string();
                            
                            let data_type: crate::types::DataType = if log_item.new_data.len() > 132 {
                                log_item.new_data[132].into()
                            } else {
                                continue;
                            };
                            let size = if log_item.new_data.len() > 134 {
                                u16::from_le_bytes([log_item.new_data[133], log_item.new_data[134]])
                            } else {
                                continue;
                            };
                            
                            let constraints = if log_item.new_data.len() > 135 {
                                log_item.new_data[135]
                            } else {
                                continue;
                            };
                            let primary_key = (constraints & 0b0001) != 0;
                            let not_null = (constraints & 0b0010) != 0;
                            let unique = (constraints & 0b0100) != 0;
                            let auto_increment = (constraints & 0b1000) != 0;
                            
                            crate::AlterTableOperation::ModifyColumn {
                                name: col_name,
                                data_type,
                                size,
                                distance_type: None,
                                default_value: None,
                                constraints: crate::FieldConstraint {
                                    primary_key,
                                    not_null,
                                    unique,
                                    auto_increment,
                                },
                            }
                        },
                        3 => {
                            // RenameColumn
                            let old_name_len = if log_item.new_data.len() > 67 {
                                log_item.new_data[67] as usize
                            } else {
                                continue;
                            };
                            let old_name = core::str::from_utf8(&log_item.new_data[68..68 + old_name_len])
                                .unwrap_or_else(|_| "unknown")
                                .to_string();
                            
                            let new_name_len = if log_item.new_data.len() > 132 {
                                log_item.new_data[132] as usize
                            } else {
                                continue;
                            };
                            let new_name = core::str::from_utf8(&log_item.new_data[133..133 + new_name_len])
                                .unwrap_or_else(|_| "unknown")
                                .to_string();
                            
                            crate::AlterTableOperation::RenameColumn {
                                old_name,
                                new_name,
                            }
                        },
                        _ => {
                            // 未知操作类型，跳过
                            #[cfg(feature = "log")]
                            warn!(
                                "Unknown AlterTable operation type {} for table {} from WAL - skipping",
                                op_type, table_name
                            );
                            continue;
                        }
                    };

                    // 执行ALTER TABLE操作
                    if let Err(err) = DdlExecutor::alter_table(db, table_name, alter_operation) {
                        #[cfg(feature = "log")]
                        error!(
                            "Failed to recover AlterTable operation for table {} from WAL: {:?}",
                            table_name, err
                        );
                    } else {
                        #[cfg(feature = "log")]
                        info!(
                            "Successfully recovered AlterTable operation for table {} from WAL",
                            table_name
                        );
                    }
                }
                _ => continue, // 跳过非schema操作
            }
        }

        // 阶段3：处理所有的数据操作和系统操作
        #[cfg(feature = "log")]
        info!("Phase 3: Processing operations (Insert, Update, Delete, TimeSeriesInsert, LowPowerMode)...");
        let mut data_operations_count = 0;
        for log_item in &valid_log_items {
            match log_item.header.op_type {
                LogOperation::EnterLowPowerMode => {
                    // 处理进入低功耗模式日志
                    #[cfg(feature = "log")]
                    info!("Processing EnterLowPowerMode log item");

                    // 检查配置是否支持低功耗模式
                    if db.config.low_power_mode_supported {
                        // 设置事务管理器为低功耗模式
                        crate::transaction::set_low_power_mode(true);

                        // 如果数据库尚未进入低功耗模式，执行相关操作
                        if !db.is_low_power_mode() {
                            // 执行进入低功耗模式的准备工作
                            unsafe {
                                // 检查当前内存使用情况
                                let current_memory = db.config.total_memory;
                                if current_memory > db.low_power_memory_limit {
                                    // 内存使用超出限制，需要进行优化
                                    db.optimize_memory_usage();
                                }
                            }

                            // 遍历所有表，设置低功耗模式
                            for table in &mut db.tables.iter_mut() {
                                if let Some(table) = table {
                                    table.set_low_power_mode(true, db.config.low_power_max_records);
                                }
                            }

                            // 更新状态
                            db.low_power_mode = true;
                        }
                    } else {
                        // 如果配置不支持低功耗模式，确保事务管理器也处于正常模式
                        crate::transaction::set_low_power_mode(false);
                    }
                }
                LogOperation::ExitLowPowerMode => {
                    // 处理退出低功耗模式日志
                    #[cfg(feature = "log")]
                    info!("Processing ExitLowPowerMode log item");

                    // 无论配置是否支持低功耗模式，都确保事务管理器处于正常模式
                    crate::transaction::set_low_power_mode(false);

                    // 检查配置是否支持低功耗模式
                    if db.config.low_power_mode_supported {
                        // 如果数据库当前处于低功耗模式，执行相关操作
                        if db.is_low_power_mode() {
                            // 执行退出低功耗模式的准备工作
                            unsafe {
                                // 恢复正常的索引更新频率
                                // 恢复正常的事务日志写入频率
                                // 检查并扩展内存使用（如果需要）
                            }

                            // 遍历所有表，退出低功耗模式
                            for table in &mut db.tables.iter_mut() {
                                if let Some(table) = table {
                                    table.set_low_power_mode(false, None);
                                }
                            }

                            // 更新状态
                            db.low_power_mode = false;
                        }
                    } else {
                        // 如果配置不支持低功耗模式，确保所有表也处于正常模式
                        for table in &mut db.tables.iter_mut() {
                            if let Some(table) = table {
                                table.set_low_power_mode(false, None);
                            }
                        }
                        // 确保数据库状态也更新
                        db.low_power_mode = false;
                    }
                }
                LogOperation::Insert => {
                    // 执行插入操作
                    // 检查table_id是否在有效范围内
                    let table_id = log_item.header.table_id as usize;
                    if table_id >= db.tables.len() {
                        // 表可能还未创建，跳过当前日志项
                        #[cfg(feature = "log")]
                        warn!("Table ID {} out of bounds (tables.len() = {}), skipping Insert log item", table_id, db.tables.len());
                        continue;
                    }
                    let table = match &mut db.tables[table_id] {
                        Some(table) => table,
                        None => {
                            #[cfg(feature = "log")]
                            warn!(
                                "Table ID {} exists but is None, skipping Insert log item",
                                table_id
                            );
                            continue;
                        }
                    };

                    #[cfg(feature = "log")]
                    info!("Processing Insert operation for table_id {} record_id {}", table_id, log_item.header.record_id);
                    data_operations_count += 1;

                    // 无论记录是否存在，都执行插入操作
                    let status_ptr = table.get_status_ptr(log_item.header.record_id as usize);
                    let record_ptr = table.get_record_ptr_mut(log_item.header.record_id as usize);
                    
                    // 复制数据到记录位置
                    // 使用实际的记录大小，而不是LOG_DATA_BULK_SIZE
                    let actual_data_size = core::cmp::min(log_item.new_data.len(), table.record_size);
                    crate::platform::memcpy(
                        record_ptr,
                        log_item.new_data.as_ptr(),
                        actual_data_size,
                    );

                    // 检查记录是否已存在
                    if (*status_ptr).status != crate::types::RecordStatus::Used {
                        // 记录不存在，更新状态和计数
                        (*status_ptr).status = crate::types::RecordStatus::Used;
                        (*status_ptr).version += 1;
                        (*status_ptr).create_tx_id = log_item.header.tx_id;
                        (*status_ptr).delete_tx_id = 0;
                        (*status_ptr).next_version_ptr = 0;
                        table.record_count += 1;

                        // 从空闲槽栈中移除该槽位，确保不重复使用
                        if table.free_slot_count > 0 {
                            // 查找并移除该槽位
                            let mut found = false;
                            let mut i = 0;
                            while i < table.free_slot_count {
                                if *table.free_slots.as_ptr().add(i) == log_item.header.record_id as usize
                                {
                                    // 找到，将最后一个元素移动到当前位置
                                    *table.free_slots.as_ptr().add(i) =
                                        *table.free_slots.as_ptr().add(table.free_slot_count - 1);
                                    table.free_slot_count -= 1;
                                    found = true;
                                    break;
                                }
                                i += 1;
                            }
                            if !found {
                                // 如果没有找到，说明可能已经被移除，或者初始状态不对，直接减少free_slot_count
                                // 但确保不小于0
                                if table.free_slot_count > 0 {
                                    table.free_slot_count -= 1;
                                }
                            }
                        }
                    } else {
                        // 记录已存在，更新版本号和创建事务ID
                        (*status_ptr).version += 1;
                        (*status_ptr).create_tx_id = log_item.header.tx_id;
                    }

                    // 更新主键索引
                    if let Some(primary_index) = &mut db.primary_indices[table_id] {
                        // 先删除旧的索引项（如果存在）
                        let _: Result<()> = primary_index.delete_composite(record_ptr);
                        // 使用复合键插入方法
                        let _: Result<()> = primary_index.insert_composite(
                            record_ptr,
                            log_item.header.record_id as u16,
                        );
                    }

                    // 更新表的max_pk值，确保新插入的记录不会覆盖旧记录
                    // 对于复合主键，只考虑第一个主键字段
                    if !table.def.primary_key.is_empty() {
                        let primary_key_field = &table.def.fields[table.def.primary_key[0]];
                        let key_ptr = record_ptr.add(primary_key_field.offset);
                        let new_pk = match primary_key_field.data_type {
                            crate::types::DataType::UInt8 => {
                                (unsafe { std::ptr::read_unaligned(key_ptr as *const u8) }) as u64
                            }
                            crate::types::DataType::UInt16 => {
                                (unsafe { std::ptr::read_unaligned(key_ptr as *const u16) }) as u64
                            }
                            crate::types::DataType::UInt32 => {
                                (unsafe { std::ptr::read_unaligned(key_ptr as *const u32) }) as u64
                            }
                            crate::types::DataType::UInt64 => unsafe { std::ptr::read_unaligned(key_ptr as *const u64) },
                            crate::types::DataType::Int8 => {
                                (unsafe { std::ptr::read_unaligned(key_ptr as *const i8) }) as u64
                            }
                            crate::types::DataType::Int16 => {
                                (unsafe { std::ptr::read_unaligned(key_ptr as *const i16) }) as u64
                            }
                            crate::types::DataType::Int32 => {
                                (unsafe { std::ptr::read_unaligned(key_ptr as *const i32) }) as u64
                            }
                            crate::types::DataType::Int64 => {
                                (unsafe { std::ptr::read_unaligned(key_ptr as *const i64) }) as u64
                            }
                            _ => 0,
                        };
                        if new_pk > table.max_pk {
                            table.max_pk = new_pk;
                        }
                    }
                }
                LogOperation::Delete => {
                    // 执行删除操作
                    // 检查table_id是否在有效范围内
                    let table_id = log_item.header.table_id as usize;
                    if table_id >= db.tables.len() {
                        // 表可能还未创建，跳过当前日志项
                        #[cfg(feature = "log")]
                        warn!("Table ID {} out of bounds (tables.len() = {}), skipping Delete log item", table_id, db.tables.len());
                        continue;
                    }
                    let table = match &mut db.tables[table_id] {
                        Some(table) => table,
                        None => {
                            #[cfg(feature = "log")]
                            warn!("Table ID {} exists but is None, skipping Delete log item", table_id);
                            continue;
                        }
                    };

                    // 检查记录是否存在
                    let status_ptr = table.get_status_ptr(log_item.header.record_id as usize);
                    if (*status_ptr).status == crate::types::RecordStatus::Used {
                        // 从主键索引中删除
                        if let Some(primary_index) = &mut db.primary_indices[table_id] {
                            let record_ptr = table.get_record_ptr_mut(log_item.header.record_id as usize);
                            let _: Result<()> = primary_index.delete_composite(record_ptr);
                        }

                        // 与实际delete方法保持一致：直接标记为Free
                        (*status_ptr).status = crate::types::RecordStatus::Free;
                        (*status_ptr).version += 1;

                        // 清空记录数据
                        let record_ptr = table.get_record_ptr_mut(log_item.header.record_id as usize);
                        crate::platform::memset(record_ptr, 0, table.record_size);

                        // 将空闲槽压回栈中，确保不超过数组大小
                        if table.free_slot_count < table.def.max_records {
                            *table.free_slots.as_ptr().add(table.free_slot_count) =
                                log_item.header.record_id as usize;
                            table.free_slot_count += 1;
                        }

                        // 更新记录计数
                        table.record_count -= 1;
                    }
                }
                LogOperation::Update => {
                    // 执行更新操作
                    // 检查table_id是否在有效范围内
                    let table_id = log_item.header.table_id as usize;
                    if table_id >= db.tables.len() {
                        // 表可能还未创建，跳过当前日志项
                        #[cfg(feature = "log")]
                        warn!("Table ID {} out of bounds (tables.len() = {}), skipping Update log item", table_id, db.tables.len());
                        continue;
                    }
                    let table = match &mut db.tables[table_id] {
                        Some(table) => table,
                        None => {
                            #[cfg(feature = "log")]
                            warn!("Table ID {} exists but is None, skipping Update log item", table_id);
                            continue;
                        }
                    };

                    // 检查记录是否存在
                    let status_ptr = table.get_status_ptr(log_item.header.record_id as usize);
                    if (*status_ptr).status == crate::types::RecordStatus::Used {
                        // 从主键索引中删除旧记录
                        if let Some(primary_index) = &mut db.primary_indices[table_id] {
                            let record_ptr = table.get_record_ptr_mut(log_item.header.record_id as usize);
                            let _: Result<()> = primary_index.delete_composite(record_ptr);
                        }

                        // 记录存在，执行更新
                        let record_ptr = table.get_record_ptr_mut(log_item.header.record_id as usize);
                        let data_size = core::cmp::min(log_item.header.new_data_size as usize, log_item.new_data.len());
                        crate::platform::memcpy(
                            record_ptr,
                            log_item.new_data.as_ptr(),
                            data_size,
                        );

                        (*status_ptr).version += 1;
                        (*status_ptr).create_tx_id = log_item.header.tx_id;

                        // 将新记录插入到主键索引中
                        if let Some(primary_index) = &mut db.primary_indices[table_id] {
                            let _: Result<()> = primary_index.insert_composite(
                                record_ptr,
                                log_item.header.record_id as u16,
                            );
                        }
                    }
                }
                LogOperation::TimeSeriesInsert => {
                    // 执行时间序列插入操作
                    if (log_item.header.table_id as usize) < db.time_series_tables.len() {
                        let ts_table = match &mut db.time_series_tables[log_item.header.table_id as usize]
                        {
                            Some(table) => table,
                            None => {
                                #[cfg(feature = "log")]
                                warn!("TimeSeries table ID {} exists but is None, skipping TimeSeriesInsert log item", log_item.header.table_id);
                                continue;
                            }
                        };

                        // 从日志中解析出时间序列记录
                        let mut record = crate::time_series::TimeSeriesRecord {
                            timestamp: 0,
                            value: 0.0,
                            tag_count: 0,
                            tags: [0; 8],
                        };
                        let data_size = core::cmp::min(core::mem::size_of::<crate::time_series::TimeSeriesRecord>(), log_item.new_data.len());
                        crate::platform::memcpy(
                            &mut record as *mut _ as *mut u8,
                            log_item.new_data.as_ptr(),
                            data_size,
                        );

                        // 获取或创建分区
                        let mut partitions_guard = try_lock!(ts_table.partitions);
                        let partition = partitions_guard.get_or_create_partition(record.timestamp);

                        // 写入记录到分区
                        let mut partition_guard = try_lock!(partition);
                        partition_guard.records.push(record);
                        partition_guard.stats.record_count = partition_guard.records.len();

                        // 更新索引
                        ts_table
                            .index
                            .insert(record.timestamp, partition_guard.records.len() - 1);
                    }
                }
                _ => continue, // 跳过非数据操作
            }
        }

        #[cfg(feature = "log")]
        info!("WAL recovery completed successfully");
        // 检查是否真的恢复了数据（至少有一个数据操作）
        if data_operations_count == 0 {
            #[cfg(feature = "log")]
            info!("WAL file contains only schema operations, no data operations");
            return Err(RemDbError::FileIoError);
        }
        Ok(())
    }
}

/// 事务管理器
pub struct TransactionManager {
    /// 当前活动事务
    current_tx: Option<NonNull<Transaction>>,
    /// 事务ID计数器
    tx_id_counter: u32,
    /// 日志管理器
    log_manager: Option<LogManager>,
    /// 快照版本号
    snapshot_version: u32,
    /// 活动快照列表
    active_snapshots: Vec<u32>,
    /// 低功耗模式标志
    low_power_mode: bool,
    /// 自旋锁
    lock: u32,
}

impl TransactionManager {
    /// 创建新的事务管理器
    pub const fn new() -> Self {
        Self {
            current_tx: None,
            tx_id_counter: 1,
            log_manager: None,
            snapshot_version: 0,
            active_snapshots: Vec::new(),
            low_power_mode: false,
            lock: 0,
        }
    }

    /// 设置日志管理器
    pub unsafe fn set_log_manager(&mut self, log_manager: LogManager) {
        self.log_manager = Some(log_manager);
    }

    /// 清除日志管理器
    pub fn clear_log_manager(&mut self) {
        self.log_manager = None;
    }

    /// 获取日志管理器（可变）
    pub unsafe fn get_log_manager_mut(&mut self) -> Option<&mut LogManager> {
        self.log_manager.as_mut()
    }

    /// 获取日志管理器（只读）
    pub unsafe fn get_log_manager(&self) -> Option<&LogManager> {
        self.log_manager.as_ref()
    }

    /// 刷新所有日志
    pub unsafe fn flush_logs(&mut self) -> Result<()> {
        if let Some(log_manager) = &mut self.log_manager {
            log_manager.flush_buffer()
        } else {
            Ok(())
        }
    }

    /// 检查是否有活动事务
    pub fn has_active_tx(&self) -> bool {
        self.current_tx.is_some()
    }

    /// 获取当前事务ID计数器
    pub fn tx_id_counter(&self) -> u32 {
        self.tx_id_counter
    }

    /// 开始事务
    pub unsafe fn begin(
        &mut self,
        tx_type: TransactionType,
        isolation_level: IsolationLevel,
        tx_buffer: *mut Transaction,
        log_buffer: *mut VariableSizeLogItem,
        max_log_items: usize,
    ) -> Result<NonNull<Transaction>> {
        // 自旋锁保护
        crate::platform::spin_lock(&mut self.lock);

        // 检查是否已有活动事务
        if self.current_tx.is_some() {
            crate::platform::spin_unlock(&mut self.lock);
            return Err(RemDbError::TransactionError);
        }

        // 生成新的事务ID
        let tx_id = self.tx_id_counter;
        self.tx_id_counter += 1;

        // 检查tx_buffer是否为空
        if tx_buffer.is_null() {
            crate::platform::spin_unlock(&mut self.lock);
            return Err(RemDbError::TransactionError);
        }

        // 初始化事务
        let tx = tx_buffer.as_mut().ok_or(RemDbError::InternalError("Transaction buffer not initialized"))?;
        *tx = Transaction {
            id: tx_id,
            tx_type,
            status: TransactionStatus::Active,
            isolation_level,
            start_time: crate::platform::get_timestamp_us(),
            log_items: NonNull::new(log_buffer).unwrap_or_else(|| NonNull::dangling()),
            max_log_items: if log_buffer.is_null() { 0 } else { max_log_items },
            log_item_count: 0,
            depth: 1,
            lock: 0,
        };

        // 设置当前事务
        self.current_tx = Some(NonNull::new_unchecked(tx_buffer));

        // 解锁
        crate::platform::spin_unlock(&mut self.lock);

        Ok(NonNull::new_unchecked(tx_buffer))
    }

    /// 提交事务
    pub unsafe fn commit(&mut self) -> Result<()> {
        // 自旋锁保护
        crate::platform::spin_lock(&mut self.lock);

        // 检查是否有活动事务
        let tx = match self.current_tx {
            Some(tx) => tx,
            None => {
                crate::platform::spin_unlock(&mut self.lock);
                return Err(RemDbError::TransactionError);
            }
        };

        // 保存当前事务指针并清除current_tx，确保无论后续操作是否成功，current_tx都被重置
        let mut tx_ptr = tx;
        self.current_tx = None;

        // 解锁以允许日志写入
        crate::platform::spin_unlock(&mut self.lock);

        // 提交事务日志项
        tx_ptr.as_mut().commit_log_item()?;

        // 重新获取锁来更新事务状态
        crate::platform::spin_lock(&mut self.lock);

        // 更新事务状态
        tx_ptr.as_mut().status = TransactionStatus::Committed;

        // 解锁
        crate::platform::spin_unlock(&mut self.lock);

        Ok(())
    }

    /// 回滚事务
    pub unsafe fn rollback(&mut self) -> Result<()> {
        // 自旋锁保护
        crate::platform::spin_lock(&mut self.lock);

        // 检查是否有活动事务
        let tx = match self.current_tx {
            Some(tx) => tx,
            None => {
                crate::platform::spin_unlock(&mut self.lock);
                return Err(RemDbError::TransactionError);
            }
        };

        // 保存当前事务指针并清除current_tx，确保无论后续操作是否成功，current_tx都被重置
        let mut tx_ptr = tx;
        self.current_tx = None;

        // 解锁以允许回滚操作
        crate::platform::spin_unlock(&mut self.lock);

        // 执行实际的回滚操作：遍历日志项，按相反顺序撤销操作
        for i in (0..tx_ptr.as_mut().log_item_count).rev() {
            // SAFETY: log_items pointer is valid and within bounds
            let log_item = unsafe { &*tx_ptr.as_mut().log_items.as_ptr().add(i) };

            // 获取数据库实例
            let db = crate::get_global_db().ok_or(RemDbError::InternalError)?;

            match log_item.header.op_type {
                LogOperation::Insert => {
                    // 撤销插入：执行删除操作
                    let table_id = log_item.header.table_id as usize;
                    if table_id < db.tables.len() {
                        if let Some(table) = &mut db.tables[table_id] {
                            // 检查记录是否存在
                            let status_ptr = table.get_status_ptr(log_item.header.record_id as usize);
                            if (*status_ptr).status == crate::types::RecordStatus::Used {
                                // 从主键索引中删除
                                if let Some(primary_index) = &mut db.primary_indices[table_id] {
                                    let record_ptr =
                                        table.get_record_ptr_mut(log_item.header.record_id as usize);
                                    // 使用复合键删除方法
                                    primary_index.delete_composite(record_ptr);
                                }

                                // 标记记录为空闲
                                (*status_ptr).status = crate::types::RecordStatus::Free;
                                (*status_ptr).version += 1;

                                // 清空记录数据
                                let record_ptr =
                                    table.get_record_ptr_mut(log_item.header.record_id as usize);
                                crate::platform::memset(record_ptr, 0, table.record_size);

                                // 将空闲槽压回栈中，确保不超过数组大小
                                if table.free_slot_count < table.def.max_records {
                                    *table.free_slots.as_ptr().add(table.free_slot_count) =
                                        log_item.header.record_id as usize;
                                    table.free_slot_count += 1;
                                }

                                // 更新记录计数
                                table.record_count -= 1;
                            }
                        }
                    }
                }
                LogOperation::Delete => {
                    // 撤销删除：执行插入操作，恢复旧数据
                    let table_id = log_item.header.table_id as usize;
                    if table_id < db.tables.len() {
                        if let Some(table) = &mut db.tables[table_id] {
                            // 检查记录是否空闲
                            let status_ptr = table.get_status_ptr(log_item.header.record_id as usize);
                            if (*status_ptr).status != crate::types::RecordStatus::Used {
                                // 恢复记录数据
                                let record_ptr =
                                    table.get_record_ptr_mut(log_item.header.record_id as usize);
                                crate::platform::memcpy(
                                    record_ptr,
                                    log_item.old_data.as_ptr(),
                                    core::cmp::min(log_item.old_data.len(), table.record_size),
                                );

                                // 标记记录为已使用
                                (*status_ptr).status = crate::types::RecordStatus::Used;
                                (*status_ptr).version += 1;
                                (*status_ptr).create_tx_id = log_item.header.tx_id;
                                (*status_ptr).delete_tx_id = 0;
                                (*status_ptr).next_version_ptr = 0;
                                table.record_count += 1;

                                // 从空闲槽栈中移除该槽位，确保不重复使用
                                if table.free_slot_count > 0 {
                                    let mut found = false;
                                    let mut i = 0;
                                    while i < table.free_slot_count {
                                        if *table.free_slots.as_ptr().add(i) == log_item.header.record_id as usize {
                                            // 找到，将最后一个元素移动到当前位置
                                            *table.free_slots.as_ptr().add(i) =
                                                *table.free_slots.as_ptr().add(table.free_slot_count - 1);
                                            table.free_slot_count -= 1;
                                            found = true;
                                            break;
                                        }
                                        i += 1;
                                    }
                                }

                                // 更新主键索引
                                if let Some(primary_index) = &mut db.primary_indices[table_id] {
                                    let _: Result<()> = primary_index.insert_composite(
                                        record_ptr,
                                        log_item.header.record_id as u16,
                                    );
                                }
                            }
                        }
                    }
                }
                LogOperation::Update => {
                    // 撤销更新：恢复旧数据
                    let table_id = log_item.header.table_id as usize;
                    if table_id < db.tables.len() {
                        if let Some(table) = &mut db.tables[table_id] {
                            // 检查记录是否存在
                            let status_ptr = table.get_status_ptr(log_item.header.record_id as usize);
                            if (*status_ptr).status == crate::types::RecordStatus::Used {
                                // 从主键索引中删除旧记录（当前更新后的数据）
                                if let Some(primary_index) = &mut db.primary_indices[table_id] {
                                    let record_ptr =
                                        table.get_record_ptr_mut(log_item.header.record_id as usize);
                                    let _: Result<()> = primary_index.delete_composite(record_ptr);
                                }

                                // 恢复旧数据
                                let record_ptr =
                                    table.get_record_ptr_mut(log_item.header.record_id as usize);
                                crate::platform::memcpy(
                                    record_ptr,
                                    log_item.old_data.as_ptr(),
                                    core::cmp::min(log_item.old_data.len(), table.record_size),
                                );

                                // 更新版本号和创建事务ID
                                (*status_ptr).version += 1;
                                (*status_ptr).create_tx_id = log_item.header.tx_id;

                                // 将恢复后的数据插入到主键索引中
                                if let Some(primary_index) = &mut db.primary_indices[table_id] {
                                    let _: Result<()> = primary_index.insert_composite(
                                        record_ptr,
                                        log_item.header.record_id as u16,
                                    );
                                }
                            }
                        }
                    }
                }
                _ => {
                    // 其他操作类型不需要回滚
                    continue;
                }
            }
        }

        // 更新事务状态为回滚中
        tx_ptr.as_mut().status = TransactionStatus::RolledBack;

        Ok(())
    }

    /// 检查记录是否对当前事务可见（MVCC实现）
    pub fn is_visible(&self, create_tx_id: u32, delete_tx_id: u32, current_tx_id: u32) -> bool {
        // MVCC可见性规则：
        // 1. 记录是由当前事务创建的，对当前事务可见
        // 2. 记录是由已提交事务创建的，并且未被删除或被当前事务删除
        // 3. 记录是由已提交事务创建的，并且删除事务尚未提交

        // 如果当前事务是创建者，记录可见
        if create_tx_id == current_tx_id {
            return true;
        }

        // 如果记录已被删除，并且删除事务已提交，记录不可见
        if delete_tx_id > 0 && delete_tx_id < current_tx_id {
            return false;
        }

        // 如果记录是由已提交事务创建的，并且未被删除，记录可见
        if create_tx_id < current_tx_id {
            return true;
        }

        // 其他情况，记录不可见
        false
    }

    /// 获取当前活动事务
    pub fn get_current_tx(&self) -> Option<NonNull<Transaction>> {
        self.current_tx
    }

    /// 设置低功耗模式
    pub fn set_low_power_mode(&mut self, enabled: bool) {
        self.low_power_mode = enabled;
    }

    /// 获取低功耗模式状态
    pub fn is_low_power_mode(&self) -> bool {
        self.low_power_mode
    }

    /// 重置事务管理器状态
    pub fn reset(&mut self) {
        // 先重置锁状态，防止上一次测试中锁未正确释放导致死锁
        unsafe {
            core::sync::atomic::AtomicU32::from_ptr(&mut self.lock as *mut u32)
                .store(0, core::sync::atomic::Ordering::Release);
        }
        
        // 自旋锁保护
        crate::platform::spin_lock(&mut self.lock);

        // 清除所有活动事务
        self.current_tx = None;
        self.active_snapshots.clear();

        // 重置事务计数
        self.tx_id_counter = 1; // 从1开始，避免与初始值0冲突
        self.snapshot_version = 0;

        // 解锁
        crate::platform::spin_unlock(&mut self.lock);
    }
}

impl Transaction {
    /// 提交事务日志项
    pub unsafe fn commit_log_item(&mut self) -> Result<()> {
        // 检查是否有活动事务
        if self.status != TransactionStatus::Active {
            return Err(RemDbError::TransactionError);
        }

        // 如果没有日志项，直接返回成功
        if self.log_item_count == 0 {
            return Ok(());
        }

        // 遍历所有日志项
        for i in 0..self.log_item_count {
            // SAFETY: log_items pointer is valid and within bounds
            let log_item = unsafe { &*self.log_items.as_ptr().add(i) };

            // 写入日志项到日志管理器
            if let Some(log_manager) = crate::transaction::TX_MANAGER.get_log_manager_mut() {
                // 创建可变大小的日志项并写入
                // 注意：系统已完全支持可变大小日志，begin_log_item不会截断数据
                let var_log_item = VariableSizeLogItem {
                    header: log_item.header,
                    old_data: log_item.old_data.clone(),
                    new_data: log_item.new_data.clone(),
                };
                log_manager.write_variable_size_log_item(&var_log_item)?;
            }
        }

        Ok(())
    }

    /// 开始事务日志项（可变大小版本）
    pub unsafe fn begin_variable_size_log_item(
        &mut self,
        tx_id: u32,
        op_type: LogOperation,
        table_id: u8,
        record_id: u16,
        old_data: Option<&[u8]>,
        new_data: Option<&[u8]>,
    ) -> VariableSizeLogItem {
        let old_data_size = old_data.map_or(0, |d| d.len() as u16);
        let new_data_size = new_data.map_or(0, |d| d.len() as u16);

        let header = LogItem {
            op_type,
            table_id,
            record_id,
            old_data_size,
            new_data_size,
            tx_id,
            timestamp: crate::platform::get_timestamp_us(),
            checksum: 0,
        };

        let old_data_vec = old_data.map_or(Vec::new(), |d| d.to_vec());
        let new_data_vec = new_data.map_or(Vec::new(), |d| d.to_vec());

        let mut var_log_item = VariableSizeLogItem {
            header,
            old_data: old_data_vec,
            new_data: new_data_vec,
        };

        // 计算校验和：直接基于字段计算，避免结构体填充问题
        let calculated_checksum = Transaction::calculate_variable_size_log_item_checksum(&var_log_item);
        var_log_item.header.checksum = calculated_checksum;

        var_log_item
    }

    /// 开始事务日志项（使用 VariableSizeLogItem 以支持数据存储）
    pub unsafe fn begin_log_item(
        &mut self,
        tx_id: u32,
        op_type: LogOperation,
        table_id: u8,
        record_id: u16,
        _data_size: u16,
        old_data: Option<&[u8]>,
        new_data: Option<&[u8]>,
    ) -> Option<NonNull<VariableSizeLogItem>> {
        if self.log_item_count >= self.max_log_items {
            return None;
        }
        let log_item_ptr = self.log_items.as_ptr().add(self.log_item_count);
        // SAFETY: log_item_ptr is valid and non-null
        let log_item = unsafe { log_item_ptr.as_mut() };
        
        // 初始化日志项头部
        log_item.header.op_type = op_type;
        log_item.header.table_id = table_id;
        log_item.header.record_id = record_id;
        log_item.header.old_data_size = old_data.map_or(0, |d| d.len() as u16);
        log_item.header.new_data_size = new_data.map_or(0, |d| d.len() as u16);
        log_item.header.tx_id = tx_id;
        log_item.header.timestamp = crate::platform::get_timestamp_us();
        log_item.header.checksum = 0;
        
        // 复制数据到日志项
        if let Some(data) = old_data {
            log_item.old_data = data.to_vec();
        } else {
            log_item.old_data = Vec::new();
        }
        
        if let Some(data) = new_data {
            log_item.new_data = data.to_vec();
        } else {
            log_item.new_data = Vec::new();
        }
        
        // 计算校验和
        let calculated_checksum = Transaction::calculate_variable_size_log_item_checksum(log_item);
        log_item.header.checksum = calculated_checksum;
        
        self.log_item_count += 1;
        Some(NonNull::new_unchecked(log_item_ptr))
    }

    /// 检查事务是否活跃
    pub fn is_active(&self) -> bool {
        self.status == TransactionStatus::Active
    }

    /// 检查事务是否为只读
    pub fn is_read_only(&self) -> bool {
        self.tx_type == TransactionType::ReadOnly
    }

    /// 计算数据校验和
    pub fn calculate_checksum(data: &[u8]) -> u32 {
        let mut checksum = 0u32;
        let mut i = 0;

        // 按4字节为单位计算校验和
        while i + 4 <= data.len() {
            let value = u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
            checksum ^= value;
            i += 4;
        }

        // 处理剩余的字节
        while i < data.len() {
            checksum ^= data[i] as u32;
            i += 1;
        }

        checksum
    }
}

/// 事务管理器全局实例
static mut TX_MANAGER: TransactionManager = TransactionManager::new();

/// 获取全局事务管理器
pub fn get_tx_manager() -> &'static mut TransactionManager {
    unsafe { &mut TX_MANAGER }
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
    unsafe { TX_MANAGER.get_log_manager_mut() }
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
    unsafe { TX_MANAGER.is_low_power_mode() }
}

/// 检查是否有活动事务
pub fn has_active_tx() -> bool {
    unsafe { TX_MANAGER.has_active_tx() }
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
    log_buffer: *mut VariableSizeLogItem,
    max_log_items: usize,
) -> Result<NonNull<Transaction>> {
    TX_MANAGER.begin(
        tx_type,
        isolation_level,
        tx_buffer,
        log_buffer,
        max_log_items,
    )
}

/// 简化的事务开始函数，用于SQL查询
pub unsafe fn begin_transaction() {
    // 使用Box::leak创建静态缓冲区，确保生命周期足够长
    static mut TX_BUFFER: Option<Box<Transaction>> = None;
    static mut LOG_BUFFER: Option<Box<[VariableSizeLogItem]>> = None;
    
    // 初始化缓冲区（仅第一次调用时）
    if TX_BUFFER.is_none() {
        TX_BUFFER = Some(Box::new(Transaction::default()));
    }
    if LOG_BUFFER.is_none() {
        LOG_BUFFER = Some(vec![VariableSizeLogItem::default(); 1024].into_boxed_slice());
    }
    
    // 默认使用读写事务和可重复读隔离级别
    let _ = TX_MANAGER.begin(
        TransactionType::ReadWrite,
        IsolationLevel::RepeatableRead,
        &mut **TX_BUFFER.as_mut().ok_or(RemDbError::InternalError("TX buffer not initialized"))?,
        LOG_BUFFER.as_mut().ok_or(RemDbError::InternalError("Log buffer not initialized"))?.as_mut_ptr(),
        1024,
    );
}

/// 提交事务
pub unsafe fn commit() -> Result<()> {
    TX_MANAGER.commit()
}

/// 简化的事务提交函数，用于SQL查询
pub unsafe fn commit_transaction() {
    let _ = TX_MANAGER.commit();
}

/// 回滚事务
pub unsafe fn rollback() -> Result<()> {
    TX_MANAGER.rollback()
}

/// 简化的事务回滚函数，用于SQL查询
pub unsafe fn rollback_transaction() {
    let _ = TX_MANAGER.rollback();
}

/// 检查记录是否对当前事务可见（MVCC实现）
pub fn is_visible(create_tx_id: u32, delete_tx_id: u32, current_tx_id: u32) -> bool {
    unsafe { TX_MANAGER.is_visible(create_tx_id, delete_tx_id, current_tx_id) }
}

/// 获取当前事务ID计数器
pub fn tx_id_counter() -> u32 {
    unsafe { TX_MANAGER.tx_id_counter() }
}
