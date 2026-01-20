use super::family::{ModelFamily, ModelRole};
use super::provider::ProviderIds;
use super::registry::ModelRegistry;
use super::spec::{Capabilities, ModelSpec, ModelVersion, Pricing};

pub fn register_all(registry: &mut ModelRegistry) {
    registry.register(sonnet_4_5());
    registry.set_default(ModelRole::Primary, "claude-sonnet-4-5-20250929".into());

    registry.register(haiku_4_5());
    registry.set_default(ModelRole::Small, "claude-haiku-4-5-20251001".into());

    registry.register(opus_4_5());
    registry.set_default(ModelRole::Reasoning, "claude-opus-4-5-20251101".into());
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
        pricing: Pricing::new(3.0, 15.0),
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
        pricing: Pricing::new(0.80, 4.0),
        provider_ids: ProviderIds {
            anthropic: Some("claude-haiku-4-5-20251001".into()),
            bedrock: Some("anthropic.claude-haiku-4-5-20251001-v1:0".into()),
            vertex: Some("claude-haiku-4-5@20251001".into()),
            foundry: Some("claude-haiku-4-5".into()),
        },
    }
}

fn opus_4_5() -> ModelSpec {
    ModelSpec {
        id: "claude-opus-4-5-20251101".into(),
        family: ModelFamily::Opus,
        version: ModelVersion {
            version: "4.5".into(),
            snapshot: Some("20251101".into()),
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
        pricing: Pricing::new(15.0, 75.0),
        provider_ids: ProviderIds {
            anthropic: Some("claude-opus-4-5-20251101".into()),
            bedrock: Some("anthropic.claude-opus-4-5-20251101-v1:0".into()),
            vertex: Some("claude-opus-4-5@20251101".into()),
            foundry: Some("claude-opus-4-5".into()),
        },
    }
}
