//! Role module
//!
//! This module defines the Role struct for RBAC.

use std::string::String;
use std::vec::Vec;

use crate::rbac::permission::Permission;

/// Role struct for RBAC
#[derive(Debug, Clone)]
pub struct Role {
    /// Role name
    pub name: String,
    /// Permissions associated with the role
    pub permissions: Vec<(Permission, Option<String>, Option<String>)>, // (permission, table_name, column_name)
}

impl Role {
    /// Create a new role
    pub fn new(name: String) -> Self {
        Self {
            name,
            permissions: Vec::new(),
        }
    }

    /// Add a permission to the role
    pub fn add_permission(
        &mut self,
        permission: Permission,
        table_name: Option<String>,
        column_name: Option<String>,
    ) {
        self.permissions.push((permission, table_name, column_name));
    }

    /// Remove a permission from the role
    pub fn remove_permission(
        &mut self,
        permission: &Permission,
        table_name: &Option<String>,
        column_name: &Option<String>,
    ) {
        self.permissions
            .retain(|(p, t, c)| p != permission || t != table_name || c != column_name);
    }

    /// Check if the role has a specific permission
    pub fn has_permission(
        &self,
        permission: &Permission,
        table_name: &Option<String>,
        column_name: &Option<String>,
    ) -> bool {
        self.permissions.iter().any(|(p, t, c)| {
            p == permission
                && (t.is_none() || table_name.is_none() || t == table_name)
                && (c.is_none() || column_name.is_none() || c == column_name)
        })
    }
}
