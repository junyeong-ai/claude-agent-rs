//! Bash command security analysis using tree-sitter AST parsing.

mod env;
mod parser;

pub use env::SanitizedEnv;
pub use parser::{BashAnalysis, BashAnalyzer, BashPolicy, ReferencedPath, SecurityConcern};
