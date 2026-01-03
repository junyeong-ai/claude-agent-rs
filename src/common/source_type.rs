use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceType {
    Builtin,
    #[default]
    User,
    Project,
    Managed,
}

impl std::fmt::Display for SourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Builtin => write!(f, "builtin"),
            Self::User => write!(f, "user"),
            Self::Project => write!(f, "project"),
            Self::Managed => write!(f, "managed"),
        }
    }
}

impl SourceType {
    pub fn from_str_opt(s: Option<&str>) -> Self {
        match s {
            Some("builtin") => Self::Builtin,
            Some("project") => Self::Project,
            Some("managed") => Self::Managed,
            _ => Self::User,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        assert_eq!(SourceType::default(), SourceType::User);
    }

    #[test]
    fn test_display() {
        assert_eq!(SourceType::Builtin.to_string(), "builtin");
        assert_eq!(SourceType::User.to_string(), "user");
        assert_eq!(SourceType::Project.to_string(), "project");
        assert_eq!(SourceType::Managed.to_string(), "managed");
    }

    #[test]
    fn test_from_str_opt() {
        assert_eq!(
            SourceType::from_str_opt(Some("builtin")),
            SourceType::Builtin
        );
        assert_eq!(
            SourceType::from_str_opt(Some("project")),
            SourceType::Project
        );
        assert_eq!(
            SourceType::from_str_opt(Some("managed")),
            SourceType::Managed
        );
        assert_eq!(SourceType::from_str_opt(Some("user")), SourceType::User);
        assert_eq!(SourceType::from_str_opt(None), SourceType::User);
        assert_eq!(SourceType::from_str_opt(Some("unknown")), SourceType::User);
    }

    #[test]
    fn test_serde() {
        let json = serde_json::to_string(&SourceType::Builtin).unwrap();
        assert_eq!(json, "\"builtin\"");

        let parsed: SourceType = serde_json::from_str("\"project\"").unwrap();
        assert_eq!(parsed, SourceType::Project);
    }
}
