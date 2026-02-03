//! User module
//! 
//! This module defines the User struct for RBAC.

use std::string::String;
use std::vec::Vec;

/// User struct for RBAC
#[derive(Debug, Clone)]
pub struct User {
    /// User name
    pub name: String,
    /// Roles assigned to the user
    pub roles: Vec<String>,
}

impl User {
    /// Create a new user
    pub fn new(name: String) -> Self {
        Self {
            name,
            roles: Vec::new(),
        }
    }

    /// Add a role to the user
    pub fn add_role(&mut self, role_name: String) {
        if !self.roles.contains(&role_name) {
            self.roles.push(role_name);
        }
    }

    /// Remove a role from the user
    pub fn remove_role(&mut self, role_name: &str) {
        self.roles.retain(|r| r != role_name);
    }

    /// Check if the user has a specific role
    pub fn has_role(&self, role_name: &str) -> bool {
        self.roles.contains(&role_name.to_string())
    }
}