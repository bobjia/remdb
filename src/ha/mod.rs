// 嵌入式高可用主从复制模块

pub mod manager;
pub mod replication;
pub mod heartbeat;
pub mod role;

use core::fmt;
use alloc::vec::Vec;
use parking_lot::Mutex;
use std::sync::OnceLock;

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
static HA_MANAGER: OnceLock<Mutex<Option<manager::HAManager>>> = OnceLock::new();

/// 通过回调访问全局HA管理器
pub fn with_ha_manager<F, R>(f: F) -> R
where
    F: FnOnce(Option<&mut manager::HAManager>) -> R,
{
    let mut guard = HA_MANAGER.get_or_init(|| Mutex::new(None)).lock();
    f(guard.as_mut())
}

/// 初始化全局HA管理器
pub fn init(config: &'static crate::config::DbConfig) -> Result<()> {
    let mut guard = HA_MANAGER.get_or_init(|| Mutex::new(None)).lock();
    if guard.is_some() {
        return Err(HAError::InitFailed);
    }

    #[cfg(feature = "std")]
    println!("[DEBUG] {}:{}: ha::init: Creating HAManager instance", file!(), line!());
    let mut ha_manager = manager::HAManager::new(config)?;

    #[cfg(feature = "std")]
    println!("[DEBUG] {}:{}: ha::init: Calling ha_manager.init()", file!(), line!());
    ha_manager.init()?;

    #[cfg(feature = "std")]
    println!("[DEBUG] {}:{}: ha::init: Storing HAManager to global static", file!(), line!());
    *guard = Some(ha_manager);

    #[cfg(feature = "std")]
    println!("[DEBUG] {}:{}: ha::init: HA initialization completed", file!(), line!());

    Ok(())
}

/// 关闭全局HA管理器
pub fn shutdown() -> Result<()> {
    let mut guard = HA_MANAGER.get_or_init(|| Mutex::new(None)).lock();
    if let Some(ref mut manager) = *guard {
        manager.shutdown()?;
    }
    *guard = None;
    Ok(())
}

/// 获取当前HA角色
pub fn get_role() -> Result<HARole> {
    with_ha_manager(|ha_manager| {
        if let Some(manager) = ha_manager {
            Ok(manager.get_role())
        } else {
            Err(HAError::InitFailed)
        }
    })
}

/// 获取当前复制模式
pub fn get_replication_mode() -> Result<ReplicationMode> {
    with_ha_manager(|ha_manager| {
        if let Some(manager) = ha_manager {
            Ok(manager.get_replication_mode())
        } else {
            Err(HAError::InitFailed)
        }
    })
}

/// 提升为Master节点
pub fn promote_to_master() -> Result<()> {
    with_ha_manager(|ha_manager| {
        if let Some(manager) = ha_manager {
            manager.promote_to_master()
        } else {
            Err(HAError::InitFailed)
        }
    })
}

/// 降级为Slave节点
pub fn demote_to_slave() -> Result<()> {
    with_ha_manager(|ha_manager| {
        if let Some(manager) = ha_manager {
            manager.demote_to_slave()
        } else {
            Err(HAError::InitFailed)
        }
    })
}

/// 检查HA状态
pub fn check_status() -> Result<()> {
    with_ha_manager(|ha_manager| {
        if let Some(manager) = ha_manager {
            manager.check_status()
        } else {
            Err(HAError::InitFailed)
        }
    })
}
