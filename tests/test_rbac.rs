//! Test cases for RBAC functionality

use remdb::config::{DbConfig, DefaultMemoryAllocator, LogMode, WALCompressionType, WALConfig};
use remdb::platform;
use remdb::rbac::{Permission, RbacManager};
use remdb::RemDb;

#[test]
fn test_rbac_default_root_user() {
    println!("=== Testing RBAC Default Root User ===");

    // Create a new RBAC manager
    let rbac_manager = RbacManager::new();

    // Test that root user exists by default
    println!("\n1. Testing root user exists by default");
    assert!(
        rbac_manager.get_user("root").is_some(),
        "Root user should exist by default"
    );
    println!("✓ Root user exists");

    // Test that admin role exists by default
    println!("\n2. Testing admin role exists by default");
    assert!(
        rbac_manager.get_role("admin").is_some(),
        "Admin role should exist by default"
    );
    println!("✓ Admin role exists");

    // Test that root user has admin role
    println!("\n3. Testing root user has admin role");
    let root_user = rbac_manager.get_user("root").unwrap();
    assert!(
        root_user.has_role("admin"),
        "Root user should have admin role"
    );
    println!("✓ Root user has admin role");

    // Test that admin role has all permissions
    println!("\n4. Testing admin role has all permissions");
    let admin_role = rbac_manager.get_role("admin").unwrap();

    let all_permissions = vec![
        Permission::Admin,
        Permission::Select,
        Permission::Insert,
        Permission::Update,
        Permission::Delete,
        Permission::Create,
        Permission::Drop,
    ];

    for permission in &all_permissions {
        assert!(
            admin_role.has_permission(permission, &None, &None),
            "Admin role should have {:?} permission",
            permission
        );
        println!("✓ Admin role has {:?} permission", permission);
    }

    // Test that root user has all permissions through admin role
    println!("\n5. Testing root user has all permissions");
    for permission in &all_permissions {
        let has_perm = rbac_manager
            .check_permission("root", permission, &None, &None)
            .unwrap();
        assert!(
            has_perm,
            "Root user should have {:?} permission",
            permission
        );
        println!("✓ Root user has {:?} permission", permission);
    }

    println!("\n=== RBAC Default Root User Test Complete ===");
}

