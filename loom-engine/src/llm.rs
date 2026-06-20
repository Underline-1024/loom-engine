use std::sync::Arc;
use async_trait::async_trait;
use rig::agent::Agent;
use rig::client::ProviderClient;
use rig::completion::Prompt;
use anyhow::{Result, Context};
use tokio::sync::Mutex;
use crate::config::LlmConfig;
pub use loom_engine_tool_macro::tool;
pub mod tool;
pub mod rig_adapter;
pub use tool::{
    Tool, ToolDefinition, ToolOutput, ToolError,
    FunctionDefinition, format_template
};
pub use rig_adapter::{
    create_rig_agent, create_rig_agent_with_dynamic_tools,
};

macro_rules! provider_match {
    (
        $config:expr,
        $dynamic_count:expr,
        $($name:literal => $provider:ident @ $mode:ident),+ $(,)?
    ) => {{
        match $config.provider.as_str() {
            $(
                $name => provider_match!(@inner $config, $dynamic_count, $provider, $mode),
            )+
            _ => anyhow::bail!("Unsupported provider: {}", $config.provider),
        }
    }};

    // 动态工具分支
    (@inner $config:expr, $dynamic_count:expr, $provider:ident, dynamic) => {{
        let client = rig::providers::$provider::Client::from_env();
        let embedding_model = $config.embedding_model.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Provider '{}' requires embedding_model in config for dynamic tools", stringify!($provider)))?;
        let agent = create_rig_agent_with_dynamic_tools(
            client,
            &$config.model,
            embedding_model,
            &$config.system_prompt,
            $dynamic_count,
        ).await
        .with_context(|| format!("Failed to create {} agent with dynamic tools", stringify!($provider)))?;
        let boxed: Box<dyn DynAgent> = Box::new(agent);
        Ok(boxed)
    }};

    // 静态工具分支
    (@inner $config:expr, $dynamic_count:expr, $provider:ident, static) => {{
        let client = rig::providers::$provider::Client::from_env();
        let agent = create_rig_agent(
            client,
            &$config.model,
            &$config.system_prompt,
        ).await
        .with_context(|| format!("Failed to create {} agent with static tools", stringify!($provider)))?;
        let boxed: Box<dyn DynAgent> = Box::new(agent);
        Ok(boxed)
    }};
}

pub struct Narrator {
    agent: Mutex<Option<Arc<dyn DynAgent>>>,
}
impl Narrator {
    pub fn new() -> Self {
        Self {
            agent: Mutex::new(None),
        }
    }
    async fn create_agent_from_config(
        config: &LlmConfig,
        dynamic_tool_count: usize,
    ) -> Result<Box<dyn DynAgent>> {
        if config.enable_dynamic {
            provider_match!(config, dynamic_tool_count,
                "azure" => azure @ dynamic,
                "cohere" => cohere @ dynamic,
                "gemini" => gemini @ dynamic,
                "mistral" => mistral @ dynamic,
                "ollama" => ollama @ dynamic,
                "openai" => openai @ dynamic,
                "together" => together @ dynamic,
                "anthropic" => anthropic @ static,
                "deepseek" => deepseek @ static,
                "galadriel" => galadriel @ static,
                "groq" => groq @ static,
                "hyperbolic" => hyperbolic @ static,
                "mira" => mira @ static,
                "moonshot" => moonshot @ static,
                "openrouter" => openrouter @ static,
                "perplexity" => perplexity @ static,
                "xai" => xai @ static,
                "huggingface" => huggingface @ static,
            )
        } else {
            provider_match!(config, dynamic_tool_count,
                "azure" => azure @ static,
                "cohere" => cohere @ static,
                "gemini" => gemini @ static,
                "mistral" => mistral @ static,
                "ollama" => ollama @ static,
                "openai" => openai @ static,
                "together" => together @ static,
                "anthropic" => anthropic @ static,
                "deepseek" => deepseek @ static,
                "galadriel" => galadriel @ static,
                "groq" => groq @ static,
                "hyperbolic" => hyperbolic @ static,
                "mira" => mira @ static,
                "moonshot" => moonshot @ static,
                "openrouter" => openrouter @ static,
                "perplexity" => perplexity @ static,
                "xai" => xai @ static,
                "huggingface" => huggingface @ static,
            )
        }
        
    }
    pub async fn init(
        &self,
        llm_config: &LlmConfig,
        dynamic_tool_count: usize,
    ) -> Result<()> {
        let agent = Self::create_agent_from_config(llm_config, dynamic_tool_count)
            .await
            .context("Failed to initialize agent")?;

        let mut guard = self.agent.lock().await;
        if guard.is_some() {
            return Err(anyhow::anyhow!("Narrator already initialized"));
        }
        *guard = Some(Arc::from(agent));

        Ok(())
    }

    pub async fn chat(&self, prompt: &str) -> Result<String> {
        let guard = self.agent.lock().await;
        let agent = guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Narrator not initialized. Call init() first."))?;

        agent.chat(prompt).await
    }
}

#[async_trait]
pub trait DynAgent: Send + Sync {
    async fn chat(&self, prompt: &str) -> Result<String>;
}
#[async_trait]
impl<T> DynAgent for Agent<T>
where
    T: rig::completion::CompletionModel + Send + Sync,
{
    async fn chat(&self, prompt: &str) -> Result<String> {
        self.prompt(prompt).await.map_err(|e| e.into())
    }
}