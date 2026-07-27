use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use rig::message::Message;
use rig::{agent::Agent, completion::Document};
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
    base_system_prompt: Mutex<String>,
}
impl Narrator {
    pub fn new() -> Self {
        Self {
            agent: Mutex::new(None),
            base_system_prompt: Mutex::new(String::new()),
        }
    }
    async fn create_agent_from_config(
        config: &LlmConfig,
        dynamic_tool_count: usize,
    ) -> Result<Box<dyn DynAgent>> {
        // ⬇️ 特殊处理 Anthropic
        if config.provider == "anthropic" {
            // 模仿 OpenAI 的实现
            let base_url: Option<String> = std::env::var("ANTHROPIC_BASE_URL").ok();
            let api_key = std::env::var("ANTHROPIC_API_KEY")
                .context("ANTHROPIC_API_KEY not set in environment")?;
            
            let mut builder = rig::providers::anthropic::Client::builder()
                .api_key(&api_key);
            
            if let Some(base) = base_url {
                builder = builder.base_url(&base);
                tracing::info!("Using custom base_url from env: {}", base);
            }
            
            let client = builder.build()
                .map_err(|e| anyhow::anyhow!("Failed to build Anthropic client: {}", e))?;
            
            let agent = create_rig_agent(
                client,
                &config.model,
                &config.system_prompt,
            ).await
            .context("Failed to create Anthropic agent")?;
            
            let boxed: Box<dyn DynAgent> = Box::new(agent);
            return Ok(boxed);
        }

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

        let mut base_guard = self.base_system_prompt.lock().await;
        *base_guard = llm_config.system_prompt.clone();

        let mut guard = self.agent.lock().await;
        if guard.is_some() {
            return Err(anyhow::anyhow!("Narrator already initialized"));
        }
        *guard = Some(Arc::from(agent));

        Ok(())
    }

    pub async fn chat(&self, prompt: &str, history: &mut Vec<Message>) -> Result<String> {
        let guard = self.agent.lock().await;
        let agent = guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Narrator not initialized. Call init() first."))?;

        agent.chat(prompt, history).await
    }

    pub async fn add_context(&self, doc: &str) -> Result<()> {
        let mut guard = self.agent.lock().await;
        let agent_arc = guard
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Narrator not initialized"))?;

        // 尝试获取 Arc 的唯一可变引用
        let agent = Arc::get_mut(agent_arc)
            .ok_or_else(|| anyhow::anyhow!("Agent is currently shared; cannot mutate. Ensure no other clones exist."))?;

        agent.add_context(doc).await;
        Ok(())
    }

    pub async fn clear_context(&self) -> Result<()> {
        let mut guard = self.agent.lock().await;
        let agent_arc = guard
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Narrator not initialized"))?;

        let agent = Arc::get_mut(agent_arc)
            .ok_or_else(|| anyhow::anyhow!("Agent is currently shared; cannot mutate. Ensure no other clones exist."))?;

        agent.clear_context().await;
        Ok(())
    }

    pub async fn update_preamble(&self, dynamic_rule: Option<&str>) -> Result<()> {
        // 1. 获取缓存的基础提示词
        let base_prompt = self.base_system_prompt.lock().await.clone();
        
        // 2. 智能拼接：如果有动态规则，就追加上去；如果没有，就只用基础提示词
        let full_preamble = match dynamic_rule {
            Some(rule) if !rule.is_empty() => {
                format!(
                    "{}\n\n[CURRENT SESSION DIRECTIVE - HIGHEST PRIORITY]\n{}", 
                    base_prompt, rule
                )
            }
            _ => base_prompt,
        };

        // 3. 获取 Agent 并修改 preamble
        let mut guard = self.agent.lock().await;
        let agent_arc = guard
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Narrator not initialized"))?;

        let agent = Arc::get_mut(agent_arc)
            .ok_or_else(|| anyhow::anyhow!("Agent is currently shared; cannot mutate."))?;

        // 4. 将拼接好的完整提示词设置给 Rig Agent
        agent.set_preamble(full_preamble).await;
        
        Ok(())
    }
}

#[async_trait]
pub trait DynAgent: Send + Sync {
    async fn chat(&self, prompt: &str, history: &mut Vec<Message>) -> Result<String>;
    async fn add_context(&mut self, doc: &str);
    async fn clear_context(&mut self);
    async fn set_preamble(&mut self, preamble: String);
}
#[async_trait]
impl<T> DynAgent for Agent<T>
where
    T: rig::completion::CompletionModel + Send + Sync,
{
    async fn chat(&self, prompt: &str, history: &mut Vec<Message>) -> Result<String> {
        self.prompt(prompt).with_history(history).await.map_err(|e| e.into())
    }

    async fn add_context(&mut self, doc: &str) {
        self.static_context.push(Document {
            id: format!("static_doc_{}", self.static_context.len()),
            text: doc.into(),
            additional_props: HashMap::new(),
        });
    }

    async fn clear_context(&mut self) {
        self.static_context.clear();
    }

    async fn set_preamble(&mut self, preamble: String) {
        self.preamble = Some(preamble);
    }
}