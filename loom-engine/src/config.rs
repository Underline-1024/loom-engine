use anyhow::{bail, Context};
use chrono::Utc;
use serde::{Serialize, Deserialize};
use std::{fs, path::PathBuf};
use anyhow::Result;
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct WorldConfig {
    pub name: String,
    pub mode: GameMode,
    pub prompt: String,
    pub timestamp: i64,
}
impl WorldConfig {
    pub fn new(name: String, mode: GameMode, prompt: String) -> Self {
        Self {
            name,
            mode,
            prompt,
            timestamp: Utc::now().timestamp_millis(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub enum GameMode {
    #[default]
    Normal,
    Author,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LlmConfig {
    pub provider: String,
    pub enable_dynamic: bool,
    pub model: String,
    #[serde(default)]
    pub embedding_model: Option<String>,
    pub system_prompt: String,
    #[serde(default)]
    pub max_tokens: Option<u64>,
    pub max_turns: usize,
}
impl LlmConfig {
    pub fn load() -> Result<Self> {
        let path = PathBuf::from("./configs/llm_config.json");
        
        if !path.exists() {
            bail!("Configuration file not found: {}", path.display());
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read configuration file: {}", path.display()))?;
        
        let config: Self = serde_json::from_str(&content)
            .with_context(|| "Failed to parse configuration file: Invalid JSON format")?;
        
        Ok(config)
    }
}