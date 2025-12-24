#![cfg_attr(not(feature = "std"), no_std)]

use std::ptr::NonNull;

// 导出公共API
pub mod types;
pub mod config;
pub mod table;
pub mod index;
pub mod transaction;
pub mod memory;
pub mod platform;

// 导出核心类型
pub use types::{DataType, FieldDef, TableDef, Value, Result, RemDbError};
pub use table::MemoryTable;
pub use index::{PrimaryIndex, SecondaryIndex, IndexStats};
pub use transaction::{Transaction, TransactionType, TransactionManager};

/// 数据库实例
pub struct RemDb {
    /// 数据库配置
    pub config: &'static config::DbConfig,
    /// 内存表数组
    tables: &'static mut [Option<MemoryTable>],
    /// 主键索引数组
    primary_indices: &'static mut [Option<PrimaryIndex>],
    /// 辅助索引数组
    secondary_indices: &'static mut [Option<SecondaryIndex>],
}

// 为RemDb实现Send和Sync trait
// 注意：这是安全的，因为RemDb的所有字段都是线程安全的
unsafe impl Send for RemDb {}
unsafe impl Sync for RemDb {}

impl RemDb {
    /// 快照魔数
    const SNAPSHOT_MAGIC: u32 = 0x52454D44; // 'REMD'
    /// 快照版本
    const SNAPSHOT_VERSION: u32 = 1;
    
    /// 创建新的数据库实例
    pub fn new(
        config: &'static config::DbConfig,
        tables: &'static mut [Option<MemoryTable>],
        primary_indices: &'static mut [Option<PrimaryIndex>],
        secondary_indices: &'static mut [Option<SecondaryIndex>]
    ) -> Self {
        RemDb {
            config,
            tables,
            primary_indices,
            secondary_indices,
        }
    }
    
    /// 获取表
    pub fn get_table(&self, table_id: usize) -> Result<&MemoryTable> {
        if table_id >= self.tables.len() {
            return Err(RemDbError::RecordNotFound);
        }
        
        match &self.tables[table_id] {
            Some(table) => Ok(table),
            None => Err(RemDbError::RecordNotFound),
        }
    }
    
    /// 获取表（可变）
    pub fn get_table_mut(&mut self, table_id: usize) -> Result<&mut MemoryTable> {
        if table_id >= self.tables.len() {
            return Err(RemDbError::RecordNotFound);
        }
        
        match &mut self.tables[table_id] {
            Some(table) => Ok(table),
            None => Err(RemDbError::RecordNotFound),
        }
    }
    
    /// 获取主键索引
    pub fn get_primary_index(&self, table_id: usize) -> Result<&PrimaryIndex> {
        if table_id >= self.primary_indices.len() {
            return Err(RemDbError::RecordNotFound);
        }
        
        match &self.primary_indices[table_id] {
            Some(index) => Ok(index),
            None => Err(RemDbError::RecordNotFound),
        }
    }
    
    /// 获取主键索引（可变）
    pub fn get_primary_index_mut(&mut self, table_id: usize) -> Result<&mut PrimaryIndex> {
        if table_id >= self.primary_indices.len() {
            return Err(RemDbError::RecordNotFound);
        }
        
        match &mut self.primary_indices[table_id] {
            Some(index) => Ok(index),
            None => Err(RemDbError::RecordNotFound),
        }
    }
    
    /// 获取辅助索引
    pub fn get_secondary_index(&self, table_id: usize) -> Result<&SecondaryIndex> {
        if table_id >= self.secondary_indices.len() {
            return Err(RemDbError::RecordNotFound);
        }
        
        match &self.secondary_indices[table_id] {
            Some(index) => Ok(index),
            None => Err(RemDbError::RecordNotFound),
        }
    }
    
    /// 获取辅助索引（可变）
    pub fn get_secondary_index_mut(&mut self, table_id: usize) -> Result<&mut SecondaryIndex> {
        if table_id >= self.secondary_indices.len() {
            return Err(RemDbError::RecordNotFound);
        }
        
        match &mut self.secondary_indices[table_id] {
            Some(index) => Ok(index),
            None => Err(RemDbError::RecordNotFound),
        }
    }
    
