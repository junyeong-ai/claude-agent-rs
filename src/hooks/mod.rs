//! Hook system for intercepting agent execution.

mod command;
mod manager;
mod traits;

pub use command::CommandHook;
pub use manager::HookManager;
pub use traits::{
    FnHook, FnHookBuilder, Hook, HookContext, HookEvent, HookEventData, HookInput, HookMetadata,
    HookOutput,
};