#[test]
fn test_rbac_basic_operations() {
    println!("=== Testing RBAC Basic Operations ===");

    // Create a new RBAC manager
    let mut rbac_manager = RbacManager::new();

    // Test creating a role (skip admin since it exists by default)
    println!("\n1. Testing role creation");
    match rbac_manager.create_role("user".to_string()) {
        Ok(_) => println!("✓ Successfully created role: user"),
        Err(e) => println!("⚠ Role creation failed (may already exist): {:?}", e),
    }

    // Test granting permissions
    println!("\n2. Testing permission granting");
    match rbac_manager.grant_permission("user", Permission::Select, Some("users".to_string()), None)
    {
        Ok(_) => println!("✓ Successfully granted SELECT permission to user"),
        Err(e) => panic!("✗ Failed to grant permission: {:?}", e),
    }

    // Test creating a user
    println!("\n3. Testing user creation");
    match rbac_manager.create_user("alice".to_string()) {
        Ok(_) => println!("✓ Successfully created user: alice"),
        Err(e) => panic!("✗ Failed to create user: {:?}", e),
    }

    // Test granting role to user
    println!("\n4. Testing role granting");
    match rbac_manager.grant_role("alice", "user") {
        Ok(_) => println!("✓ Successfully granted user role to alice"),
        Err(e) => panic!("✗ Failed to grant role: {:?}", e),
    }

    // Test checking permissions
    println!("\n5. Testing permission checking");
    match rbac_manager.check_permission(
        "alice",
        &Permission::Select,
        &Some("users".to_string()),
        &None,
    ) {
        Ok(has_perm) => {
            println!("✓ User alice has SELECT permission: {}", has_perm);
            assert!(has_perm);
        }
        Err(e) => panic!("✗ Failed to check permission: {:?}", e),
    }

    match rbac_manager.check_permission(
        "alice",
        &Permission::Insert,
        &Some("users".to_string()),
        &None,
    ) {
        Ok(has_perm) => {
            println!("✓ User alice has INSERT permission: {}", has_perm);
            assert!(!has_perm);
        }
        Err(e) => panic!("✗ Failed to check permission: {:?}", e),
    }

    match rbac_manager.check_permission(
        "alice",
        &Permission::Delete,
        &Some("users".to_string()),
        &None,
    ) {
        Ok(has_perm) => {
            println!("✓ User alice has DELETE permission: {}", has_perm);
            assert!(!has_perm);
        }
        Err(e) => panic!("✗ Failed to check permission: {:?}", e),
    }

    // Test revoking permissions
    println!("\n6. Testing permission revocation");
    match rbac_manager.revoke_permission(
        "user",
        &Permission::Select,
        &Some("users".to_string()),
        &None,
    ) {
        Ok(_) => println!("✓ Successfully revoked SELECT permission from user"),
        Err(e) => panic!("✗ Failed to revoke permission: {:?}", e),
    }

    // Test checking permissions after revocation
    match rbac_manager.check_permission(
        "alice",
        &Permission::Select,
        &Some("users".to_string()),
        &None,
    ) {
        Ok(has_perm) => {
            println!(
                "✓ User alice has SELECT permission after revocation: {}",
                has_perm
            );
            assert!(!has_perm);
        }
        Err(e) => panic!("✗ Failed to check permission: {:?}", e),
    }

    // Test revoking role
    println!("\n7. Testing role revocation");
    match rbac_manager.revoke_role("alice", "user") {
        Ok(_) => println!("✓ Successfully revoked user role from alice"),
        Err(e) => panic!("✗ Failed to revoke role: {:?}", e),
    }

    // Test checking permissions after role revocation
    match rbac_manager.check_permission(
        "alice",
        &Permission::Select,
        &Some("users".to_string()),
        &None,
    ) {
        Ok(has_perm) => {
            println!(
                "✓ User alice has SELECT permission after role revocation: {}",
                has_perm
            );
            assert!(!has_perm);
        }
        Err(e) => panic!("✗ Failed to check permission: {:?}", e),
    }

    // Test dropping user
    println!("\n8. Testing user deletion");
    match rbac_manager.drop_user("alice") {
        Ok(_) => println!("✓ Successfully deleted user: alice"),
        Err(e) => panic!("✗ Failed to delete user: {:?}", e),
    }

    // Test dropping role
    println!("\n9. Testing role deletion");
    match rbac_manager.drop_role("user") {
        Ok(_) => println!("✓ Successfully deleted role: user"),
        Err(e) => panic!("✗ Failed to delete role: {:?}", e),
    }

    println!("\n=== RBAC Basic Operations Test Complete ===");
}

// Test memory allocator
static TEST_MEMORY_ALLOCATOR: DefaultMemoryAllocator = DefaultMemoryAllocator;

// Test config
static TEST_CONFIG: DbConfig = DbConfig {
    total_memory: 1024 * 1024 * 100, // 100MB
    default_max_records: 10000,
    low_power_mode_supported: false,
    low_power_max_records: None,
    wal_config: WALConfig {
        log_path: "./test_wal",
        log_mode: LogMode::Async,
        checkpoint_interval_ms: 60000,
        log_file_size_limit: 16 * 1024 * 1024,
        log_prealloc_size: 1 * 1024 * 1024,
        log_segment_size: 16 * 1024 * 1024,
        max_consecutive_invalid: 100,
        retained_checkpoints: 2,
        skip_threshold: 1000,
        skip_block_size: 1024 * 1024,
        max_skip_attempts: 3,
        compression_type: WALCompressionType::None,
        compression_level: 3,
    },
    tables: Vec::new(),
    memory_allocator: &TEST_MEMORY_ALLOCATOR,
    time_series_defaults: remdb::time_series::TimeSeriesConfig {
        partition_duration_secs: 3600,
        retention_period_secs: 86400,
        compression: remdb::time_series::CompressionType::None,
        max_partitions: 24,
    },
    #[cfg(feature = "pubsub")]
    pubsub_config: None,
    #[cfg(feature = "ha")]
    ha_config: None,

    model_worker_config: remdb::config::ModelWorkerConfig::DEFAULT,
};

