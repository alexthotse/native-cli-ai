use nca_common::config::{NcaConfig, ProviderKind};

use super::anthropic::AnthropicProvider;
use super::generic::GenericOpenAiCompatibleProvider;
use super::minimax::MiniMaxProvider;
use super::openai::OpenAiProvider;
use super::openrouter::OpenRouterProvider;
use super::{Provider, ProviderError};

/// Build the configured provider for the current workspace.
pub fn build_provider(config: &NcaConfig) -> Result<Box<dyn Provider>, ProviderError> {
    match config.provider.default {
        ProviderKind::MiniMax => Ok(Box::new(MiniMaxProvider::from_config(config)?)),
        ProviderKind::OpenRouter => Ok(Box::new(OpenRouterProvider::from_config(config)?)),
        ProviderKind::Anthropic => Ok(Box::new(AnthropicProvider::from_config(config)?)),
        ProviderKind::OpenAi => Ok(Box::new(OpenAiProvider::from_config(config)?)),
        ProviderKind::NvidiaNim => Ok(Box::new(GenericOpenAiCompatibleProvider::from_config(
            config,
            ProviderKind::NvidiaNim,
            "NVIDIA NIM",
        )?)),
        ProviderKind::OpenCode => Ok(Box::new(GenericOpenAiCompatibleProvider::from_config(
            config,
            ProviderKind::OpenCode,
            "OpenCode",
        )?)),
        ProviderKind::Glm => Ok(Box::new(GenericOpenAiCompatibleProvider::from_config(
            config,
            ProviderKind::Glm,
            "GLM",
        )?)),
        ProviderKind::Kimi => Ok(Box::new(GenericOpenAiCompatibleProvider::from_config(
            config,
            ProviderKind::Kimi,
            "Kimi",
        )?)),
        ProviderKind::KiloCode => Ok(Box::new(GenericOpenAiCompatibleProvider::from_config(
            config,
            ProviderKind::KiloCode,
            "KiloCode",
        )?)),
        ProviderKind::Generic => Ok(Box::new(GenericOpenAiCompatibleProvider::from_config(
            config,
            ProviderKind::Generic,
            "Generic",
        )?)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factory_builds_each_supported_provider_when_configured() {
        for kind in ProviderKind::ALL {
            let mut config = NcaConfig::default();
            config.provider.default = kind;
            match kind {
                ProviderKind::MiniMax => {
                    config.provider.minimax.api_key = Some("minimax-key".into());
                }
                ProviderKind::OpenAi => {
                    config.provider.openai.api_key = Some("openai-key".into());
                }
                ProviderKind::Anthropic => {
                    config.provider.anthropic.api_key = Some("anthropic-key".into());
                }
                ProviderKind::OpenRouter => {
                    config.provider.openrouter.api_key = Some("openrouter-key".into());
                }
                ProviderKind::NvidiaNim => {
                    config.provider.nvidianim.api_key = Some("nvidia-key".into());
                }
                ProviderKind::OpenCode => {
                    config.provider.opcode.api_key = Some("opencode-key".into());
                }
                ProviderKind::Glm => {
                    config.provider.glm.api_key = Some("glm-key".into());
                }
                ProviderKind::Kimi => {
                    config.provider.kimi.api_key = Some("kimi-key".into());
                }
                ProviderKind::KiloCode => {
                    config.provider.kilocode.api_key = Some("kilocode-key".into());
                }
                ProviderKind::Generic => {
                    config.provider.nvidianim.api_key = Some("generic-key".into());
                }
            }

            let provider = build_provider(&config);
            assert!(
                provider.is_ok(),
                "expected provider {:?} to build, got {:?}",
                kind,
                provider.as_ref().err()
            );
        }
    }

    #[test]
    fn factory_fails_loudly_when_selected_provider_is_missing_credentials() {
        let mut config = NcaConfig::default();
        config.provider.default = ProviderKind::OpenAi;
        match build_provider(&config) {
            Ok(_) => panic!("missing credentials should fail"),
            Err(error) => {
                assert!(
                    matches!(error, ProviderError::Configuration(message) if message.contains("missing OpenAI API key"))
                );
            }
        }
    }
}
