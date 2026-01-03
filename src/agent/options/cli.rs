//! CLI integration methods for AgentBuilder.

use std::path::Path;

use crate::auth::Auth;
use crate::common::Provider;
use crate::config::{Settings, SettingsLoader};
use crate::context::LeveledMemoryProvider;
use crate::hooks::CommandHook;
use crate::permissions::{PermissionMode, PermissionPolicy};
use crate::skills::file_skill_provider;
use crate::subagents::file_subagent_provider;

use super::builder::AgentBuilder;

impl AgentBuilder {
    pub async fn from_claude_code(mut self, path: impl AsRef<Path>) -> crate::Result<Self> {
        let path = path.as_ref();

        self = self.auth(Auth::ClaudeCli).await?;

        match SettingsLoader::load_merged(path).await {
            Ok(settings) => self = self.apply_settings(settings),
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "Failed to load settings")
            }
        }

        let project_skills = file_skill_provider().with_project_path(path);
        match project_skills.load_all().await {
            Ok(skills) => {
                for skill in skills {
                    self.skill_registry
                        .get_or_insert_with(crate::skills::SkillRegistry::new)
                        .register(skill);
                }
            }
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "Failed to load project skills")
            }
        }

        let user_skills = file_skill_provider().with_user_path();
        match user_skills.load_all().await {
            Ok(skills) => {
                for skill in skills {
                    self.skill_registry
                        .get_or_insert_with(crate::skills::SkillRegistry::new)
                        .register(skill);
                }
            }
            Err(e) => tracing::warn!(error = %e, "Failed to load user skills"),
        }

        let project_subagents = file_subagent_provider().with_project_path(path);
        match project_subagents.load_all().await {
            Ok(subagents) => {
                for subagent in subagents {
                    self.subagent_registry
                        .get_or_insert_with(crate::subagents::SubagentRegistry::with_builtins)
                        .register(subagent);
                }
            }
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "Failed to load project subagents")
            }
        }

        self.memory_provider = Some(
            LeveledMemoryProvider::from_project(path)
                .with_user()
                .with_enterprise(),
        );
        self.config.working_dir = Some(path.to_path_buf());

        Ok(self)
    }

    pub(super) fn apply_settings(mut self, settings: Settings) -> Self {
        let default_model = crate::agent::config::AgentModelConfig::default();

        if self.config.model.primary == default_model.primary
            && let Some(model) = &settings.model
        {
            self.config.model.primary = model.clone();
        }
        if self.config.model.small == default_model.small
            && let Some(small) = &settings.small_model
        {
            self.config.model.small = small.clone();
        }
        if self.config.model.max_tokens == default_model.max_tokens
            && let Some(max_tokens) = settings.max_tokens
        {
            self.config.model.max_tokens = max_tokens;
        }

        if let Some(ref hooks_settings) = settings.hooks {
            for hook in CommandHook::from_settings(hooks_settings) {
                self.hooks.register(hook);
            }
        }

        self.config.security.env.extend(settings.env.clone());

        if !settings.permissions.is_empty() {
            let loaded_policy = settings.permissions.to_policy();
            let existing_policy = std::mem::take(&mut self.config.security.permission_policy);
            self.config.security.permission_policy =
                Self::merge_permission_policies(loaded_policy, existing_policy);
        }

        if settings.sandbox.is_enabled() || settings.sandbox.has_network_settings() {
            self.sandbox_settings = Some(settings.sandbox.clone());
        }

        if self.output_style_name.is_none()
            && let Some(ref style_name) = settings.output_style
        {
            self.output_style_name = Some(style_name.clone());
        }

        for (name, config_value) in &settings.mcp_servers {
            if self.mcp_configs.contains_key(name) {
                continue;
            }
            match serde_json::from_value(config_value.clone()) {
                Ok(config) => {
                    self.mcp_configs.insert(name.clone(), config);
                }
                Err(e) => {
                    tracing::warn!(
                        server = %name,
                        error = %e,
                        "Failed to parse MCP server config from settings, skipping"
                    );
                }
            }
        }

        self
    }

    fn merge_permission_policies(
        from_settings: PermissionPolicy,
        programmatic: PermissionPolicy,
    ) -> PermissionPolicy {
        let mode = if programmatic.mode != PermissionMode::Default {
            programmatic.mode
        } else {
            from_settings.mode
        };

        let mut rules = from_settings.rules;
        rules.extend(programmatic.rules);

        let mut tool_limits = from_settings.tool_limits;
        tool_limits.extend(programmatic.tool_limits);

        PermissionPolicy {
            mode,
            rules,
            tool_limits,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_explicit_model_takes_precedence_over_settings() {
        let settings = Settings {
            model: Some("settings-model".to_string()),
            small_model: Some("settings-small".to_string()),
            max_tokens: Some(1000),
            ..Default::default()
        };

        let builder = AgentBuilder::new()
            .model("explicit-model")
            .small_model("explicit-small")
            .max_tokens(2000)
            .apply_settings(settings);

        assert_eq!(builder.config.model.primary, "explicit-model");
        assert_eq!(builder.config.model.small, "explicit-small");
        assert_eq!(builder.config.model.max_tokens, 2000);
    }

    #[test]
    fn test_settings_apply_when_no_explicit_config() {
        let settings = Settings {
            model: Some("settings-model".to_string()),
            small_model: Some("settings-small".to_string()),
            max_tokens: Some(1000),
            ..Default::default()
        };

        let builder = AgentBuilder::new().apply_settings(settings);

        assert_eq!(builder.config.model.primary, "settings-model");
        assert_eq!(builder.config.model.small, "settings-small");
        assert_eq!(builder.config.model.max_tokens, 1000);
    }

    #[test]
    fn test_partial_explicit_config() {
        let settings = Settings {
            model: Some("settings-model".to_string()),
            small_model: Some("settings-small".to_string()),
            ..Default::default()
        };

        let builder = AgentBuilder::new()
            .model("explicit-model")
            .apply_settings(settings);

        assert_eq!(builder.config.model.primary, "explicit-model");
        assert_eq!(builder.config.model.small, "settings-small");
    }
}