#[test]
fn test_rbac_with_remdb() {
    println!("\n=== Testing RBAC with RemDb ===");

    // Initialize platform (required for RemDb operations)
    platform::init_platform(platform::posix::get_posix_platform());

    // Create a new RemDb instance
    let mut db = RemDb::new_with_name("test_rbac", &TEST_CONFIG);

    // Test creating a role through RemDb (skip admin since it exists by default)
    println!("\n1. Testing role creation through RemDb");
    match db.create_role("user") {
        Ok(_) => println!("✓ Successfully created role: user"),
        Err(e) => println!("⚠ Role creation failed (may already exist): {:?}", e),
    }

    // Test granting permissions through RemDb
    println!("\n2. Testing permission granting through RemDb");
    match db.grant_permission("user", Permission::Select, Some("users".to_string()), None) {
        Ok(_) => println!("✓ Successfully granted SELECT permission to user"),
        Err(e) => panic!("✗ Failed to grant permission: {:?}", e),
    }

    // Test creating a user through RemDb
    println!("\n3. Testing user creation through RemDb");
    match db.create_user("alice") {
        Ok(_) => println!("✓ Successfully created user: alice"),
        Err(e) => panic!("✗ Failed to create user: {:?}", e),
    }

    // Test granting role through RemDb
    println!("\n4. Testing role granting through RemDb");
    match db.grant_role("alice", "user") {
        Ok(_) => println!("✓ Successfully granted user role to alice"),
        Err(e) => panic!("✗ Failed to grant role: {:?}", e),
    }

    // Test checking permissions through RemDb
    println!("\n5. Testing permission checking through RemDb");
    match db.check_permission(
        "alice",
        &Permission::Select,
        &Some("users".to_string()),
        &None,
    ) {
        Ok(has_perm) => {
            println!("✓ User alice has SELECT permission: {}", has_perm);
            assert!(has_perm);
        }
        Err(e) => panic!("✗ Failed to check permission: {:?}", e),
    }

    // Test revoking permissions through RemDb
    println!("\n6. Testing permission revocation through RemDb");
    match db.revoke_permission(
        "user",
        &Permission::Select,
        &Some("users".to_string()),
        &None,
    ) {
        Ok(_) => println!("✓ Successfully revoked SELECT permission from user"),
        Err(e) => panic!("✗ Failed to revoke permission: {:?}", e),
    }

    // Test revoking role through RemDb
    println!("\n7. Testing role revocation through RemDb");
    match db.revoke_role("alice", "user") {
        Ok(_) => println!("✓ Successfully revoked user role from alice"),
        Err(e) => panic!("✗ Failed to revoke role: {:?}", e),
    }

    // Test dropping user through RemDb
    println!("\n8. Testing user deletion through RemDb");
    match db.drop_user("alice") {
        Ok(_) => println!("✓ Successfully deleted user: alice"),
        Err(e) => panic!("✗ Failed to delete user: {:?}", e),
    }

    // Test dropping role through RemDb
    println!("\n9. Testing role deletion through RemDb");
    match db.drop_role("user") {
        Ok(_) => println!("✓ Successfully deleted role: user"),
        Err(e) => panic!("✗ Failed to delete role: {:?}", e),
    }

    println!("\n=== RBAC with RemDb Test Complete ===");
}