    /// 获取表数量
    pub fn table_count(&self) -> usize {
        self.config.tables.len()
    }
    
    /// 开始事务
    pub unsafe fn begin_transaction(
        &mut self,
        tx_type: transaction::TransactionType,
        isolation_level: transaction::IsolationLevel,
        tx_buffer: *mut transaction::Transaction,
        log_buffer: *mut transaction::LogItem,
        max_log_items: usize
    ) -> Result<NonNull<transaction::Transaction>> {
        crate::transaction::TX_MANAGER.begin(tx_type, isolation_level, tx_buffer, log_buffer, max_log_items)
    }
    
    /// 提交事务
    pub unsafe fn commit_transaction(&mut self) -> Result<()> {
        crate::transaction::TX_MANAGER.commit()
    }
    
    /// 回滚事务
    pub unsafe fn rollback_transaction(&mut self) -> Result<()> {
        crate::transaction::TX_MANAGER.rollback(self)
    }
    
    /// 初始化数据库
    pub fn init(&mut self) -> Result<()> {
        // 直接初始化平台抽象层，不检查当前状态
        // 默认使用POSIX平台（如果可用）
        #[cfg(feature = "posix")]
        crate::platform::init_platform(crate::platform::posix::get_posix_platform());
        
        Ok(())
    }
    
    /// 保存快照到文件
    pub fn save_snapshot(&self, path: &str) -> Result<()> {
        // 打开文件 - 使用Write模式
        let handle = crate::platform::file_open(path, crate::platform::FileMode::Write)
            .map_err(|_| RemDbError::FileIoError)?;
        
        // 使用defer确保文件关闭
        let _defer = Defer::new(|| {
            let _ = crate::platform::file_close(handle);
        });
        
        // 写入魔数
        let magic = Self::SNAPSHOT_MAGIC.to_le_bytes();
        let written = crate::platform::file_write(handle, magic.as_ptr(), magic.len())
            .map_err(|_| RemDbError::FileIoError)?;
        if written != magic.len() {
            return Err(RemDbError::FileIoError);
        }
        
        // 写入版本号
        let version = Self::SNAPSHOT_VERSION.to_le_bytes();
        let written = crate::platform::file_write(handle, version.as_ptr(), version.len())
            .map_err(|_| RemDbError::FileIoError)?;
        if written != version.len() {
            return Err(RemDbError::FileIoError);
        }
        
        // 写入表数量
        let table_count = self.config.tables.len() as u32;
        let table_count_bytes = table_count.to_le_bytes();
        let written = crate::platform::file_write(handle, table_count_bytes.as_ptr(), table_count_bytes.len())
            .map_err(|_| RemDbError::FileIoError)?;
        if written != table_count_bytes.len() {
            return Err(RemDbError::FileIoError);
        }
        
        // 写入每个表的数据
        for table_id in 0..table_count as usize {
            if let Some(table) = &self.tables[table_id] {
                // 写入表ID（4字节）
                let table_id_u32 = table_id as u32;
                let table_id_bytes = table_id_u32.to_le_bytes();
                let written = crate::platform::file_write(handle, table_id_bytes.as_ptr(), table_id_bytes.len())
                    .map_err(|_| RemDbError::FileIoError)?;
                if written != table_id_bytes.len() {
                    return Err(RemDbError::FileIoError);
                }
                
                // 写入已使用的记录数（4字节）
                let used_count_u32 = table.record_count() as u32;
                let used_count_bytes = used_count_u32.to_le_bytes();
                let written = crate::platform::file_write(handle, used_count_bytes.as_ptr(), used_count_bytes.len())
                    .map_err(|_| RemDbError::FileIoError)?;
                if written != used_count_bytes.len() {
                    return Err(RemDbError::FileIoError);
                }
                
                // 动态计算记录大小
                let mut record_size = 0;
                for field in table.def.fields {
                    record_size += field.size;
                }
                
                // 写入已使用的记录
                for i in 0..table.def.max_records {
                    let status_ptr = unsafe { table.get_status_ptr(i) };
                    if unsafe { (*status_ptr).status } == crate::types::RecordStatus::Used {
                        // 写入记录索引（4字节）
                        let index_u32 = i as u32;
                        let index_bytes = index_u32.to_le_bytes();
                        let written = crate::platform::file_write(handle, index_bytes.as_ptr(), index_bytes.len())
                            .map_err(|_| RemDbError::FileIoError)?;
                        if written != index_bytes.len() {
                            return Err(RemDbError::FileIoError);
                        }
                        
                        // 写入记录数据
                        let record_ptr = unsafe { table.get_record_ptr(i) };
                        let written = crate::platform::file_write(handle, record_ptr, record_size)
                            .map_err(|_| RemDbError::FileIoError)?;
                        if written != record_size {
                            return Err(RemDbError::FileIoError);
                        }
                    }
                }
            }
        }
        
        // 跳过CRC32计算和写入，简化实现
        Ok(())
    }
    
