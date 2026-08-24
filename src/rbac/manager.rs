//! RBAC Manager module
//!
//! This module defines the RbacManager struct for managing RBAC operations.

use core::result::Result;
use std::collections::HashMap;
use std::string::String;
use std::vec::Vec;

use crate::rbac::permission::Permission;
use crate::rbac::role::Role;
use crate::rbac::user::User;

/// RBAC Manager for managing roles and users
#[derive(Debug)]
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
        admin_role.add_permission(Permission::Admin, None, None);
        admin_role.add_permission(Permission::Select, None, None);
        admin_role.add_permission(Permission::Insert, None, None);
        admin_role.add_permission(Permission::Update, None, None);
        admin_role.add_permission(Permission::Delete, None, None);
        admin_role.add_permission(Permission::Create, None, None);
        admin_role.add_permission(Permission::Drop, None, None);
        manager.roles.insert("admin".to_string(), admin_role);

        // Create default root user and assign admin role
        let mut root_user = User::new("root".to_string());
        root_user.add_role("admin".to_string());
        manager.users.insert("root".to_string(), root_user);

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
        column_name: Option<String>,
    ) -> Result<(), RbacError> {
        let role = self
            .roles
            .get_mut(role_name)
            .ok_or(RbacError::RoleNotFound(role_name.to_string()))?;
        role.add_permission(permission, table_name, column_name);
        Ok(())
    }

    /// Revoke permission from a role
    pub fn revoke_permission(
        &mut self,
        role_name: &str,
        permission: &Permission,
        table_name: &Option<String>,
        column_name: &Option<String>,
    ) -> Result<(), RbacError> {
        let role = self
            .roles
            .get_mut(role_name)
            .ok_or(RbacError::RoleNotFound(role_name.to_string()))?;
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

        let user = self
            .users
            .get_mut(user_name)
            .ok_or(RbacError::UserNotFound(user_name.to_string()))?;
        user.add_role(role_name.to_string());
        Ok(())
    }

    /// Revoke a role from a user
    pub fn revoke_role(&mut self, user_name: &str, role_name: &str) -> Result<(), RbacError> {
        let user = self
            .users
            .get_mut(user_name)
            .ok_or(RbacError::UserNotFound(user_name.to_string()))?;
        user.remove_role(role_name);
        Ok(())
    }

    /// Check if a user has a specific permission
    pub fn check_permission(
        &self,
        user_name: &str,
        permission: &Permission,
        table_name: &Option<String>,
        column_name: &Option<String>,
    ) -> Result<bool, RbacError> {
        let user = self
            .users
            .get(user_name)
            .ok_or(RbacError::UserNotFound(user_name.to_string()))?;

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

    /// Load RBAC data from system tables
    pub unsafe fn load_from_system_tables(
        manager: &mut Self,
        db: &crate::RemDb,
    ) -> Result<(), RbacError> {
        use crate::system_tables::{
            SYSTEM_ROLES_TABLE, SYSTEM_ROLE_PERMISSIONS_TABLE, SYSTEM_USERS_TABLE,
            SYSTEM_USER_ROLES_TABLE,
        };

        // Clear existing data
        manager.roles.clear();
        manager.users.clear();

        // Load roles
        if let Some(roles_table_id) = db.tables.iter().position(|table_opt| {
            table_opt
                .as_ref()
                .map(|table| table.def.name == SYSTEM_ROLES_TABLE)
                .unwrap_or(false)
        }) {
            if let Ok(roles_table) = db.get_table(roles_table_id) {
                let cursor = roles_table.scan_ref();
                for record in cursor {
                    let role_name = record.get_str(0).unwrap_or("");
                    if !role_name.is_empty() {
                        let role = Role::new(role_name.to_string());
                        manager.roles.insert(role_name.to_string(), role);
                    }
                }
            }
        }

        // Load role permissions
        if let Some(role_perms_table_id) = db.tables.iter().position(|table_opt| {
            table_opt
                .as_ref()
                .map(|table| table.def.name == SYSTEM_ROLE_PERMISSIONS_TABLE)
                .unwrap_or(false)
        }) {
            if let Ok(role_perms_table) = db.get_table(role_perms_table_id) {
                let cursor = role_perms_table.scan_ref();
                for record in cursor {
                    let role_name = record.get_str(0).unwrap_or("");
                    let perm_str = record.get_str(1).unwrap_or("");
                    let table_name = record.get_str(2).unwrap_or("");

                    if !role_name.is_empty() && !perm_str.is_empty() {
                        if let Some(role) = manager.roles.get_mut(role_name) {
                            if let Some(permission) = Permission::from_str(perm_str) {
                                let table_name_opt = if table_name.is_empty() {
                                    None
                                } else {
                                    Some(table_name.to_string())
                                };
                                role.add_permission(permission, table_name_opt, None);
                            }
                        }
                    }
                }
            }
        }

        // Load users
        if let Some(users_table_id) = db.tables.iter().position(|table_opt| {
            table_opt
                .as_ref()
                .map(|table| table.def.name == SYSTEM_USERS_TABLE)
                .unwrap_or(false)
        }) {
            if let Ok(users_table) = db.get_table(users_table_id) {
                let cursor = users_table.scan_ref();
                for record in cursor {
                    let username = record.get_str(0).unwrap_or("");
                    if !username.is_empty() {
                        let user = User::new(username.to_string());
                        manager.users.insert(username.to_string(), user);
                    }
                }
            }
        }

        // Load user roles
        if let Some(user_roles_table_id) = db.tables.iter().position(|table_opt| {
            table_opt
                .as_ref()
                .map(|table| table.def.name == SYSTEM_USER_ROLES_TABLE)
                .unwrap_or(false)
        }) {
            if let Ok(user_roles_table) = db.get_table(user_roles_table_id) {
                let cursor = user_roles_table.scan_ref();
                for record in cursor {
                    let username = record.get_str(0).unwrap_or("");
                    let role_name = record.get_str(1).unwrap_or("");

                    if !username.is_empty() && !role_name.is_empty() {
                        if let Some(user) = manager.users.get_mut(username) {
                            user.add_role(role_name.to_string());
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Save RBAC data to system tables
    pub unsafe fn save_to_system_tables(db: &mut crate::RemDb) -> Result<(), RbacError> {
        use crate::platform::{memcpy, memset};
        use crate::system_tables::{
            SYSTEM_ROLES_TABLE, SYSTEM_ROLE_PERMISSIONS_TABLE, SYSTEM_USERS_TABLE,
            SYSTEM_USER_ROLES_TABLE,
        };

        let manager = &mut db.rbac_manager;
        let now = crate::platform::get_timestamp_us();

        // Collect data to avoid borrowing issues
        let roles: Vec<_> = manager.roles.values().cloned().collect();
        let users: Vec<_> = manager.users.values().cloned().collect();

        // Save roles
        if let Some(roles_table_id) = db.tables.iter().position(|table_opt| {
            table_opt
                .as_ref()
                .map(|table| table.def.name == SYSTEM_ROLES_TABLE)
                .unwrap_or(false)
        }) {
            if let Ok(roles_table) = db.get_table_mut(roles_table_id) {
                // Clear existing records
                // roles_table.clear();

                // Insert roles
                for role in &roles {
                    let mut record_data = [0u8; 64 + 256 + 8 + 8];
                    let mut offset = 0;

                    // role_name
                    memset(record_data.as_mut_ptr().add(offset), 0, 64);
                    let role_name_bytes = role.name.as_bytes();
                    memcpy(
                        record_data.as_mut_ptr().add(offset),
                        role_name_bytes.as_ptr(),
                        role_name_bytes.len(),
                    );
                    offset += 64;

                    // description (empty for now)
                    memset(record_data.as_mut_ptr().add(offset), 0, 256);
                    offset += 256;

                    // created_at
                    memcpy(
                        record_data.as_mut_ptr().add(offset),
                        &now as *const u64 as *const u8,
                        8,
                    );
                    offset += 8;

                    // updated_at
                    memcpy(
                        record_data.as_mut_ptr().add(offset),
                        &now as *const u64 as *const u8,
                        8,
                    );

                    let _ = roles_table.insert(record_data.as_ptr());
                }
            }
        }

        // Save role permissions
        if let Some(role_perms_table_id) = db.tables.iter().position(|table_opt| {
            table_opt
                .as_ref()
                .map(|table| table.def.name == SYSTEM_ROLE_PERMISSIONS_TABLE)
                .unwrap_or(false)
        }) {
            if let Ok(role_perms_table) = db.get_table_mut(role_perms_table_id) {
                // Clear existing records
                // role_perms_table.clear();

                // Insert permissions
                for role in &roles {
                    for (permission, table_name, _column_name) in &role.permissions {
                        let mut record_data = [0u8; 64 + 64 + 256 + 8];
                        let mut offset = 0;

                        // role_name
                        memset(record_data.as_mut_ptr().add(offset), 0, 64);
                        let role_name_bytes = role.name.as_bytes();
                        memcpy(
                            record_data.as_mut_ptr().add(offset),
                            role_name_bytes.as_ptr(),
                            role_name_bytes.len(),
                        );
                        offset += 64;

                        // permission
                        memset(record_data.as_mut_ptr().add(offset), 0, 64);
                        let perm_str = permission.to_string();
                        let perm_bytes = perm_str.as_bytes();
                        memcpy(
                            record_data.as_mut_ptr().add(offset),
                            perm_bytes.as_ptr(),
                            perm_bytes.len(),
                        );
                        offset += 64;

                        // table_name
                        memset(record_data.as_mut_ptr().add(offset), 0, 256);
                        if let Some(table) = table_name {
                            let table_bytes = table.as_bytes();
                            memcpy(
                                record_data.as_mut_ptr().add(offset),
                                table_bytes.as_ptr(),
                                table_bytes.len(),
                            );
                        }
                        offset += 256;

                        // created_at
                        memcpy(
                            record_data.as_mut_ptr().add(offset),
                            &now as *const u64 as *const u8,
                            8,
                        );

                        let _ = role_perms_table.insert(record_data.as_ptr());
                    }
                }
            }
        }

        // Save users
        if let Some(users_table_id) = db.tables.iter().position(|table_opt| {
            table_opt
                .as_ref()
                .map(|table| table.def.name == SYSTEM_USERS_TABLE)
                .unwrap_or(false)
        }) {
            if let Ok(users_table) = db.get_table_mut(users_table_id) {
                // Clear existing records
                // users_table.clear();

                // Insert users
                for user in &users {
                    let mut record_data = [0u8; 64 + 256 + 8 + 8];
                    let mut offset = 0;

                    // username
                    memset(record_data.as_mut_ptr().add(offset), 0, 64);
                    let username_bytes = user.name.as_bytes();
                    memcpy(
                        record_data.as_mut_ptr().add(offset),
                        username_bytes.as_ptr(),
                        username_bytes.len(),
                    );
                    offset += 64;

                    // description (empty for now)
                    memset(record_data.as_mut_ptr().add(offset), 0, 256);
                    offset += 256;

                    // created_at
                    memcpy(
                        record_data.as_mut_ptr().add(offset),
                        &now as *const u64 as *const u8,
                        8,
                    );
                    offset += 8;

                    // updated_at
                    memcpy(
                        record_data.as_mut_ptr().add(offset),
                        &now as *const u64 as *const u8,
                        8,
                    );

                    let _ = users_table.insert(record_data.as_ptr());
                }
            }
        }

        // Save user roles
        if let Some(user_roles_table_id) = db.tables.iter().position(|table_opt| {
            table_opt
                .as_ref()
                .map(|table| table.def.name == SYSTEM_USER_ROLES_TABLE)
                .unwrap_or(false)
        }) {
            if let Ok(user_roles_table) = db.get_table_mut(user_roles_table_id) {
                // Clear existing records
                // user_roles_table.clear();

                // Insert user roles
                for user in &users {
                    for role_name in &user.roles {
                        let mut record_data = [0u8; 64 + 64 + 8];
                        let mut offset = 0;

                        // username
                        memset(record_data.as_mut_ptr().add(offset), 0, 64);
                        let username_bytes = user.name.as_bytes();
                        memcpy(
                            record_data.as_mut_ptr().add(offset),
                            username_bytes.as_ptr(),
                            username_bytes.len(),
                        );
                        offset += 64;

                        // role_name
                        memset(record_data.as_mut_ptr().add(offset), 0, 64);
                        let role_name_bytes = role_name.as_bytes();
                        memcpy(
                            record_data.as_mut_ptr().add(offset),
                            role_name_bytes.as_ptr(),
                            role_name_bytes.len(),
                        );
                        offset += 64;

                        // created_at
                        memcpy(
                            record_data.as_mut_ptr().add(offset),
                            &now as *const u64 as *const u8,
                            8,
                        );

                        let _ = user_roles_table.insert(record_data.as_ptr());
                    }
                }
            }
        }

        Ok(())
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
