// 嵌入式高可用主从复制模块

pub mod heartbeat;
pub mod manager;
pub mod protocol;
pub mod replication;
pub mod role;
pub mod sync_handler;
pub mod sync_receiver;

use core::fmt;

#[cfg(feature = "log")]
use crate::log::{debug, error, info, warn};

// HA角色
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum HARole {
    /// 主节点
    Master,
    /// 从节点
    Slave,
    /// 自动模式（通过集群协商确定角色）
    Auto,
}

// 复制模式
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum ReplicationMode {
    /// 同步模式：等待至少一个从节点确认后才返回
    Sync,
    /// 异步模式：立即返回，异步复制
    Async,
}

/// Sync state enumeration
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum SyncState {
    /// Idle, no sync in progress
    Idle = 0,
    /// Sync in progress
    Syncing = 1,
    /// Sync completed successfully
    Synced = 2,
    /// Sync failed
    Failed = 3,
}

impl From<u32> for SyncState {
    fn from(value: u32) -> Self {
        match value {
            0 => SyncState::Idle,
            1 => SyncState::Syncing,
            2 => SyncState::Synced,
            3 => SyncState::Failed,
            _ => SyncState::Idle,
        }
    }
}

/// HA配置结构体
#[derive(Copy, Clone, Debug)]
pub struct HAConfig {
    /// 节点ID，唯一标识集群中的节点
    pub node_id: u32,
    /// HA角色
    pub ha_role: HARole,
    /// 复制模式
    pub replication_mode: ReplicationMode,
    /// 心跳间隔（毫秒）
    pub heartbeat_interval_ms: u64,
    /// 故障检测时间（毫秒）
    pub failure_detection_ms: u64,
    /// 同步超时时间（毫秒）
    pub sync_timeout_ms: u64,
    /// 主节点地址（从节点使用）
    pub master_address: Option<&'static str>,
    /// 主节点端口（从节点使用）
    pub master_port: Option<u16>,
    /// 复制端口（用于WAL日志复制和数据同步）
    pub replication_port: u16,
}

impl Default for HAConfig {
    fn default() -> Self {
        HAConfig {
            node_id: 1,
            ha_role: HARole::Auto,
            replication_mode: ReplicationMode::Async,
            heartbeat_interval_ms: 1000,
            failure_detection_ms: 3000,
            sync_timeout_ms: 2000,
            master_address: None,
            master_port: None,
            replication_port: 5556,
        }
    }
}

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
        // 使用原始指针检查是否已初始化
        let ha_manager_ptr = core::ptr::addr_of_mut!(HA_MANAGER);
        if (*ha_manager_ptr).is_some() {
            return Err(HAError::InitFailed);
        }

        // 如果HA配置为None，跳过HA初始化
        if config.ha_config.is_none() {
            return Ok(());
        }

        #[cfg(feature = "log")]
        debug!(
            "ha::init: Creating HAManager instance"
        );
        let mut ha_manager = manager::HAManager::new(config)?;
        #[cfg(feature = "log")]
        debug!(
            "ha::init: Calling ha_manager.init()"
        );
        ha_manager.init()?;
        #[cfg(feature = "log")]
        debug!(
            "ha::init: Storing HAManager to global static"
        );
        *ha_manager_ptr = Some(ha_manager);

        #[cfg(feature = "log")]
        debug!(
            "ha::init: HA initialization completed"
        );

        Ok(())
    }
}

/// 获取全局HA管理器
pub fn get_ha_manager() -> Option<&'static mut manager::HAManager> {
    unsafe {
        // 使用原始指针获取可变引用
        let ha_manager_ptr = core::ptr::addr_of_mut!(HA_MANAGER);
        (*ha_manager_ptr).as_mut()
    }
}

/// 关闭全局HA管理器
pub fn shutdown() -> Result<()> {
    unsafe {
        let ha_manager_ptr = core::ptr::addr_of_mut!(HA_MANAGER);
        if let Some(ref mut manager) = *ha_manager_ptr {
            manager.shutdown()?;
            *ha_manager_ptr = None;
        }
        Ok(())
    }
}

/// 获取当前HA角色
pub fn get_role() -> Result<HARole> {
    unsafe {
        let ha_manager_ptr = core::ptr::addr_of!(HA_MANAGER);
        if let Some(manager) = &*ha_manager_ptr {
            Ok(manager.get_role())
        } else {
            Err(HAError::InitFailed)
        }
    }
}

/// 获取当前复制模式
pub fn get_replication_mode() -> Result<ReplicationMode> {
    unsafe {
        let ha_manager_ptr = core::ptr::addr_of!(HA_MANAGER);
        if let Some(manager) = &*ha_manager_ptr {
            Ok(manager.get_replication_mode())
        } else {
            Err(HAError::InitFailed)
        }
    }
}

/// 提升为Master节点
pub fn promote_to_master() -> Result<()> {
    unsafe {
        let ha_manager_ptr = core::ptr::addr_of_mut!(HA_MANAGER);
        if let Some(manager) = (*ha_manager_ptr).as_mut() {
            manager.promote_to_master()
        } else {
            Err(HAError::InitFailed)
        }
    }
}

/// 降级为Slave节点
pub fn demote_to_slave() -> Result<()> {
    unsafe {
        let ha_manager_ptr = core::ptr::addr_of_mut!(HA_MANAGER);
        if let Some(manager) = (*ha_manager_ptr).as_mut() {
            manager.demote_to_slave()
        } else {
            Err(HAError::InitFailed)
        }
    }
}

/// 检查HA状态
pub fn check_status() -> Result<()> {
    unsafe {
        let ha_manager_ptr = core::ptr::addr_of!(HA_MANAGER);
        if let Some(manager) = &*ha_manager_ptr {
            manager.check_status()
        } else {
            Err(HAError::InitFailed)
        }
    }
}
