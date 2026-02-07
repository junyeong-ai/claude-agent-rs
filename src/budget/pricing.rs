//! Model pricing definitions for cost calculation.
//!
//! Prices can be customized via environment variables or programmatically.
//! Default prices are based on Anthropic's published rates.
//!
//! Uses `rust_decimal` for precise monetary calculations without floating-point errors.

use std::collections::HashMap;
use std::sync::LazyLock;

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

use crate::models::LONG_CONTEXT_THRESHOLD;

const CACHE_READ_DISCOUNT: Decimal = dec!(0.1);
const CACHE_WRITE_PREMIUM: Decimal = dec!(1.25);
const DEFAULT_LONG_CONTEXT_MULTIPLIER: Decimal = dec!(2);
const MILLION: Decimal = dec!(1_000_000);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelPricing {
    pub input_per_mtok: Decimal,
    pub output_per_mtok: Decimal,
    pub cache_read_per_mtok: Decimal,
    pub cache_write_per_mtok: Decimal,
    pub long_context_multiplier: Decimal,
}

impl ModelPricing {
    pub const fn new(
        input_per_mtok: Decimal,
        output_per_mtok: Decimal,
        cache_read_per_mtok: Decimal,
        cache_write_per_mtok: Decimal,
        long_context_multiplier: Decimal,
    ) -> Self {
        Self {
            input_per_mtok,
            output_per_mtok,
            cache_read_per_mtok,
            cache_write_per_mtok,
            long_context_multiplier,
        }
    }

    pub fn from_base(input_per_mtok: Decimal, output_per_mtok: Decimal) -> Self {
        Self {
            input_per_mtok,
            output_per_mtok,
            cache_read_per_mtok: input_per_mtok * CACHE_READ_DISCOUNT,
            cache_write_per_mtok: input_per_mtok * CACHE_WRITE_PREMIUM,
            long_context_multiplier: DEFAULT_LONG_CONTEXT_MULTIPLIER,
        }
    }

    /// Calculate cost from raw token counts.
    pub fn calculate_raw(
        &self,
        input_tokens: u64,
        output_tokens: u64,
        cache_read: u64,
        cache_write: u64,
    ) -> Decimal {
        let context = input_tokens + cache_read + cache_write;
        let multiplier = if context > LONG_CONTEXT_THRESHOLD {
            self.long_context_multiplier
        } else {
            Decimal::ONE
        };

        let input = (Decimal::from(input_tokens) / MILLION) * self.input_per_mtok * multiplier;
        let output = (Decimal::from(output_tokens) / MILLION) * self.output_per_mtok;
        let cache_read_cost =
            (Decimal::from(cache_read) / MILLION) * self.cache_read_per_mtok * multiplier;
        let cache_write_cost =
            (Decimal::from(cache_write) / MILLION) * self.cache_write_per_mtok * multiplier;

        input + output + cache_read_cost + cache_write_cost
    }

