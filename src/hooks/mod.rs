//! Hook system for intercepting agent execution.

mod manager;
mod traits;

pub use manager::HookManager;
pub use traits::{Hook, HookContext, HookEvent, HookInput, HookOutput};

use crate::permissions::PermissionDecision;

/// Convert a hook output permission decision to a permission result
pub fn apply_hook_decision(
    decision: Option<PermissionDecision>,
    reason: Option<String>,
) -> Option<crate::permissions::PermissionResult> {
    decision.map(|d| match d {
        PermissionDecision::Allow => {
            crate::permissions::PermissionResult::allowed(reason.unwrap_or_default())
        }
        PermissionDecision::Deny => {
            crate::permissions::PermissionResult::denied(reason.unwrap_or_default())
        }
        PermissionDecision::Ask => {
            crate::permissions::PermissionResult::ask(reason.unwrap_or_default())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_hook_decision() {
        let result = apply_hook_decision(Some(PermissionDecision::Allow), Some("test".into()));
        assert!(result.unwrap().is_allowed());

        let result = apply_hook_decision(Some(PermissionDecision::Deny), Some("test".into()));
        assert!(result.unwrap().is_denied());

        let result = apply_hook_decision(None, None);
        assert!(result.is_none());
    }
}