#[test]
fn test_rbac_fine_grained_permissions() {
    println!("\n=== Testing RBAC Fine-Grained Permissions ===");

    // Create a new RBAC manager
    let mut rbac_manager = RbacManager::new();

    // Test creating roles with different permissions
    println!("\n1. Testing role creation with fine-grained permissions");
    match rbac_manager.create_role("viewer".to_string()) {
        Ok(_) => println!("✓ Successfully created role: viewer"),
        Err(e) => panic!("✗ Failed to create role: {:?}", e),
    }

    match rbac_manager.create_role("editor".to_string()) {
        Ok(_) => println!("✓ Successfully created role: editor"),
        Err(e) => panic!("✗ Failed to create role: {:?}", e),
    }

    // Test granting different permissions
    println!("\n2. Testing fine-grained permission granting");
    match rbac_manager.grant_permission(
        "viewer",
        Permission::Select,
        Some("users".to_string()),
        None,
    ) {
        Ok(_) => println!("✓ Successfully granted SELECT permission to viewer"),
        Err(e) => panic!("✗ Failed to grant permission: {:?}", e),
    }

    match rbac_manager.grant_permission(
        "editor",
        Permission::Select,
        Some("users".to_string()),
        None,
    ) {
        Ok(_) => println!("✓ Successfully granted SELECT permission to editor"),
        Err(e) => panic!("✗ Failed to grant permission: {:?}", e),
    }

    match rbac_manager.grant_permission(
        "editor",
        Permission::Insert,
        Some("users".to_string()),
        None,
    ) {
        Ok(_) => println!("✓ Successfully granted INSERT permission to editor"),
        Err(e) => panic!("✗ Failed to grant permission: {:?}", e),
    }

    match rbac_manager.grant_permission(
        "editor",
        Permission::Update,
        Some("users".to_string()),
        None,
    ) {
        Ok(_) => println!("✓ Successfully granted UPDATE permission to editor"),
        Err(e) => panic!("✗ Failed to grant permission: {:?}", e),
    }

    match rbac_manager.grant_permission(
        "editor",
        Permission::Delete,
        Some("users".to_string()),
        None,
    ) {
        Ok(_) => println!("✓ Successfully granted DELETE permission to editor"),
        Err(e) => panic!("✗ Failed to grant permission: {:?}", e),
    }

    // Test creating users with different roles
    println!("\n3. Testing user creation with different roles");
    match rbac_manager.create_user("bob".to_string()) {
        Ok(_) => println!("✓ Successfully created user: bob"),
        Err(e) => panic!("✗ Failed to create user: {:?}", e),
    }

    match rbac_manager.create_user("charlie".to_string()) {
        Ok(_) => println!("✓ Successfully created user: charlie"),
        Err(e) => panic!("✗ Failed to create user: {:?}", e),
    }

    // Test assigning roles
    println!("\n4. Testing role assignment");
    match rbac_manager.grant_role("bob", "viewer") {
        Ok(_) => println!("✓ Successfully granted viewer role to bob"),
        Err(e) => panic!("✗ Failed to grant role: {:?}", e),
    }

    match rbac_manager.grant_role("charlie", "editor") {
        Ok(_) => println!("✓ Successfully granted editor role to charlie"),
        Err(e) => panic!("✗ Failed to grant role: {:?}", e),
    }

    // Test permission checking
    println!("\n5. Testing fine-grained permission checking");

    // Check viewer permissions
    match rbac_manager.check_permission(
        "bob",
        &Permission::Select,
        &Some("users".to_string()),
        &None,
    ) {
        Ok(has_perm) => {
            println!("✓ User bob has SELECT permission: {}", has_perm);
            assert!(has_perm);
        }
        Err(e) => panic!("✗ Failed to check permission: {:?}", e),
    }

    match rbac_manager.check_permission(
        "bob",
        &Permission::Insert,
        &Some("users".to_string()),
        &None,
    ) {
        Ok(has_perm) => {
            println!("✓ User bob has INSERT permission: {}", has_perm);
            assert!(!has_perm);
        }
        Err(e) => panic!("✗ Failed to check permission: {:?}", e),
    }

    // Check editor permissions
    match rbac_manager.check_permission(
        "charlie",
        &Permission::Select,
        &Some("users".to_string()),
        &None,
    ) {
        Ok(has_perm) => {
            println!("✓ User charlie has SELECT permission: {}", has_perm);
            assert!(has_perm);
        }
        Err(e) => panic!("✗ Failed to check permission: {:?}", e),
    }

    match rbac_manager.check_permission(
        "charlie",
        &Permission::Insert,
        &Some("users".to_string()),
        &None,
    ) {
        Ok(has_perm) => {
            println!("✓ User charlie has INSERT permission: {}", has_perm);
            assert!(has_perm);
        }
        Err(e) => panic!("✗ Failed to check permission: {:?}", e),
    }

    match rbac_manager.check_permission(
        "charlie",
        &Permission::Update,
        &Some("users".to_string()),
        &None,
    ) {
        Ok(has_perm) => {
            println!("✓ User charlie has UPDATE permission: {}", has_perm);
            assert!(has_perm);
        }
        Err(e) => panic!("✗ Failed to check permission: {:?}", e),
    }

    match rbac_manager.check_permission(
        "charlie",
        &Permission::Delete,
        &Some("users".to_string()),
        &None,
    ) {
        Ok(has_perm) => {
            println!("✓ User charlie has DELETE permission: {}", has_perm);
            assert!(has_perm);
        }
        Err(e) => panic!("✗ Failed to check permission: {:?}", e),
    }

    // Test root user has all permissions
    println!("\n6. Testing root user has all permissions");
    let all_permissions = vec![
        Permission::Admin,
        Permission::Select,
        Permission::Insert,
        Permission::Update,
        Permission::Delete,
        Permission::Create,
        Permission::Drop,
    ];

    for permission in &all_permissions {
        match rbac_manager.check_permission("root", permission, &Some("users".to_string()), &None) {
            Ok(has_perm) => {
                println!("✓ Root user has {:?} permission: {}", permission, has_perm);
                assert!(has_perm);
            }
            Err(e) => panic!("✗ Failed to check permission: {:?}", e),
        }
    }

    println!("\n=== RBAC Fine-Grained Permissions Test Complete ===");
}
