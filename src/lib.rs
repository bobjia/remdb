#![cfg_attr(not(feature = "std"), no_std)]

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
    /// 事务管理器
    pub tx_manager: TransactionManager,
}

impl RemDb {
    /// 创建新的数据库实例
    pub unsafe fn new(
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
            tx_manager: TransactionManager::new(),
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
    
    /// 初始化数据库
    pub unsafe fn init(&mut self) -> Result<()> {
        // 初始化平台抽象层（如果未初始化）
        if crate::platform::PLATFORM.is_none() {
            // 默认使用POSIX平台（如果可用）
            #[cfg(feature = "posix")]
            crate::platform::init_platform(crate::platform::posix::get_posix_platform());
        }
        
        Ok(())
    }
}

/// 初始化数据库全局实例
/// 注意：这是一个简化的实现，实际应用中应该根据需要创建数据库实例
pub unsafe fn init_global_db(
    config: &'static config::DbConfig,
    tables: &'static mut [Option<MemoryTable>],
    primary_indices: &'static mut [Option<PrimaryIndex>],
    secondary_indices: &'static mut [Option<SecondaryIndex>]
) -> Result<&'static mut RemDb> {
    static mut DB_INSTANCE: Option<RemDb> = None;
    
    if DB_INSTANCE.is_some() {
        return Err(RemDbError::ConfigError);
    }
    
    DB_INSTANCE = Some(RemDb::new(config, tables, primary_indices, secondary_indices));
    
    let db = DB_INSTANCE.as_mut().unwrap();
    db.init()?;
    
    Ok(db)
}

/// 获取全局数据库实例
pub fn get_global_db() -> Option<&'static mut RemDb> {
    unsafe {
        static mut DB_INSTANCE: Option<RemDb> = None;
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
