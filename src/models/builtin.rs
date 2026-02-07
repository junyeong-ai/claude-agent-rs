use rust_decimal_macros::dec;

use super::family::{ModelFamily, ModelRole};
use super::provider::ProviderIds;
use super::registry::ModelRegistry;
use super::spec::{Capabilities, ModelSpec, ModelVersion};
use crate::budget::ModelPricing;

pub fn register_all(registry: &mut ModelRegistry) {
    registry.register(sonnet_4_5());
    registry.set_default(ModelRole::Primary, "claude-sonnet-4-5-20250929".into());

    registry.register(haiku_4_5());
    registry.set_default(ModelRole::Small, "claude-haiku-4-5-20251001".into());

    registry.register(opus_4_6());
    registry.set_default(ModelRole::Reasoning, "claude-opus-4-6".into());
}

fn sonnet_4_5() -> ModelSpec {
    ModelSpec {
        id: "claude-sonnet-4-5-20250929".into(),
        family: ModelFamily::Sonnet,
        version: ModelVersion {
            version: "4.5".into(),
            snapshot: Some("20250929".into()),
            knowledge_cutoff: Some("2025-01".into()),
        },
        capabilities: Capabilities {
            context_window: 200_000,
            extended_context_window: Some(1_000_000),
            max_output_tokens: 64_000,
            thinking: true,
            vision: true,
            tool_use: true,
            caching: true,
        },
        pricing: ModelPricing::from_base(dec!(3), dec!(15)),
        provider_ids: ProviderIds {
            anthropic: Some("claude-sonnet-4-5-20250929".into()),
            bedrock: Some("anthropic.claude-sonnet-4-5-20250929-v1:0".into()),
            vertex: Some("claude-sonnet-4-5@20250929".into()),
            foundry: Some("claude-sonnet-4-5".into()),
        },
    }
}

fn haiku_4_5() -> ModelSpec {
    ModelSpec {
        id: "claude-haiku-4-5-20251001".into(),
        family: ModelFamily::Haiku,
        version: ModelVersion {
            version: "4.5".into(),
            snapshot: Some("20251001".into()),
            knowledge_cutoff: Some("2025-01".into()),
        },
        capabilities: Capabilities {
            context_window: 200_000,
            extended_context_window: None,
            max_output_tokens: 64_000,
            thinking: true,
            vision: true,
            tool_use: true,
            caching: true,
        },
        pricing: ModelPricing::from_base(dec!(0.80), dec!(4)),
        provider_ids: ProviderIds {
            anthropic: Some("claude-haiku-4-5-20251001".into()),
            bedrock: Some("anthropic.claude-haiku-4-5-20251001-v1:0".into()),
            vertex: Some("claude-haiku-4-5@20251001".into()),
            foundry: Some("claude-haiku-4-5".into()),
        },
    }
}

fn opus_4_6() -> ModelSpec {
    ModelSpec {
        id: "claude-opus-4-6".into(),
        family: ModelFamily::Opus,
        version: ModelVersion {
            version: "4.6".into(),
            snapshot: None,
            knowledge_cutoff: Some("2025-05".into()),
        },
        capabilities: Capabilities {
            context_window: 200_000,
            extended_context_window: None,
            max_output_tokens: 64_000,
            thinking: true,
            vision: true,
            tool_use: true,
            caching: true,
        },
        pricing: ModelPricing::from_base(dec!(15), dec!(75)),
        provider_ids: ProviderIds {
            anthropic: Some("claude-opus-4-6".into()),
            bedrock: Some("anthropic.claude-opus-4-6-v1:0".into()),
            vertex: Some("claude-opus-4-6".into()),
            foundry: Some("claude-opus-4-6".into()),
        },
    }
}
