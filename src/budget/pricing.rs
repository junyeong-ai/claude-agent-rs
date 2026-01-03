//! Model pricing definitions for cost calculation.
//!
//! Prices can be customized via environment variables or programmatically.
//! Default prices are based on Anthropic's published rates.

use std::collections::HashMap;
use std::sync::LazyLock;

const CACHE_READ_DISCOUNT: f64 = 0.1;
const CACHE_WRITE_PREMIUM: f64 = 1.25;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelPricing {
    pub input_per_mtok: f64,
    pub output_per_mtok: f64,
    pub cache_read_per_mtok: f64,
    pub cache_write_per_mtok: f64,
}

impl ModelPricing {
    pub const fn new(
        input_per_mtok: f64,
        output_per_mtok: f64,
        cache_read_per_mtok: f64,
        cache_write_per_mtok: f64,
    ) -> Self {
        Self {
            input_per_mtok,
            output_per_mtok,
            cache_read_per_mtok,
            cache_write_per_mtok,
        }
    }

    pub fn from_base(input_per_mtok: f64, output_per_mtok: f64) -> Self {
        Self {
            input_per_mtok,
            output_per_mtok,
            cache_read_per_mtok: input_per_mtok * CACHE_READ_DISCOUNT,
            cache_write_per_mtok: input_per_mtok * CACHE_WRITE_PREMIUM,
        }
    }

    pub fn calculate(&self, usage: &crate::types::Usage) -> f64 {
        let input = (usage.input_tokens as f64 / 1_000_000.0) * self.input_per_mtok;
        let output = (usage.output_tokens as f64 / 1_000_000.0) * self.output_per_mtok;
        let cache_read = (usage.cache_read_input_tokens.unwrap_or(0) as f64 / 1_000_000.0)
            * self.cache_read_per_mtok;
        let cache_write = (usage.cache_creation_input_tokens.unwrap_or(0) as f64 / 1_000_000.0)
            * self.cache_write_per_mtok;
        input + output + cache_read + cache_write
    }
}

#[derive(Debug, Clone)]
pub struct PricingTable {
    models: HashMap<String, ModelPricing>,
    default: ModelPricing,
}

impl PricingTable {
    pub fn builder() -> PricingTableBuilder {
        PricingTableBuilder::new()
    }

    pub fn get(&self, model: &str) -> &ModelPricing {
        let normalized = Self::normalize_model_name(model);
        self.models.get(&normalized).unwrap_or(&self.default)
    }

    pub fn calculate(&self, model: &str, usage: &crate::types::Usage) -> f64 {
        self.get(model).calculate(usage)
    }

    fn normalize_model_name(model: &str) -> String {
        let model = model.to_lowercase();
        if model.contains("opus") {
            "opus".to_string()
        } else if model.contains("sonnet") {
            "sonnet".to_string()
        } else if model.contains("haiku") {
            "haiku".to_string()
        } else {
            model
        }
    }
}

impl Default for PricingTable {
    fn default() -> Self {
        global_pricing_table().clone()
    }
}

#[derive(Debug, Default)]
pub struct PricingTableBuilder {
    models: HashMap<String, ModelPricing>,
    default: Option<ModelPricing>,
}

