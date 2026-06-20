use serde::{Serialize, Deserialize};
use chrono::Utc;
use serde_json::Value;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Dialogue {
    pub speaker: String,
    pub content: Option<String>,
    pub timestamp: i64,
    pub actions: Option<Vec<Value>>,
}
impl Dialogue {
    pub fn new(speaker: String, content: Option<String>, actions:Option<Vec<Value>>) -> Self {
        Self {
            speaker,
            content,
            timestamp: Utc::now().timestamp_millis(),
            actions,
        }
    }
}