    pub fn calculate(&self, usage: &crate::types::Usage) -> Decimal {
        self.calculate_raw(
            usage.input_tokens as u64,
            usage.output_tokens as u64,
            usage.cache_read_input_tokens.unwrap_or(0) as u64,
            usage.cache_creation_input_tokens.unwrap_or(0) as u64,
        )
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

    pub fn calculate(&self, model: &str, usage: &crate::types::Usage) -> Decimal {
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

    pub fn defaults(mut self) -> Self {
        self.models.insert(
            "opus".into(),
            ModelPricing::new(dec!(15), dec!(75), dec!(1.5), dec!(18.75), dec!(2)),
        );
        self.models.insert(
            "sonnet".into(),
            ModelPricing::new(dec!(3), dec!(15), dec!(0.3), dec!(3.75), dec!(2)),
        );
        self.models.insert(
            "haiku".into(),
            ModelPricing::new(dec!(0.80), dec!(4), dec!(0.08), dec!(1), dec!(2)),
        );
        self
    }

    pub fn model(mut self, name: impl Into<String>, pricing: ModelPricing) -> Self {
        self.models.insert(name.into(), pricing);
        self
    }

    pub fn model_base(self, name: impl Into<String>, input: Decimal, output: Decimal) -> Self {
        self.model(name, ModelPricing::from_base(input, output))
    }

    pub fn default_pricing(mut self, pricing: ModelPricing) -> Self {
        self.default = Some(pricing);
        self
    }

    pub fn from_env(mut self) -> Self {
        self = self.defaults();

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
        let input: Decimal = std::env::var(format!("ANTHROPIC_PRICING_{}_INPUT", model))
            .ok()?
            .parse()
            .ok()?;
        let output: Decimal = std::env::var(format!("ANTHROPIC_PRICING_{}_OUTPUT", model))
            .ok()?
            .parse()
            .ok()?;

        let cache_read: Decimal = std::env::var(format!("ANTHROPIC_PRICING_{}_CACHE_READ", model))
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(input * CACHE_READ_DISCOUNT);
        let cache_write: Decimal =
            std::env::var(format!("ANTHROPIC_PRICING_{}_CACHE_WRITE", model))
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(input * CACHE_WRITE_PREMIUM);

        Some(ModelPricing::new(
            input,
            output,
            cache_read,
            cache_write,
            DEFAULT_LONG_CONTEXT_MULTIPLIER,
        ))
    }

    pub fn build(self) -> PricingTable {
        let default = self
            .default
            .or_else(|| self.models.get("sonnet").copied())
            .unwrap_or(ModelPricing::new(
                dec!(3),
                dec!(15),
                dec!(0.3),
                dec!(3.75),
                dec!(2),
            ));

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
    fn test_pricing_standard_context() {
        let usage = Usage {
            input_tokens: 100_000,
            output_tokens: 100_000,
            cache_read_input_tokens: None,
            cache_creation_input_tokens: None,
            ..Default::default()
        };

        let table = global_pricing_table();

        // Sonnet: 0.1M * $3 + 0.1M * $15 = $0.3 + $1.5 = $1.8
        let cost = table.calculate("claude-sonnet-4-5", &usage);
        assert_eq!(cost, dec!(1.8));

        // Opus: 0.1M * $15 + 0.1M * $75 = $1.5 + $7.5 = $9
        let cost = table.calculate("claude-opus-4-6", &usage);
        assert_eq!(cost, dec!(9));

        // Haiku: 0.1M * $0.80 + 0.1M * $4 = $0.08 + $0.4 = $0.48
        let cost = table.calculate("claude-haiku-4-5", &usage);
        assert_eq!(cost, dec!(0.48));
    }

    #[test]
    fn test_pricing_long_context_multiplier() {
        let usage = Usage {
            input_tokens: 1_000_000,
            output_tokens: 1_000_000,
            cache_read_input_tokens: None,
            cache_creation_input_tokens: None,
            ..Default::default()
        };

        let table = global_pricing_table();

        // Sonnet long context (1M > 200K): input 1M * $3 * 2 = $6, output 1M * $15 = $15
        let cost = table.calculate("claude-sonnet-4-5", &usage);
        assert_eq!(cost, dec!(21));

        // Opus long context: input 1M * $15 * 2 = $30, output 1M * $75 = $75
        let cost = table.calculate("claude-opus-4-6", &usage);
        assert_eq!(cost, dec!(105));

        // Haiku long context: input 1M * $0.80 * 2 = $1.60, output 1M * $4 = $4
        let cost = table.calculate("claude-haiku-4-5", &usage);
        assert_eq!(cost, dec!(5.60));
    }

    #[test]
    fn test_cache_pricing() {
        let usage = Usage {
            input_tokens: 50_000,
            output_tokens: 10_000,
            cache_read_input_tokens: Some(50_000),
            cache_creation_input_tokens: Some(20_000),
            ..Default::default()
        };

        let table = global_pricing_table();
        // Standard context (120K < 200K):
        // input: 0.05M * $3 = $0.15, output: 0.01M * $15 = $0.15
        // cache_read: 0.05M * $0.3 = $0.015, cache_write: 0.02M * $3.75 = $0.075
        let cost = table.calculate("claude-sonnet-4-5", &usage);
        assert_eq!(cost, dec!(0.39));
    }

    #[test]
    fn test_cache_pricing_long_context() {
        let usage = Usage {
            input_tokens: 1_000_000,
            output_tokens: 100_000,
            cache_read_input_tokens: Some(500_000),
            cache_creation_input_tokens: Some(200_000),
            ..Default::default()
        };

        let table = global_pricing_table();
        // Long context (1.7M > 200K), 2x multiplier on input/cache_read/cache_write:
        // input: 1M * $3 * 2 = $6, output: 0.1M * $15 = $1.5
        // cache_read: 0.5M * $0.3 * 2 = $0.3, cache_write: 0.2M * $3.75 * 2 = $1.5
        let cost = table.calculate("claude-sonnet-4-5", &usage);
        assert_eq!(cost, dec!(9.3));
    }

    #[test]
    fn test_long_context_pricing() {
        let table = global_pricing_table();

        let usage = Usage {
            input_tokens: 250_000,
            output_tokens: 50_000,
            ..Default::default()
        };

        // Sonnet long context (250K > 200K): input 0.25M * $3 * 2 = $1.5, output 0.05M * $15 = $0.75
        let cost = table.calculate("claude-sonnet-4-5", &usage);
        assert_eq!(cost, dec!(2.25));
    }

    #[test]
    fn test_custom_pricing_table() {
        let table = PricingTableBuilder::new()
            .model_base("custom", dec!(10), dec!(50))
            .default_pricing(ModelPricing::from_base(dec!(10), dec!(50)))
            .build();

        let usage = Usage {
            input_tokens: 100_000,
            output_tokens: 100_000,
            ..Default::default()
        };

        let cost = table.calculate("custom", &usage);
        assert_eq!(cost, dec!(6));
    }

    #[test]
    fn test_from_base_pricing() {
        let pricing = ModelPricing::from_base(dec!(10), dec!(50));
        assert_eq!(pricing.cache_read_per_mtok, dec!(1));
        assert_eq!(pricing.cache_write_per_mtok, dec!(12.5));
        assert_eq!(pricing.long_context_multiplier, dec!(2));
    }
}
