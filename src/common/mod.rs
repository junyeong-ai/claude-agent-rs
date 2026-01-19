mod content_source;
mod directory;
mod file_provider;
mod frontmatter;
mod index;
mod index_registry;
mod path_matched;
mod provider;
mod source_type;

use std::path::PathBuf;

pub use content_source::ContentSource;
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
pub use frontmatter::{ParsedDocument, parse_frontmatter, strip_frontmatter};
pub use index::Index;
pub use index_registry::{IndexRegistry, LoadedEntry};
pub use path_matched::PathMatched;
pub use provider::{ChainProvider, InMemoryProvider, Provider};
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