    /// 从文件恢复快照
    pub fn restore_snapshot(&mut self, path: &str) -> Result<()> {
        // 打开文件
        let handle = crate::platform::file_open(path, crate::platform::FileMode::Read)
            .map_err(|_| RemDbError::FileIoError)?;
        
        // 使用defer确保文件关闭
        let _defer = Defer::new(|| {
            let _ = crate::platform::file_close(handle);
        });
        
        // 读取魔数
        let mut magic_bytes = [0u8; 4];
        let read = crate::platform::file_read(handle, magic_bytes.as_mut_ptr(), magic_bytes.len())
            .map_err(|_| RemDbError::FileIoError)?;
        if read != magic_bytes.len() {
            return Err(RemDbError::FileIoError);
        }
        let magic = u32::from_le_bytes(magic_bytes);
        if magic != Self::SNAPSHOT_MAGIC {
            return Err(RemDbError::SnapshotFormatError);
        }
        
        // 读取版本号
        let mut version_bytes = [0u8; 4];
        let read = crate::platform::file_read(handle, version_bytes.as_mut_ptr(), version_bytes.len())
            .map_err(|_| RemDbError::FileIoError)?;
        if read != version_bytes.len() {
            return Err(RemDbError::FileIoError);
        }
        let version = u32::from_le_bytes(version_bytes);
        if version != Self::SNAPSHOT_VERSION {
            return Err(RemDbError::SnapshotFormatError);
        }
        
        // 读取表数量
        let mut table_count_bytes = [0u8; 4];
        let read = crate::platform::file_read(handle, table_count_bytes.as_mut_ptr(), table_count_bytes.len())
            .map_err(|_| RemDbError::FileIoError)?;
        if read != table_count_bytes.len() {
            return Err(RemDbError::FileIoError);
        }
        let table_count = u32::from_le_bytes(table_count_bytes) as usize;
        
        // 检查表数量是否匹配
        if table_count != self.config.tables.len() {
            return Err(RemDbError::SnapshotFormatError);
        }
        
        // 读取每个表的数据
        for _ in 0..table_count {
            // 读取表ID（4字节）
            let mut table_id_bytes = [0u8; 4];
            let read = crate::platform::file_read(handle, table_id_bytes.as_mut_ptr(), table_id_bytes.len())
                .map_err(|_| RemDbError::FileIoError)?;
            if read != table_id_bytes.len() {
                return Err(RemDbError::FileIoError);
            }
            let table_id = u32::from_le_bytes(table_id_bytes) as usize;
            
            // 检查表ID是否有效
            if table_id >= self.tables.len() {
                return Err(RemDbError::SnapshotFormatError);
            }
            
            // 获取表引用
            let table = match &mut self.tables[table_id] {
                Some(table) => table,
                None => return Err(RemDbError::SnapshotFormatError),
            };
            
            // 重置所有记录的状态和数据
            for i in 0..table.def.max_records {
                let status_ptr = unsafe { table.get_status_ptr(i) };
                let record_ptr = unsafe { table.get_record_ptr_mut(i) };
                
                unsafe {
                    (*status_ptr).status = crate::types::RecordStatus::Free;
                    (*status_ptr).version += 1;
                    crate::platform::memset(record_ptr, 0, table.def.record_size);
                }
            }
            
            // 重置记录数
            unsafe {
                table.set_record_count(0);
            }
            
            // 读取已使用的记录数（4字节）
            let mut used_count_bytes = [0u8; 4];
            let read = crate::platform::file_read(handle, used_count_bytes.as_mut_ptr(), used_count_bytes.len())
                .map_err(|_| RemDbError::FileIoError)?;
            if read != used_count_bytes.len() {
                return Err(RemDbError::FileIoError);
            }
            let used_count = u32::from_le_bytes(used_count_bytes) as usize;
            
            // 动态计算记录大小
            let mut record_size = 0;
            for field in table.def.fields {
                record_size += field.size;
            }
            
            // 读取已使用的记录
            for _ in 0..used_count {
                // 读取记录索引（4字节）
                let mut index_bytes = [0u8; 4];
                let read = crate::platform::file_read(handle, index_bytes.as_mut_ptr(), index_bytes.len())
                    .map_err(|_| RemDbError::FileIoError)?;
                if read != index_bytes.len() {
                    return Err(RemDbError::FileIoError);
                }
                let i = u32::from_le_bytes(index_bytes) as usize;
                
                // 检查索引是否有效
                if i >= table.def.max_records {
                    return Err(RemDbError::SnapshotFormatError);
                }
                
                // 读取记录数据
                let record_ptr = unsafe { table.get_record_ptr_mut(i) };
                let read = crate::platform::file_read(handle, record_ptr, record_size)
                    .map_err(|_| RemDbError::FileIoError)?;
                if read != record_size {
                    return Err(RemDbError::FileIoError);
                }
                
                // 更新记录状态
                let status_ptr = unsafe { table.get_status_ptr(i) };
                unsafe {
                    (*status_ptr).status = crate::types::RecordStatus::Used;
                    (*status_ptr).version += 1;
                }
                unsafe {
                    table.inc_record_count();
                }
            }
        }
        
        // 简化实现，跳过CRC32校验
        Ok(())
    }
}

