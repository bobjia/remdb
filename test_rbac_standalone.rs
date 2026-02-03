//! Standalone test for RBAC module

use remdb::rbac::{RbacManager, Permission};

fn main() {
    println!("=== Running Standalone RBAC Tests ===");
    
    // Test 1: Basic RBAC Operations
    println!("\n=== Test 1: Basic RBAC Operations ===");
    test_basic_operations();
    
    // Test 2: Fine-grained Permissions
    println!("\n=== Test 2: Fine-grained Permissions ===");
    test_fine_grained_permissions();
    
    println!("\n=== All RBAC Tests Passed! ===");
}

fn test_basic_operations() {
    // Create a new RBAC manager
    let mut rbac_manager = RbacManager::new();
    
    // Test creating a role
    println!("\n1. Testing role creation");
    match rbac_manager.create_role("admin".to_string()) {
        Ok(_) => println!("✓ Successfully created role: admin"),
        Err(e) => panic!("✗ Failed to create role: {:?}", e),
    }
    
    // Test granting permissions
    println!("\n2. Testing permission granting");
    match rbac_manager.grant_permission("admin", Permission::Select, Some("users".to_string()), None) {
        Ok(_) => println!("✓ Successfully granted SELECT permission to admin"),
        Err(e) => panic!("✗ Failed to grant permission: {:?}", e),
    }
    
    match rbac_manager.grant_permission("admin", Permission::Insert, Some("users".to_string()), None) {
        Ok(_) => println!("✓ Successfully granted INSERT permission to admin"),
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
    match rbac_manager.grant_role("alice", "admin") {
        Ok(_) => println!("✓ Successfully granted admin role to alice"),
        Err(e) => panic!("✗ Failed to grant role: {:?}", e),
    }
    
    // Test checking permissions
    println!("\n5. Testing permission checking");
    match rbac_manager.check_permission("alice", &Permission::Select, &Some("users".to_string()), &None) {
        Ok(has_perm) => {
            println!("✓ User alice has SELECT permission: {}", has_perm);
            assert!(has_perm, "User alice should have SELECT permission");
        }
        Err(e) => panic!("✗ Failed to check permission: {:?}", e),
    }
    
    match rbac_manager.check_permission("alice", &Permission::Insert, &Some("users".to_string()), &None) {
        Ok(has_perm) => {
            println!("✓ User alice has INSERT permission: {}", has_perm);
            assert!(has_perm, "User alice should have INSERT permission");
        }
        Err(e) => panic!("✗ Failed to check permission: {:?}", e),
    }
    
    match rbac_manager.check_permission("alice", &Permission::Delete, &Some("users".to_string()), &None) {
        Ok(has_perm) => {
            println!("✓ User alice has DELETE permission: {}", has_perm);
            assert!(!has_perm, "User alice should not have DELETE permission");
        }
        Err(e) => panic!("✗ Failed to check permission: {:?}", e),
    }
    
    // Test revoking permissions
    println!("\n6. Testing permission revocation");
    match rbac_manager.revoke_permission("admin", &Permission::Insert, &Some("users".to_string()), &None) {
        Ok(_) => println!("✓ Successfully revoked INSERT permission from admin"),
        Err(e) => panic!("✗ Failed to revoke permission: {:?}", e),
    }
    
    // Test checking permissions after revocation
    match rbac_manager.check_permission("alice", &Permission::Insert, &Some("users".to_string()), &None) {
        Ok(has_perm) => {
            println!("✓ User alice has INSERT permission after revocation: {}", has_perm);
            assert!(!has_perm, "User alice should not have INSERT permission after revocation");
        }
        Err(e) => panic!("✗ Failed to check permission: {:?}", e),
    }
    
    // Test revoking role
    println!("\n7. Testing role revocation");
    match rbac_manager.revoke_role("alice", "admin") {
        Ok(_) => println!("✓ Successfully revoked admin role from alice"),
        Err(e) => panic!("✗ Failed to revoke role: {:?}", e),
    }
    
    // Test checking permissions after role revocation
    match rbac_manager.check_permission("alice", &Permission::Select, &Some("users".to_string()), &None) {
        Ok(has_perm) => {
            println!("✓ User alice has SELECT permission after role revocation: {}", has_perm);
            assert!(!has_perm, "User alice should not have SELECT permission after role revocation");
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
    match rbac_manager.drop_role("admin") {
        Ok(_) => println!("✓ Successfully deleted role: admin"),
        Err(e) => panic!("✗ Failed to delete role: {:?}", e),
    }
}

fn test_fine_grained_permissions() {
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
    match rbac_manager.grant_permission("viewer", Permission::Select, Some("users".to_string()), None) {
        Ok(_) => println!("✓ Successfully granted SELECT permission to viewer"),
        Err(e) => panic!("✗ Failed to grant permission: {:?}", e),
    }
    
    match rbac_manager.grant_permission("editor", Permission::Select, Some("users".to_string()), None) {
        Ok(_) => println!("✓ Successfully granted SELECT permission to editor"),
        Err(e) => panic!("✗ Failed to grant permission: {:?}", e),
    }
    
    match rbac_manager.grant_permission("editor", Permission::Insert, Some("users".to_string()), None) {
        Ok(_) => println!("✓ Successfully granted INSERT permission to editor"),
        Err(e) => panic!("✗ Failed to grant permission: {:?}", e),
    }
    
    match rbac_manager.grant_permission("editor", Permission::Update, Some("users".to_string()), None) {
        Ok(_) => println!("✓ Successfully granted UPDATE permission to editor"),
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
    match rbac_manager.check_permission("bob", &Permission::Select, &Some("users".to_string()), &None) {
        Ok(has_perm) => {
            println!("✓ User bob has SELECT permission: {}", has_perm);
            assert!(has_perm, "User bob should have SELECT permission");
        }
        Err(e) => panic!("✗ Failed to check permission: {:?}", e),
    }
    
    match rbac_manager.check_permission("bob", &Permission::Insert, &Some("users".to_string()), &None) {
        Ok(has_perm) => {
            println!("✓ User bob has INSERT permission: {}", has_perm);
            assert!(!has_perm, "User bob should not have INSERT permission");
        }
        Err(e) => panic!("✗ Failed to check permission: {:?}", e),
    }
    
    // Check editor permissions
    match rbac_manager.check_permission("charlie", &Permission::Select, &Some("users".to_string()), &None) {
        Ok(has_perm) => {
            println!("✓ User charlie has SELECT permission: {}", has_perm);
            assert!(has_perm, "User charlie should have SELECT permission");
        }
        Err(e) => panic!("✗ Failed to check permission: {:?}", e),
    }
    
    match rbac_manager.check_permission("charlie", &Permission::Insert, &Some("users".to_string()), &None) {
        Ok(has_perm) => {
            println!("✓ User charlie has INSERT permission: {}", has_perm);
            assert!(has_perm, "User charlie should have INSERT permission");
        }
        Err(e) => panic!("✗ Failed to check permission: {:?}", e),
    }
    
    match rbac_manager.check_permission("charlie", &Permission::Update, &Some("users".to_string()), &None) {
        Ok(has_perm) => {
            println!("✓ User charlie has UPDATE permission: {}", has_perm);
            assert!(has_perm, "User charlie should have UPDATE permission");
        }
        Err(e) => panic!("✗ Failed to check permission: {:?}", e),
    }
    
    match rbac_manager.check_permission("charlie", &Permission::Delete, &Some("users".to_string()), &None) {
        Ok(has_perm) => {
            println!("✓ User charlie has DELETE permission: {}", has_perm);
            assert!(!has_perm, "User charlie should not have DELETE permission");
        }
        Err(e) => panic!("✗ Failed to check permission: {:?}", e),
    }
}
