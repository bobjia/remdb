//! Minimal test to verify RBAC module compiles

use remdb::rbac::{RbacManager, Permission};

fn main() {
    println!("=== Testing RBAC Module Compilation ===");
    
    // Create a new RBAC manager
    let mut rbac_manager = RbacManager::new();
    
    // Test creating a role
    match rbac_manager.create_role("admin".to_string()) {
        Ok(_) => println!("✓ Successfully created role: admin"),
        Err(e) => println!("✗ Failed to create role: {:?}", e),
    }
    
    // Test granting permissions
    match rbac_manager.grant_permission("admin", Permission::Select, Some("users".to_string()), None) {
        Ok(_) => println!("✓ Successfully granted SELECT permission to admin"),
        Err(e) => println!("✗ Failed to grant permission: {:?}", e),
    }
    
    // Test creating a user
    match rbac_manager.create_user("alice".to_string()) {
        Ok(_) => println!("✓ Successfully created user: alice"),
        Err(e) => println!("✗ Failed to create user: {:?}", e),
    }
    
    // Test granting role to user
    match rbac_manager.grant_role("alice", "admin") {
        Ok(_) => println!("✓ Successfully granted admin role to alice"),
        Err(e) => println!("✗ Failed to grant role: {:?}", e),
    }
    
    // Test checking permissions
    match rbac_manager.check_permission("alice", &Permission::Select, &Some("users".to_string()), &None) {
        Ok(has_perm) => println!("✓ User alice has SELECT permission: {}", has_perm),
        Err(e) => println!("✗ Failed to check permission: {:?}", e),
    }
    
    println!("=== RBAC Module Compilation Test Complete ===");
}
