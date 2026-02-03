//! Test cases for RBAC functionality

use remdb::rbac::{RbacManager, Permission};
use remdb::RemDb;

#[test]
fn test_rbac_basic_operations() {
    println!("=== Testing RBAC Basic Operations ===");
    
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
            assert!(has_perm);
        }
        Err(e) => panic!("✗ Failed to check permission: {:?}", e),
    }
    
    match rbac_manager.check_permission("alice", &Permission::Insert, &Some("users".to_string()), &None) {
        Ok(has_perm) => {
            println!("✓ User alice has INSERT permission: {}", has_perm);
            assert!(has_perm);
        }
        Err(e) => panic!("✗ Failed to check permission: {:?}", e),
    }
    
    match rbac_manager.check_permission("alice", &Permission::Delete, &Some("users".to_string()), &None) {
        Ok(has_perm) => {
            println!("✓ User alice has DELETE permission: {}", has_perm);
            assert!(!has_perm);
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
            assert!(!has_perm);
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
    match rbac_manager.drop_role("admin") {
        Ok(_) => println!("✓ Successfully deleted role: admin"),
        Err(e) => panic!("✗ Failed to delete role: {:?}", e),
    }
    
    println!("\n=== RBAC Basic Operations Test Complete ===");
}

#[test]
fn test_rbac_with_remdb() {
    println!("\n=== Testing RBAC with RemDb ===");
    
    // Create a new RemDb instance
    let mut db = RemDb::new_with_name("test_rbac".to_string());
    
    // Test creating a role through RemDb
    println!("\n1. Testing role creation through RemDb");
    match db.create_role("admin") {
        Ok(_) => println!("✓ Successfully created role: admin"),
        Err(e) => panic!("✗ Failed to create role: {:?}", e),
    }
    
    // Test granting permissions through RemDb
    println!("\n2. Testing permission granting through RemDb");
    match db.grant_permission("admin", Permission::Select, "users") {
        Ok(_) => println!("✓ Successfully granted SELECT permission to admin"),
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
    match db.grant_role("alice", "admin") {
        Ok(_) => println!("✓ Successfully granted admin role to alice"),
        Err(e) => panic!("✗ Failed to grant role: {:?}", e),
    }
    
    // Test checking permissions through RemDb
    println!("\n5. Testing permission checking through RemDb");
    match db.check_permission("alice", Permission::Select, "users") {
        Ok(has_perm) => {
            println!("✓ User alice has SELECT permission: {}", has_perm);
            assert!(has_perm);
        }
        Err(e) => panic!("✗ Failed to check permission: {:?}", e),
    }
    
    // Test revoking permissions through RemDb
    println!("\n6. Testing permission revocation through RemDb");
    match db.revoke_permission("admin", Permission::Select, "users") {
        Ok(_) => println!("✓ Successfully revoked SELECT permission from admin"),
        Err(e) => panic!("✗ Failed to revoke permission: {:?}", e),
    }
    
    // Test revoking role through RemDb
    println!("\n7. Testing role revocation through RemDb");
    match db.revoke_role("alice", "admin") {
        Ok(_) => println!("✓ Successfully revoked admin role from alice"),
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
    match db.drop_role("admin") {
        Ok(_) => println!("✓ Successfully deleted role: admin"),
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
            assert!(has_perm);
        }
        Err(e) => panic!("✗ Failed to check permission: {:?}", e),
    }
    
    match rbac_manager.check_permission("bob", &Permission::Insert, &Some("users".to_string()), &None) {
        Ok(has_perm) => {
            println!("✓ User bob has INSERT permission: {}", has_perm);
            assert!(!has_perm);
        }
        Err(e) => panic!("✗ Failed to check permission: {:?}", e),
    }
    
    // Check editor permissions
    match rbac_manager.check_permission("charlie", &Permission::Select, &Some("users".to_string()), &None) {
        Ok(has_perm) => {
            println!("✓ User charlie has SELECT permission: {}", has_perm);
            assert!(has_perm);
        }
        Err(e) => panic!("✗ Failed to check permission: {:?}", e),
    }
    
    match rbac_manager.check_permission("charlie", &Permission::Insert, &Some("users".to_string()), &None) {
        Ok(has_perm) => {
            println!("✓ User charlie has INSERT permission: {}", has_perm);
            assert!(has_perm);
        }
        Err(e) => panic!("✗ Failed to check permission: {:?}", e),
    }
    
    match rbac_manager.check_permission("charlie", &Permission::Update, &Some("users".to_string()), &None) {
        Ok(has_perm) => {
            println!("✓ User charlie has UPDATE permission: {}", has_perm);
            assert!(has_perm);
        }
        Err(e) => panic!("✗ Failed to check permission: {:?}", e),
    }
    
    match rbac_manager.check_permission("charlie", &Permission::Delete, &Some("users".to_string()), &None) {
        Ok(has_perm) => {
            println!("✓ User charlie has DELETE permission: {}", has_perm);
            assert!(!has_perm);
        }
        Err(e) => panic!("✗ Failed to check permission: {:?}", e),
    }
    
    println!("\n=== RBAC Fine-Grained Permissions Test Complete ===");
}
