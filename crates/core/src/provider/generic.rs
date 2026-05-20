use std::path::Path;

use nca_common::config::{GenericProviderConfig, NcaConfig, ProviderKind};
use nca_common::message::Message;
use nca_common::tool::ToolDefinition;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};

use super::openai_compat::{map_provider_error, openai_request_body, spawn_openai_stream};
use super::{Provider, ProviderError, StreamChunk};

/// A generic OpenAI-compatible provider that can be configured for any OpenAI-compatible API.
pub struct GenericOpenAiCompatibleProvider {
    client: reqwest::Client,
    config: GenericProviderConfig,
    provider_name: &'static str,
    max_tokens: u32,
}

impl GenericOpenAiCompatibleProvider {
    pub fn from_config(
        config: &NcaConfig,
        provider_kind: ProviderKind,
        provider_name: &'static str,
    ) -> Result<Self, ProviderError> {
        let generic_config = match provider_kind {
            ProviderKind::NvidiaNim => config.provider.nvidianim.clone(),
            ProviderKind::OpenCode => config.provider.opcode.clone(),
            ProviderKind::Glm => config.provider.glm.clone(),
            ProviderKind::Kimi => config.provider.kimi.clone(),
            ProviderKind::KiloCode => config.provider.kilocode.clone(),
            ProviderKind::Generic => config.provider.nvidianim.clone(),
            _ => {
                return Err(ProviderError::Configuration(format!(
                    "provider {:?} is not supported by GenericOpenAiCompatibleProvider",
                    provider_kind
                )))
            }
        };

        let api_key = generic_config.resolve_api_key().ok_or_else(|| {
            ProviderError::Configuration(format!(
                "missing {} API key; set {} or provide `provider.{}.api_key` in config",
                provider_name, generic_config.api_key_env, provider_name.to_lowercase()
            ))
        })?;

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {api_key}")).map_err(|err| {
                ProviderError::Configuration(format!(
                    "failed to build {} authorization header: {err}",
                    provider_name
                ))
            })?,
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .map_err(|err| {
                ProviderError::Configuration(format!("failed to build HTTP client: {err}"))
            })?;

        Ok(Self {
            client,
            config: generic_config,
            provider_name,
            max_tokens: config.model.max_tokens,
        })
    }

    fn endpoint(&self) -> String {
        format!(
            "{}/v1/chat/completions",
            self.config.base_url.trim_end_matches('/')
        )
    }
}

#[async_trait::async_trait]
impl Provider for GenericOpenAiCompatibleProvider {
    async fn chat(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        model: &str,
        workspace_root: &Path,
    ) -> Result<tokio::sync::mpsc::Receiver<StreamChunk>, ProviderError> {
        let model = if model.is_empty() {
            self.config.model.clone()
        } else {
            model.to_string()
        };

        let body = openai_request_body(
            messages,
            tools,
            &model,
            self.max_tokens,
            self.config.temperature,
            workspace_root,
        )?;

        let response = self
            .client
            .post(self.endpoint())
            .json(&body)
            .send()
            .await
            .map_err(|err| ProviderError::RequestFailed(err.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let body_text = response.text().await.unwrap_or_default();
            return Err(map_provider_error(status, body_text));
        }

        Ok(spawn_openai_stream(response, self.provider_name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::test_support::{collect_chunks, spawn_sse_server};
    use nca_common::message::Message;

    #[tokio::test]
    async fn generic_provider_streams_text_and_usage() {
        let body = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"Hello \"},\"index\":0,\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":5,\"completion_tokens\":3}}\n\n",
            "data: [DONE]\n\n"
        )
        .to_string();
        let base_url = spawn_sse_server(body, 200, |request| {
            assert_eq!(request.url(), "/v1/chat/completions");
        });

        let mut config = NcaConfig::default();
        config.provider.nvidianim.api_key = Some("test-key".into());
        config.provider.nvidianim.base_url = base_url;

        let provider =
            GenericOpenAiCompatibleProvider::from_config(&config, ProviderKind::NvidiaNim, "NVIDIA")
                .expect("provider");
        let stream = provider
            .chat(
                &[Message::user("hello")],
                &[],
                "",
                std::path::Path::new("."),
            )
            .await
            .expect("chat stream");

        let chunks = collect_chunks(stream).await;
        assert!(matches!(&chunks[0], StreamChunk::TextDelta(text) if text == "Hello "));
        assert!(matches!(
            &chunks[1],
            StreamChunk::Usage {
                input_tokens: 5,
                output_tokens: 3
            }
        ));
        assert!(matches!(chunks.last(), Some(StreamChunk::Done)));
    }
}
