// 嵌入式高可用主从复制模块

pub mod manager;
pub mod replication;
pub mod heartbeat;
pub mod role;

use core::fmt;
use alloc::vec::Vec;

// HA相关错误类型
#[derive(Debug, PartialEq, Eq)]
pub enum HAError {
    // 初始化错误
    InitFailed,
    // 网络错误
    NetworkError,
    // 无效参数
    InvalidParameter,
    // 角色冲突
    RoleConflict,
    // 同步失败
    SyncFailed,
    // 心跳超时
    HeartbeatTimeout,
    // 复制错误
    ReplicationError,
    // 不支持的操作
    UnsupportedOperation,
}

impl fmt::Display for HAError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HAError::InitFailed => write!(f, "HA initialization failed"),
            HAError::NetworkError => write!(f, "Network error"),
            HAError::InvalidParameter => write!(f, "Invalid parameter"),
            HAError::RoleConflict => write!(f, "Role conflict"),
            HAError::SyncFailed => write!(f, "Sync failed"),
            HAError::HeartbeatTimeout => write!(f, "Heartbeat timeout"),
            HAError::ReplicationError => write!(f, "Replication error"),
            HAError::UnsupportedOperation => write!(f, "Unsupported operation"),
        }
    }
}

// HA结果类型
pub type Result<T> = core::result::Result<T, HAError>;

// 全局HA管理器实例
static mut HA_MANAGER: Option<manager::HAManager> = None;

/// 初始化全局HA管理器
pub fn init(config: &'static crate::config::DbConfig) -> Result<()> {
    unsafe {
        if HA_MANAGER.is_some() {
            return Err(HAError::InitFailed);
        }
        
        let ha_manager = manager::HAManager::new(config)?;
        ha_manager.init()?;
        HA_MANAGER = Some(ha_manager);
        
        Ok(())
    }
}

/// 获取全局HA管理器
pub fn get_ha_manager() -> Option<&'static mut manager::HAManager> {
    unsafe {
        HA_MANAGER.as_mut()
    }
}

/// 关闭全局HA管理器
pub fn shutdown() -> Result<()> {
    unsafe {
        if let Some(ref mut manager) = HA_MANAGER {
            manager.shutdown()?;
            HA_MANAGER = None;
        }
        Ok(())
    }
}
