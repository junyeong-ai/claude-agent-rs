mod builtin;
mod family;
mod provider;
mod registry;
mod spec;

pub use family::{ModelFamily, ModelRole};
pub use provider::{ProviderIds, ProviderKind};
pub use registry::{ModelRegistry, read_registry, registry};
pub use spec::{Capabilities, LONG_CONTEXT_THRESHOLD, ModelId, ModelSpec, ModelVersion, Pricing};

pub const DEFAULT_COMPACT_THRESHOLD: f32 = 0.8;

pub mod context_window {
    use super::read_registry;

    pub const STANDARD: u64 = 200_000;
    pub const EXTENDED: u64 = 1_000_000;
    pub const DEFAULT: u64 = 128_000;

    pub fn for_model(model: &str) -> u64 {
        read_registry()
            .resolve(model)
            .map(|spec| spec.capabilities.context_window)
            .unwrap_or(DEFAULT)
    }

    pub fn for_model_extended(model: &str, extended_enabled: bool) -> u64 {
        read_registry()
            .resolve(model)
            .map(|spec| spec.capabilities.effective_context(extended_enabled))
            .unwrap_or(DEFAULT)
    }
}

pub mod output_tokens {
    use super::read_registry;

    pub const DEFAULT: u64 = 8_192;

    pub fn for_model(model: &str) -> u64 {
        read_registry()
            .resolve(model)
            .map(|spec| spec.capabilities.max_output_tokens)
            .unwrap_or(DEFAULT)
    }
}
