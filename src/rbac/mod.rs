//! RBAC (Role-Based Access Control) module
//! 
//! This module implements role-based access control for the remdb database,
//! including role creation, permission granting, and user-role assignment.

pub mod manager;
pub mod permission;
pub mod role;
pub mod user;

pub use manager::RbacManager;
pub use permission::Permission;
pub use role::Role;
pub use user::User;