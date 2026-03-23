//! Synapsis Permission System
//!
//! Implements fine-grained permissions and trust levels for agent access control.
//!
//! # Trust Levels
//!
//! Trust increases from `Zero` (blocked) to `Firmware` (TPM-verified).
//!
//! # Permissions
//!
//! Permissions are organized by resource type and capability level.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Permission {
    ReadContext,
    WriteContext,
    CreateTask,
    AssignTask,
    ReadTasks,
    ExecuteTask,
    DeleteTask,
    ReadRecycleBin,
    WriteRecycleBin,
    SearchRecycleBin,
    PurgeRecycleBin,
    ManageAgents,
    ManageApiKeys,
    ViewAuditLog,
    PqcEncrypt,
    PqcDecrypt,
    ManageSessions,
    ConfigureSecurity,
    Admin,
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Permission::ReadContext => write!(f, "ReadContext"),
            Permission::WriteContext => write!(f, "WriteContext"),
            Permission::CreateTask => write!(f, "CreateTask"),
            Permission::AssignTask => write!(f, "AssignTask"),
            Permission::ReadTasks => write!(f, "ReadTasks"),
            Permission::ExecuteTask => write!(f, "ExecuteTask"),
            Permission::DeleteTask => write!(f, "DeleteTask"),
            Permission::ReadRecycleBin => write!(f, "ReadRecycleBin"),
            Permission::WriteRecycleBin => write!(f, "WriteRecycleBin"),
            Permission::SearchRecycleBin => write!(f, "SearchRecycleBin"),
            Permission::PurgeRecycleBin => write!(f, "PurgeRecycleBin"),
            Permission::ManageAgents => write!(f, "ManageAgents"),
            Permission::ManageApiKeys => write!(f, "ManageApiKeys"),
            Permission::ViewAuditLog => write!(f, "ViewAuditLog"),
            Permission::PqcEncrypt => write!(f, "PqcEncrypt"),
            Permission::PqcDecrypt => write!(f, "PqcDecrypt"),
            Permission::ManageSessions => write!(f, "ManageSessions"),
            Permission::ConfigureSecurity => write!(f, "ConfigureSecurity"),
            Permission::Admin => write!(f, "Admin"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TrustLevel {
    Zero = 0,
    Minimal = 1,
    Basic = 2,
    Trusted = 3,
    Firmware = 4,
}

impl fmt::Display for TrustLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrustLevel::Zero => write!(f, "Zero"),
            TrustLevel::Minimal => write!(f, "Minimal"),
            TrustLevel::Basic => write!(f, "Basic"),
            TrustLevel::Trusted => write!(f, "Trusted"),
            TrustLevel::Firmware => write!(f, "Firmware"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionSet {
    pub permissions: BTreeSet<Permission>,
    pub max_trust_level: TrustLevel,
    pub session_timeout: u64,
    pub can_delegate: bool,
}

impl Default for PermissionSet {
    fn default() -> Self {
        Self::none()
    }
}

impl PermissionSet {
    pub fn none() -> Self {
        Self {
            permissions: BTreeSet::new(),
            max_trust_level: TrustLevel::Zero,
            session_timeout: 0,
            can_delegate: false,
        }
    }

    pub fn all() -> Self {
        Self {
            permissions: BTreeSet::from([
                Permission::ReadContext,
                Permission::WriteContext,
                Permission::CreateTask,
                Permission::AssignTask,
                Permission::ReadTasks,
                Permission::ExecuteTask,
                Permission::DeleteTask,
                Permission::ReadRecycleBin,
                Permission::WriteRecycleBin,
                Permission::SearchRecycleBin,
                Permission::PurgeRecycleBin,
                Permission::ManageAgents,
                Permission::ManageApiKeys,
                Permission::ViewAuditLog,
                Permission::PqcEncrypt,
                Permission::PqcDecrypt,
                Permission::ManageSessions,
                Permission::ConfigureSecurity,
                Permission::Admin,
            ]),
            max_trust_level: TrustLevel::Firmware,
            session_timeout: 86400,
            can_delegate: true,
        }
    }

    pub fn basic() -> Self {
        Self {
            permissions: BTreeSet::from([
                Permission::ReadContext,
                Permission::ReadTasks,
                Permission::PqcEncrypt,
            ]),
            max_trust_level: TrustLevel::Basic,
            session_timeout: 3600,
            can_delegate: false,
        }
    }

    pub fn trusted() -> Self {
        Self {
            permissions: BTreeSet::from([
                Permission::ReadContext,
                Permission::WriteContext,
                Permission::CreateTask,
                Permission::ReadTasks,
                Permission::ExecuteTask,
                Permission::ReadRecycleBin,
                Permission::SearchRecycleBin,
                Permission::PqcEncrypt,
                Permission::PqcDecrypt,
            ]),
            max_trust_level: TrustLevel::Trusted,
            session_timeout: 43200,
            can_delegate: true,
        }
    }

    pub fn minimal() -> Self {
        Self {
            permissions: BTreeSet::from([Permission::ReadContext]),
            max_trust_level: TrustLevel::Minimal,
            session_timeout: 1800,
            can_delegate: false,
        }
    }

    pub fn has_permission(&self, permission: Permission) -> bool {
        if self.permissions.contains(&Permission::Admin) {
            return true;
        }
        self.permissions.contains(&permission)
    }

    pub fn grant(&mut self, permission: Permission) {
        if self.max_trust_level != TrustLevel::Zero {
            self.permissions.insert(permission);
        }
    }

    pub fn revoke(&mut self, permission: Permission) {
        if permission != Permission::Admin {
            self.permissions.remove(&permission);
        }
    }

    pub fn is_admin(&self) -> bool {
        self.permissions.contains(&Permission::Admin)
    }

    pub fn can_encrypt(&self) -> bool {
        self.has_permission(Permission::PqcEncrypt)
    }

    pub fn can_decrypt(&self) -> bool {
        self.has_permission(Permission::PqcDecrypt)
    }

    pub fn can_manage_agents(&self) -> bool {
        self.has_permission(Permission::ManageAgents)
    }

    pub fn can_configure_security(&self) -> bool {
        self.has_permission(Permission::ConfigureSecurity)
    }

    pub fn can_access_recycle_bin(&self) -> bool {
        self.has_permission(Permission::ReadRecycleBin)
            || self.has_permission(Permission::SearchRecycleBin)
    }

    pub fn can_write_recycle_bin(&self) -> bool {
        self.has_permission(Permission::WriteRecycleBin)
    }
}

pub struct PermissionChecker<'a> {
    permission_set: &'a PermissionSet,
}

impl<'a> PermissionChecker<'a> {
    pub fn new(permission_set: &'a PermissionSet) -> Self {
        Self { permission_set }
    }

    pub fn check(&self, permission: Permission) -> Result<(), PermissionDenied> {
        if self.permission_set.has_permission(permission) {
            Ok(())
        } else {
            Err(PermissionDenied(permission))
        }
    }

    pub fn check_any(&self, permissions: &[Permission]) -> Result<(), PermissionDenied> {
        for &perm in permissions {
            if self.permission_set.has_permission(perm) {
                return Ok(());
            }
        }
        Err(PermissionDenied(permissions[0]))
    }

    pub fn check_all(&self, permissions: &[Permission]) -> Result<(), PermissionDenied> {
        for &perm in permissions {
            self.check(perm)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct PermissionDenied(pub Permission);

impl fmt::Display for PermissionDenied {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Permission denied: {}", self.0)
    }
}

impl std::error::Error for PermissionDenied {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_set_none() {
        let perms = PermissionSet::none();
        assert!(!perms.has_permission(Permission::ReadContext));
        assert_eq!(perms.max_trust_level, TrustLevel::Zero);
    }

    #[test]
    fn test_permission_set_all() {
        let perms = PermissionSet::all();
        assert!(perms.has_permission(Permission::ReadContext));
        assert!(perms.has_permission(Permission::Admin));
        assert_eq!(perms.max_trust_level, TrustLevel::Firmware);
    }

    #[test]
    fn test_permission_grant_revoke() {
        let mut perms = PermissionSet::minimal();
        assert!(!perms.has_permission(Permission::WriteContext));

        perms.grant(Permission::WriteContext);
        assert!(perms.has_permission(Permission::WriteContext));

        perms.revoke(Permission::WriteContext);
        assert!(!perms.has_permission(Permission::WriteContext));
    }

    #[test]
    fn test_admin_has_all_permissions() {
        let mut perms = PermissionSet::minimal();
        perms.grant(Permission::Admin);

        assert!(perms.has_permission(Permission::ReadContext));
        assert!(perms.has_permission(Permission::WriteContext));
        assert!(perms.has_permission(Permission::DeleteTask));
    }

    #[test]
    fn test_cannot_revoke_admin() {
        let mut perms = PermissionSet::all();
        perms.revoke(Permission::Admin);
        assert!(perms.has_permission(Permission::Admin));
    }
}
