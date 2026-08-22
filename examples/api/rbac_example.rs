//! RBAC (基于角色的访问控制) 示例
//!
//! 该示例展示如何使用 RemDB 的 RBAC 功能：
//! - 创建角色和用户
//! - 授予和撤销权限
//! - 分配角色给用户
//! - 权限检查

use remdb::config::{DbConfig, DefaultMemoryAllocator, WALConfig};
use remdb::rbac::Permission;
use remdb::{RemDb, Result};

static mut DB_MEMORY: [u8; 4 * 1024 * 1024] = [0; 4 * 1024 * 1024];

static ALLOCATOR: DefaultMemoryAllocator = DefaultMemoryAllocator;

fn main() -> Result<()> {
    unsafe {
        remdb::memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len())?;
    }

    let config = Box::leak(Box::new(DbConfig {
        tables: vec![],
        total_memory: 4 * 1024 * 1024,
        low_power_mode_supported: false,
        low_power_max_records: None,
        default_max_records: 1000,
        memory_allocator: &ALLOCATOR,
        wal_config: WALConfig {
            log_path: "./wal",
            log_mode: remdb::config::LogMode::Sync,
            checkpoint_interval_ms: 60000,
            log_file_size_limit: 16 * 1024 * 1024,
            log_prealloc_size: 1 * 1024 * 1024,
            log_segment_size: 16 * 1024 * 1024,
            retained_checkpoints: 3,
            max_consecutive_invalid: 100,
            skip_threshold: 1000,
            skip_block_size: 1024 * 1024,
            max_skip_attempts: 3,
            compression_type: remdb::config::WALCompressionType::None,
            compression_level: 3,
        },
        time_series_defaults: remdb::time_series::TimeSeriesConfig::DEFAULT,
        #[cfg(feature = "ha")]
        ha_config: None,
        #[cfg(feature = "pubsub")]
        pubsub_config: None,
    }));

    let mut db = RemDb::new(config);
    db.init()?;

    println!("=== RBAC (基于角色的访问控制) 示例 ===\n");

    // 1. 创建角色 (注意: "admin" 角色已默认存在)
    println!("1. 创建角色");
    // "admin" 角色已由系统默认创建，无需重复创建
    println!("   使用默认角色: admin");
    db.create_role("developer")?;
    println!("   创建角色: developer");
    db.create_role("readonly")?;
    println!("   创建角色: readonly");

    // 2. 创建用户
    println!("\n2. 创建用户");
    db.create_user("alice")?;
    println!("   创建用户: alice");
    db.create_user("bob")?;
    println!("   创建用户: bob");
    db.create_user("charlie")?;
    println!("   创建用户: charlie");

    // 3. 授予权限给角色
    println!("\n3. 授予权限给角色");
    
    // admin 角色拥有所有权限
    db.grant_permission("admin", Permission::Admin, None, None)?;
    println!("   授予 admin 角色管理员权限");
    
    // developer 角色拥有读写权限
    db.grant_permission("developer", Permission::Insert, None, None)?;
    db.grant_permission("developer", Permission::Select, None, None)?;
    db.grant_permission("developer", Permission::Update, None, None)?;
    db.grant_permission("developer", Permission::Delete, None, None)?;
    println!("   授予 developer 角色读写权限");
    
    // readonly 角色只有读取权限
    db.grant_permission("readonly", Permission::Select, None, None)?;
    println!("   授予 readonly 角色读取权限");

    // 4. 分配角色给用户
    println!("\n4. 分配角色给用户");
    db.grant_role("alice", "admin")?;
    println!("   分配 admin 角色给用户 alice");
    db.grant_role("bob", "developer")?;
    println!("   分配 developer 角色给用户 bob");
    db.grant_role("charlie", "readonly")?;
    println!("   分配 readonly 角色给用户 charlie");

    // 5. 检查权限
    println!("\n5. 检查权限");
    
    // alice (admin) 应该有所有权限
    let has_insert = db.check_permission("alice", &Permission::Insert, &None, &None)?;
    let has_select = db.check_permission("alice", &Permission::Select, &None, &None)?;
    let has_update = db.check_permission("alice", &Permission::Update, &None, &None)?;
    println!("   alice (admin): Insert={}, Select={}, Update={}", has_insert, has_select, has_update);
    
    // bob (developer) 应该有读写权限
    let has_insert = db.check_permission("bob", &Permission::Insert, &None, &None)?;
    let has_select = db.check_permission("bob", &Permission::Select, &None, &None)?;
    let has_admin = db.check_permission("bob", &Permission::Admin, &None, &None)?;
    println!("   bob (developer): Insert={}, Select={}, Admin={}", has_insert, has_select, has_admin);
    
    // charlie (readonly) 应该只有读取权限
    let has_select = db.check_permission("charlie", &Permission::Select, &None, &None)?;
    let has_delete = db.check_permission("charlie", &Permission::Delete, &None, &None)?;
    println!("   charlie (readonly): Select={}, Delete={}", has_select, has_delete);

    // 6. 撤销权限
    println!("\n6. 撤销权限");
    db.revoke_permission("developer", &Permission::Delete, &None, &None)?;
    println!("   撤销 developer 角色的删除权限");
    
    let has_delete = db.check_permission("bob", &Permission::Delete, &None, &None)?;
    println!("   bob 现在是否有删除权限: {}", has_delete);

    // 7. 撤销用户角色
    println!("\n7. 撤销用户角色");
    db.revoke_role("bob", "developer")?;
    println!("   撤销 bob 的 developer 角色");
    
    // bob 现在没有任何权限
    let has_select = db.check_permission("bob", &Permission::Select, &None, &None)?;
    println!("   bob 现在是否有读取权限: {}", has_select);

    // 8. 删除角色和用户
    println!("\n8. 删除角色和用户");
    db.drop_user("charlie")?;
    println!("   删除用户: charlie");
    db.drop_role("readonly")?;
    println!("   删除角色: readonly");

    // 9. 表级别权限
    println!("\n9. 表级别权限");
    
    // 先创建一个表
    db.sql_query("CREATE TABLE sensitive_data (id INT32 PRIMARY KEY, value TEXT)")?;
    println!("   创建表: sensitive_data");
    
    // 授予 developer 角色对特定表的读取权限
    db.grant_permission("developer", Permission::Select, Some("sensitive_data".to_string()), None)?;
    println!("   授予 developer 角色对 sensitive_data 表的读取权限");
    
    // 重新给 bob 分配 developer 角色
    db.grant_role("bob", "developer")?;
    
    let has_select = db.check_permission("bob", &Permission::Select, &Some("sensitive_data".to_string()), &None)?;
    println!("   bob 是否有读取 sensitive_data 表的权限: {}", has_select);

    println!("\n=== RBAC 示例完成 ===");
    Ok(())
}
