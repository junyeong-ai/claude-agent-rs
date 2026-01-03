mod directory;
mod file_provider;
mod frontmatter;
mod provider;
mod registry;
mod source_type;

use std::path::PathBuf;

pub use directory::{is_markdown, is_skill_file, load_files};

/// Get the user's home directory.
///
/// Uses `directories` crate for cross-platform compatibility.
pub fn home_dir() -> Option<PathBuf> {
    directories::UserDirs::new().map(|d| d.home_dir().to_path_buf())
}
pub use file_provider::{
    DocumentLoader, FileProvider, LookupStrategy, OutputStyleLookupStrategy, SkillLookupStrategy,
    SubagentLookupStrategy,
};
pub use frontmatter::{ParsedDocument, parse_frontmatter};
pub use provider::{ChainProvider, InMemoryProvider, Provider};
pub use registry::{BaseRegistry, RegistryItem};
pub use source_type::SourceType;

pub trait Named {
    fn name(&self) -> &str;
}

pub trait ToolRestricted {
    fn allowed_tools(&self) -> &[String];

    fn has_tool_restrictions(&self) -> bool {
        !self.allowed_tools().is_empty()
    }

    fn is_tool_allowed(&self, tool_name: &str) -> bool {
        crate::tools::is_tool_allowed(self.allowed_tools(), tool_name)
    }
}
