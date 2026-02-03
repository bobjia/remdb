//! Permission module
//! 
//! This module defines the Permission enum for RBAC.

use alloc::string::String;

/// Permission types for RBAC
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Permission {
    /// Select permission (read access)
    Select,
    /// Insert permission (create access)
    Insert,
    /// Update permission (modify access)
    Update,
    /// Delete permission (remove access)
    Delete,
}

impl Permission {
    /// Parse permission from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "SELECT" => Some(Permission::Select),
            "INSERT" => Some(Permission::Insert),
            "UPDATE" => Some(Permission::Update),
            "DELETE" => Some(Permission::Delete),
            _ => None,
        }
    }

    /// Convert permission to string
    pub fn to_string(&self) -> String {
        match self {
            Permission::Select => "SELECT".to_string(),
            Permission::Insert => "INSERT".to_string(),
            Permission::Update => "UPDATE".to_string(),
            Permission::Delete => "DELETE".to_string(),
        }
    }
}