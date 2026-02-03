//! Test program for RBAC module

use remdb::rbac::{RbacManager, Permission};

fn main() {
    println!("=== Testing RBAC Module ===");
    
    // Create a new RBAC manager
    let mut rbac_manager = RbacManager::new();
    
    // Test creating a role
    println!("\n1. Testing role creation");
    match rbac_manager.create_role("admin".to_string()) {
        Ok(_) => println!("✓ Successfully created role: admin"),
        Err(e) => println!("✗ Failed to create role: {:?}", e),
    }
    
    // Test granting permissions
    println!("\n2. Testing permission granting");
    match rbac_manager.grant_permission("admin", Permission::Select, Some("users".to_string()), None) {
        Ok(_) => println!("✓ Successfully granted SELECT permission to admin"),
        Err(e) => println!("✗ Failed to grant permission: {:?}", e),
    }
    
    match rbac_manager.grant_permission("admin", Permission::Insert, Some("users".to_string()), None) {
        Ok(_) => println!("✓ Successfully granted INSERT permission to admin"),
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
    match rbac_manager.grant_role("alice", "admin") {
        Ok(_) => println!("✓ Successfully granted admin role to alice"),
        Err(e) => println!("✗ Failed to grant role: {:?}", e),
    }
    
    // Test checking permissions
    println!("\n5. Testing permission checking");
    match rbac_manager.check_permission("alice", &Permission::Select, &Some("users".to_string()), &None) {
        Ok(has_perm) => println!("✓ User alice has SELECT permission: {}", has_perm),
        Err(e) => println!("✗ Failed to check permission: {:?}", e),
    }
    
    match rbac_manager.check_permission("alice", &Permission::Insert, &Some("users".to_string()), &None) {
        Ok(has_perm) => println!("✓ User alice has INSERT permission: {}", has_perm),
        Err(e) => println!("✗ Failed to check permission: {:?}", e),
    }
    
    match rbac_manager.check_permission("alice", &Permission::Delete, &Some("users".to_string()), &None) {
        Ok(has_perm) => println!("✓ User alice has DELETE permission: {}", has_perm),
        Err(e) => println!("✗ Failed to check permission: {:?}", e),
    }
    
    // Test revoking permissions
    println!("\n6. Testing permission revocation");
    match rbac_manager.revoke_permission("admin", &Permission::Insert, &Some("users".to_string()), &None) {
        Ok(_) => println!("✓ Successfully revoked INSERT permission from admin"),
        Err(e) => println!("✗ Failed to revoke permission: {:?}", e),
    }
    
    // Test checking permissions after revocation
    match rbac_manager.check_permission("alice", &Permission::Insert, &Some("users".to_string()), &None) {
        Ok(has_perm) => println!("✓ User alice has INSERT permission after revocation: {}", has_perm),
        Err(e) => println!("✗ Failed to check permission: {:?}", e),
    }
    
    // Test revoking role
    println!("\n7. Testing role revocation");
    match rbac_manager.revoke_role("alice", "admin") {
        Ok(_) => println!("✓ Successfully revoked admin role from alice"),
        Err(e) => println!("✗ Failed to revoke role: {:?}", e),
    }
    
    // Test checking permissions after role revocation
    match rbac_manager.check_permission("alice", &Permission::Select, &Some("users".to_string()), &None) {
        Ok(has_perm) => println!("✓ User alice has SELECT permission after role revocation: {}", has_perm),
        Err(e) => println!("✗ Failed to check permission: {:?}", e),
    }
    
    // Test dropping user
    println!("\n8. Testing user deletion");
    match rbac_manager.drop_user("alice") {
        Ok(_) => println!("✓ Successfully deleted user: alice"),
        Err(e) => println!("✗ Failed to delete user: {:?}", e),
    }
    
    // Test dropping role
    println!("\n9. Testing role deletion");
    match rbac_manager.drop_role("admin") {
        Ok(_) => println!("✓ Successfully deleted role: admin"),
        Err(e) => println!("✗ Failed to delete role: {:?}", e),
    }
    
    println!("\n=== RBAC Module Test Complete ===");
}
