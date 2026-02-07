//! Session state enumerations.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    #[default]
    Created,
    Active,
    WaitingForTools,
    Completed,
    Failed,
    Cancelled,
}

impl SessionState {
    /// Parse from string with lenient matching (case-insensitive, accepts common aliases).
    pub fn from_str_lenient(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "active" => Self::Active,
            "waitingfortools" | "waiting_for_tools" => Self::WaitingForTools,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            "cancelled" | "canceled" => Self::Cancelled,
            _ => Self::Created,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionType {
    #[default]
    Main,
    Subagent {
        agent_type: String,
        description: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_state_from_str_lenient() {
        assert_eq!(
            SessionState::from_str_lenient("active"),
            SessionState::Active
        );
        assert_eq!(
            SessionState::from_str_lenient("waitingfortools"),
            SessionState::WaitingForTools
        );
        assert_eq!(
            SessionState::from_str_lenient("waiting_for_tools"),
            SessionState::WaitingForTools
        );
        assert_eq!(
            SessionState::from_str_lenient("completed"),
            SessionState::Completed
        );
        assert_eq!(
            SessionState::from_str_lenient("failed"),
            SessionState::Failed
        );
        assert_eq!(
            SessionState::from_str_lenient("cancelled"),
            SessionState::Cancelled
        );
        assert_eq!(
            SessionState::from_str_lenient("canceled"),
            SessionState::Cancelled
        );
        assert_eq!(
            SessionState::from_str_lenient("unknown"),
            SessionState::Created
        );
    }
}
