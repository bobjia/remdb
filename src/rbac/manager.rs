//! RBAC Manager module
//! 
//! This module defines the RbacManager struct for managing RBAC operations.

use std::collections::HashMap;
use std::string::String;
use std::vec::Vec;

use crate::rbac::permission::Permission;
use crate::rbac::role::Role;
use crate::rbac::user::User;

/// RBAC Manager for managing roles and users
#[derive(Debug, Clone)]
pub struct RbacManager {
    /// Roles mapping (role name -> role)
    roles: HashMap<String, Role>,
    /// Users mapping (user name -> user)
    users: HashMap<String, User>,
}

impl RbacManager {
    /// Create a new RbacManager
    pub fn new() -> Self {
        let mut manager = Self {
            roles: HashMap::new(),
            users: HashMap::new(),
        };

        // Create default admin role with all permissions
        let mut admin_role = Role::new("admin".to_string());
        // Grant all permissions to admin role
        admin_role.add_permission(Permission::Select, None, None);
        admin_role.add_permission(Permission::Insert, None, None);
        admin_role.add_permission(Permission::Update, None, None);
        admin_role.add_permission(Permission::Delete, None, None);
        manager.roles.insert("admin".to_string(), admin_role);

        // Create default current_user and assign admin role
        let mut current_user = User::new("current_user".to_string());
        current_user.add_role("admin".to_string());
        manager.users.insert("current_user".to_string(), current_user);

        manager
    }

    /// Create a new role
    pub fn create_role(&mut self, role_name: String) -> Result<(), RbacError> {
        if self.roles.contains_key(&role_name) {
            return Err(RbacError::RoleAlreadyExists(role_name));
        }

        let role = Role::new(role_name);
        self.roles.insert(role.name.clone(), role);
        Ok(())
    }

    /// Drop a role
    pub fn drop_role(&mut self, role_name: &str) -> Result<(), RbacError> {
        if !self.roles.contains_key(role_name) {
            return Err(RbacError::RoleNotFound(role_name.to_string()));
        }

        // Remove the role from all users
        for user in self.users.values_mut() {
            user.remove_role(role_name);
        }

        // Remove the role itself
        self.roles.remove(role_name);
        Ok(())
    }

    /// Grant permission to a role
    pub fn grant_permission(
        &mut self, 
        role_name: &str, 
        permission: Permission, 
        table_name: Option<String>, 
        column_name: Option<String>
    ) -> Result<(), RbacError> {
        let role = self.roles.get_mut(role_name).ok_or(RbacError::RoleNotFound(role_name.to_string()))?;
        role.add_permission(permission, table_name, column_name);
        Ok(())
    }

    /// Revoke permission from a role
    pub fn revoke_permission(
        &mut self, 
        role_name: &str, 
        permission: &Permission, 
        table_name: &Option<String>, 
        column_name: &Option<String>
    ) -> Result<(), RbacError> {
        let role = self.roles.get_mut(role_name).ok_or(RbacError::RoleNotFound(role_name.to_string()))?;
        role.remove_permission(permission, table_name, column_name);
        Ok(())
    }

    /// Create a new user
    pub fn create_user(&mut self, user_name: String) -> Result<(), RbacError> {
        if self.users.contains_key(&user_name) {
            return Err(RbacError::UserAlreadyExists(user_name));
        }

        let user = User::new(user_name);
        self.users.insert(user.name.clone(), user);
        Ok(())
    }

    /// Drop a user
    pub fn drop_user(&mut self, user_name: &str) -> Result<(), RbacError> {
        if !self.users.contains_key(user_name) {
            return Err(RbacError::UserNotFound(user_name.to_string()));
        }

        self.users.remove(user_name);
        Ok(())
    }

    /// Grant a role to a user
    pub fn grant_role(&mut self, user_name: &str, role_name: &str) -> Result<(), RbacError> {
        if !self.roles.contains_key(role_name) {
            return Err(RbacError::RoleNotFound(role_name.to_string()));
        }

        let user = self.users.get_mut(user_name).ok_or(RbacError::UserNotFound(user_name.to_string()))?;
        user.add_role(role_name.to_string());
        Ok(())
    }

    /// Revoke a role from a user
    pub fn revoke_role(&mut self, user_name: &str, role_name: &str) -> Result<(), RbacError> {
        let user = self.users.get_mut(user_name).ok_or(RbacError::UserNotFound(user_name.to_string()))?;
        user.remove_role(role_name);
        Ok(())
    }

    /// Check if a user has a specific permission
    pub fn check_permission(
        &self, 
        user_name: &str, 
        permission: &Permission, 
        table_name: &Option<String>, 
        column_name: &Option<String>
    ) -> Result<bool, RbacError> {
        let user = self.users.get(user_name).ok_or(RbacError::UserNotFound(user_name.to_string()))?;

        // Check all roles assigned to the user
        for role_name in &user.roles {
            if let Some(role) = self.roles.get(role_name) {
                if role.has_permission(permission, table_name, column_name) {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Get all roles
    pub fn get_roles(&self) -> Vec<&Role> {
        self.roles.values().collect()
    }

    /// Get all users
    pub fn get_users(&self) -> Vec<&User> {
        self.users.values().collect()
    }

    /// Get a specific role
    pub fn get_role(&self, role_name: &str) -> Option<&Role> {
        self.roles.get(role_name)
    }

    /// Get a specific user
    pub fn get_user(&self, user_name: &str) -> Option<&User> {
        self.users.get(user_name)
    }
}

/// RBAC Error types
#[derive(Debug, Clone, PartialEq)]
pub enum RbacError {
    /// Role already exists
    RoleAlreadyExists(String),
    /// Role not found
    RoleNotFound(String),
    /// User already exists
    UserAlreadyExists(String),
    /// User not found
    UserNotFound(String),
    /// Permission not found
    PermissionNotFound,
}

impl core::fmt::Display for RbacError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RbacError::RoleAlreadyExists(role) => write!(f, "Role already exists: {}", role),
            RbacError::RoleNotFound(role) => write!(f, "Role not found: {}", role),
            RbacError::UserAlreadyExists(user) => write!(f, "User already exists: {}", user),
            RbacError::UserNotFound(user) => write!(f, "User not found: {}", user),
            RbacError::PermissionNotFound => write!(f, "Permission not found"),
        }
    }
}

impl core::error::Error for RbacError {}