impl PricingTableBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_defaults(mut self) -> Self {
        self.models
            .insert("opus".into(), ModelPricing::new(15.0, 75.0, 1.5, 18.75));
        self.models
            .insert("sonnet".into(), ModelPricing::new(3.0, 15.0, 0.3, 3.75));
        self.models
            .insert("haiku".into(), ModelPricing::new(0.80, 4.0, 0.08, 1.0));
        self
    }

    pub fn model(mut self, name: impl Into<String>, pricing: ModelPricing) -> Self {
        self.models.insert(name.into(), pricing);
        self
    }

    pub fn model_base(self, name: impl Into<String>, input: f64, output: f64) -> Self {
        self.model(name, ModelPricing::from_base(input, output))
    }

    pub fn default_pricing(mut self, pricing: ModelPricing) -> Self {
        self.default = Some(pricing);
        self
    }

    pub fn from_env(mut self) -> Self {
        self = self.with_defaults();

        if let Some(pricing) = Self::parse_env_pricing("OPUS") {
            self.models.insert("opus".into(), pricing);
        }
        if let Some(pricing) = Self::parse_env_pricing("SONNET") {
            self.models.insert("sonnet".into(), pricing);
        }
        if let Some(pricing) = Self::parse_env_pricing("HAIKU") {
            self.models.insert("haiku".into(), pricing);
        }

        self
    }

    fn parse_env_pricing(model: &str) -> Option<ModelPricing> {
        let input = std::env::var(format!("ANTHROPIC_PRICING_{}_INPUT", model))
            .ok()?
            .parse::<f64>()
            .ok()?;
        let output = std::env::var(format!("ANTHROPIC_PRICING_{}_OUTPUT", model))
            .ok()?
            .parse::<f64>()
            .ok()?;

        let cache_read = std::env::var(format!("ANTHROPIC_PRICING_{}_CACHE_READ", model))
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(input * CACHE_READ_DISCOUNT);
        let cache_write = std::env::var(format!("ANTHROPIC_PRICING_{}_CACHE_WRITE", model))
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(input * CACHE_WRITE_PREMIUM);

        Some(ModelPricing::new(input, output, cache_read, cache_write))
    }

    pub fn build(self) -> PricingTable {
        let default = self
            .default
            .or_else(|| self.models.get("sonnet").copied())
            .unwrap_or(ModelPricing::new(3.0, 15.0, 0.3, 3.75));

        PricingTable {
            models: self.models,
            default,
        }
    }
}

static GLOBAL_PRICING: LazyLock<PricingTable> =
    LazyLock::new(|| PricingTableBuilder::new().from_env().build());

pub fn global_pricing_table() -> &'static PricingTable {
    &GLOBAL_PRICING
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Usage;

    #[test]
    fn test_pricing_calculation() {
        let usage = Usage {
            input_tokens: 1_000_000,
            output_tokens: 1_000_000,
            cache_read_input_tokens: None,
            cache_creation_input_tokens: None,
            ..Default::default()
        };

        let table = global_pricing_table();

        let cost = table.calculate("claude-sonnet-4-5", &usage);
        assert!((cost - 18.0).abs() < 0.01);

        let cost = table.calculate("claude-opus-4-5", &usage);
        assert!((cost - 90.0).abs() < 0.01);

        let cost = table.calculate("claude-3-5-haiku", &usage);
        assert!((cost - 4.8).abs() < 0.01);
    }

    #[test]
    fn test_cache_pricing() {
        let usage = Usage {
            input_tokens: 1_000_000,
            output_tokens: 100_000,
            cache_read_input_tokens: Some(500_000),
            cache_creation_input_tokens: Some(200_000),
            ..Default::default()
        };

        let table = global_pricing_table();
        let cost = table.calculate("claude-sonnet-4-5", &usage);
        assert!((cost - 5.40).abs() < 0.01);
    }

    #[test]
    fn test_custom_pricing_table() {
        let table = PricingTableBuilder::new()
            .model_base("custom", 10.0, 50.0)
            .default_pricing(ModelPricing::from_base(10.0, 50.0))
            .build();

        let usage = Usage {
            input_tokens: 1_000_000,
            output_tokens: 1_000_000,
            ..Default::default()
        };

        let cost = table.calculate("custom", &usage);
        assert!((cost - 60.0).abs() < 0.01);
    }

    #[test]
    fn test_from_base_pricing() {
        let pricing = ModelPricing::from_base(10.0, 50.0);
        assert!((pricing.cache_read_per_mtok - 1.0).abs() < 0.01);
        assert!((pricing.cache_write_per_mtok - 12.5).abs() < 0.01);
    }
}
