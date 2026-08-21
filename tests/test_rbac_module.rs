//! Test program for RBAC module

use remdb::rbac::{RbacManager, Permission};

#[test]
fn test_rbac_module() {
    println!("=== Testing RBAC Module ===");
    
    // Create a new RBAC manager
    let mut rbac_manager = RbacManager::new();
    
    // Test that root user exists by default
    println!("\n0. Testing root user exists by default");
    assert!(rbac_manager.get_user("root").is_some(), "Root user should exist by default");
    println!("✓ Root user exists");
    
    // Test that admin role exists by default
    println!("\n0.1. Testing admin role exists by default");
    assert!(rbac_manager.get_role("admin").is_some(), "Admin role should exist by default");
    println!("✓ Admin role exists");
    
    // Test creating a role
    println!("\n1. Testing role creation");
    match rbac_manager.create_role("user".to_string()) {
        Ok(_) => println!("✓ Successfully created role: user"),
        Err(e) => println!("⚠ Role creation failed (may already exist): {:?}", e),
    }
    
    // Test granting permissions
    println!("\n2. Testing permission granting");
    match rbac_manager.grant_permission("user", Permission::Select, Some("users".to_string()), None) {
        Ok(_) => println!("✓ Successfully granted SELECT permission to user"),
        Err(e) => println!("✗ Failed to grant permission: {:?}", e),
    }
    
    match rbac_manager.grant_permission("user", Permission::Insert, Some("users".to_string()), None) {
        Ok(_) => println!("✓ Successfully granted INSERT permission to user"),
        Err(e) => println!("✗ Failed to grant permission: {:?}", e),
    }
    
    // Test creating a user
    println!("\n3. Testing user creation");
    match rbac_manager.create_user("alice".to_string()) {
        Ok(_) => println!("✓ Successfully created user: alice"),
        Err(e) => println!("✗ Failed to create user: {:?}", e),
    }
    
    // Test granting role to user
    println!("\n4. Testing role granting");
    match rbac_manager.grant_role("alice", "user") {
        Ok(_) => println!("✓ Successfully granted user role to alice"),
        Err(e) => println!("✗ Failed to grant role: {:?}", e),
    }
    
    // Test checking permissions
    println!("\n5. Testing permission checking");
    match rbac_manager.check_permission("alice", &Permission::Select, &Some("users".to_string()), &None) {
        Ok(has_perm) => {
            println!("✓ User alice has SELECT permission: {}", has_perm);
            assert!(has_perm);
        }
        Err(e) => {
            println!("✗ Failed to check permission: {:?}", e);
            assert!(false);
        }
    }
    
    match rbac_manager.check_permission("alice", &Permission::Insert, &Some("users".to_string()), &None) {
        Ok(has_perm) => {
            println!("✓ User alice has INSERT permission: {}", has_perm);
            assert!(has_perm);
        }
        Err(e) => {
            println!("✗ Failed to check permission: {:?}", e);
            assert!(false);
        }
    }
    
    match rbac_manager.check_permission("alice", &Permission::Delete, &Some("users".to_string()), &None) {
        Ok(has_perm) => {
            println!("✓ User alice has DELETE permission: {}", has_perm);
            assert!(!has_perm);
        }
        Err(e) => {
            println!("✗ Failed to check permission: {:?}", e);
            assert!(false);
        }
    }
    
    // Test root user has all permissions
    println!("\n5.1. Testing root user has all permissions");
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
            Err(e) => {
                println!("✗ Failed to check permission: {:?}", e);
                assert!(false);
            }
        }
    }
    
    // Test revoking permissions
    println!("\n6. Testing permission revocation");
    match rbac_manager.revoke_permission("user", &Permission::Insert, &Some("users".to_string()), &None) {
        Ok(_) => println!("✓ Successfully revoked INSERT permission from user"),
        Err(e) => println!("✗ Failed to revoke permission: {:?}", e),
    }
    
    // Test checking permissions after revocation
    match rbac_manager.check_permission("alice", &Permission::Insert, &Some("users".to_string()), &None) {
        Ok(has_perm) => {
            println!("✓ User alice has INSERT permission after revocation: {}", has_perm);
            assert!(!has_perm);
        }
        Err(e) => {
            println!("✗ Failed to check permission: {:?}", e);
            assert!(false);
        }
    }
    
    // Test revoking role
    println!("\n7. Testing role revocation");
    match rbac_manager.revoke_role("alice", "user") {
        Ok(_) => println!("✓ Successfully revoked user role from alice"),
        Err(e) => println!("✗ Failed to revoke role: {:?}", e),
    }
    
    // Test checking permissions after role revocation
    match rbac_manager.check_permission("alice", &Permission::Select, &Some("users".to_string()), &None) {
        Ok(has_perm) => {
            println!("✓ User alice has SELECT permission after role revocation: {}", has_perm);
            assert!(!has_perm);
        }
        Err(e) => {
            println!("✗ Failed to check permission: {:?}", e);
            assert!(false);
        }
    }
    
    // Test dropping user
    println!("\n8. Testing user deletion");
    match rbac_manager.drop_user("alice") {
        Ok(_) => println!("✓ Successfully deleted user: alice"),
        Err(e) => println!("✗ Failed to delete user: {:?}", e),
    }
    
    // Test dropping role
    println!("\n9. Testing role deletion");
    match rbac_manager.drop_role("user") {
        Ok(_) => println!("✓ Successfully deleted role: user"),
        Err(e) => println!("✗ Failed to delete role: {:?}", e),
    }
    
    println!("\n=== RBAC Module Test Complete ===");
}
