//! Security policy configuration.

use crate::permissions::{PermissionMode, PermissionPolicy};

#[derive(Debug, Clone)]
pub struct SecurityPolicy {
    pub permission: PermissionPolicy,
    pub allow_sandbox_bypass: bool,
    pub max_symlink_depth: u8,
}

impl SecurityPolicy {
    pub fn new(permission: PermissionPolicy) -> Self {
        Self {
            permission,
            allow_sandbox_bypass: false,
            max_symlink_depth: 10,
        }
    }

    pub fn permissive() -> Self {
        Self {
            permission: PermissionPolicy::permissive(),
            allow_sandbox_bypass: true,
            max_symlink_depth: 255,
        }
    }

    pub fn strict() -> Self {
        Self {
            permission: PermissionPolicy::new(),
            allow_sandbox_bypass: false,
            max_symlink_depth: 5,
        }
    }

    pub fn permission(mut self, policy: PermissionPolicy) -> Self {
        self.permission = policy;
        self
    }

    pub fn sandbox_bypass(mut self, allow: bool) -> Self {
        self.allow_sandbox_bypass = allow;
        self
    }

    pub fn symlink_depth(mut self, depth: u8) -> Self {
        self.max_symlink_depth = depth;
        self
    }

    pub fn can_bypass_sandbox(&self) -> bool {
        self.allow_sandbox_bypass
    }

    pub fn mode(&self) -> PermissionMode {
        self.permission.mode
    }
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self::new(PermissionPolicy::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_policy() {
        let policy = SecurityPolicy::default();
        assert!(!policy.allow_sandbox_bypass);
        assert_eq!(policy.max_symlink_depth, 10);
    }

    #[test]
    fn test_permissive_policy() {
        let policy = SecurityPolicy::permissive();
        assert!(policy.allow_sandbox_bypass);
    }

    #[test]
    fn test_strict_policy() {
        let policy = SecurityPolicy::strict();
        assert!(!policy.allow_sandbox_bypass);
        assert_eq!(policy.max_symlink_depth, 5);
    }
}