/// 延迟执行结构体（用于确保资源释放）
struct Defer<F: FnMut()>(Option<F>);

impl<F: FnMut()> Defer<F> {
    /// 创建新的延迟执行实例
    pub fn new(f: F) -> Self {
        Defer(Some(f))
    }
}

impl<F: FnMut()> Drop for Defer<F> {
    fn drop(&mut self) {
        if let Some(mut f) = self.0.take() {
            f();
        }
    }
}

use std::sync::OnceLock;

/// 全局数据库实例 - 使用静态可变变量存储
static mut DB_INSTANCE: Option<RemDb> = None;

/// 初始化数据库全局实例
/// 注意：这是一个简化的实现，实际应用中应该根据需要创建数据库实例
pub fn init_global_db(
    config: &'static config::DbConfig,
    tables: &'static mut [Option<MemoryTable>],
    primary_indices: &'static mut [Option<PrimaryIndex>],
    secondary_indices: &'static mut [Option<SecondaryIndex>]
) -> Result<&'static mut RemDb> {
    unsafe {
        if DB_INSTANCE.is_some() {
            return Err(RemDbError::ConfigError);
        }
        
        let mut db = RemDb::new(config, tables, primary_indices, secondary_indices);
        db.init()?;
        DB_INSTANCE = Some(db);
        
        Ok(DB_INSTANCE.as_mut().unwrap())
    }
}

/// 获取全局数据库实例
pub fn get_global_db() -> Option<&'static mut RemDb> {
    unsafe {
        DB_INSTANCE.as_mut()
    }
}

// 导出C接口（可选）
#[cfg(feature = "c-api")]
extern "C" {
    // C API 声明
}

// Panic handler for no_std environments
#[cfg(not(feature = "std"))]
#[panic_handler]
fn panic_handler(_info: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
