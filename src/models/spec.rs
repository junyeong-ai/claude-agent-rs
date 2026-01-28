use super::family::ModelFamily;
use super::provider::{ProviderIds, ProviderKind};
use serde::{Deserialize, Serialize};

pub type ModelId = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSpec {
    pub id: ModelId,
    pub family: ModelFamily,
    pub version: ModelVersion,
    pub capabilities: Capabilities,
    pub pricing: Pricing,
    pub provider_ids: ProviderIds,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelVersion {
    pub version: String,
    pub snapshot: Option<String>,
    pub knowledge_cutoff: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Capabilities {
    pub context_window: u64,
    pub extended_context_window: Option<u64>,
    pub max_output_tokens: u64,
    pub thinking: bool,
    pub vision: bool,
    pub tool_use: bool,
    pub caching: bool,
}

impl Capabilities {
    pub fn effective_context(&self, extended_enabled: bool) -> u64 {
        if extended_enabled {
            self.extended_context_window.unwrap_or(self.context_window)
        } else {
            self.context_window
        }
    }

    pub fn supports_extended_context(&self) -> bool {
        self.extended_context_window.is_some()
    }
}

pub const LONG_CONTEXT_THRESHOLD: u64 = 200_000;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Pricing {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
    pub long_context_multiplier: f64,
}

impl Pricing {
    pub fn new(input: f64, output: f64) -> Self {
        Self {
            input,
            output,
            cache_read: input * 0.1,
            cache_write: input * 1.25,
            long_context_multiplier: 2.0,
        }
    }

    pub fn calculate(
        &self,
        input_tokens: u64,
        output_tokens: u64,
        cache_read: u64,
        cache_write: u64,
    ) -> f64 {
        let context = input_tokens + cache_read + cache_write;
        let multiplier = if context > LONG_CONTEXT_THRESHOLD {
            self.long_context_multiplier
        } else {
            1.0
        };

        let input_cost = (input_tokens as f64 / 1_000_000.0) * self.input * multiplier;
        let output_cost = (output_tokens as f64 / 1_000_000.0) * self.output;
        let cache_read_cost = (cache_read as f64 / 1_000_000.0) * self.cache_read * multiplier;
        let cache_write_cost = (cache_write as f64 / 1_000_000.0) * self.cache_write * multiplier;

        input_cost + output_cost + cache_read_cost + cache_write_cost
    }
}

impl ModelSpec {
    pub fn provider_id(&self, provider: ProviderKind) -> Option<&str> {
        self.provider_ids.for_provider(provider)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pricing_standard() {
        let pricing = Pricing::new(3.0, 15.0);
        // Standard pricing: context < 200K
        let cost = pricing.calculate(100_000, 100_000, 0, 0);
        // input: 0.1 * 3.0 = 0.3, output: 0.1 * 15.0 = 1.5
        assert!((cost - 1.8).abs() < 0.01);
    }

    #[test]
    fn test_pricing_large_volume() {
        let pricing = Pricing::new(3.0, 15.0);
        // 1M tokens each, context = 1M > 200K, so 2x multiplier on input
        let cost = pricing.calculate(1_000_000, 1_000_000, 0, 0);
        // input: 1.0 * 3.0 * 2.0 = 6.0, output: 1.0 * 15.0 = 15.0
        assert!((cost - 21.0).abs() < 0.01);
    }

    #[test]
    fn test_pricing_long_context() {
        let pricing = Pricing::new(3.0, 15.0);
        let cost = pricing.calculate(250_000, 0, 0, 0);
        let expected = (250_000.0 / 1_000_000.0) * 3.0 * 2.0;
        assert!((cost - expected).abs() < 0.01);
    }

    #[test]
    fn test_effective_context() {
        let caps = Capabilities {
            context_window: 200_000,
            extended_context_window: Some(1_000_000),
            max_output_tokens: 64_000,
            thinking: true,
            vision: true,
            tool_use: true,
            caching: true,
        };

        assert_eq!(caps.effective_context(false), 200_000);
        assert_eq!(caps.effective_context(true), 1_000_000);
    }
}
